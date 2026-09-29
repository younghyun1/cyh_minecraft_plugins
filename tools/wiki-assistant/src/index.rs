//! Immutable section index; archive remains the authoritative revision copy.
use crate::{
    aliases, corpus,
    error::{Error, Result},
};
use std::{fs, path::Path};
use tantivy::{
    Index, doc,
    schema::{Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions},
};

#[derive(Clone, Copy)]
pub struct Fields {
    pub page_key: Field,
    pub title: Field,
    pub body: Field,
    pub url: Field,
}

/// Share an English stemmed analyzer across indexing and query tokenization.
pub fn analyzer() -> tantivy::tokenizer::TextAnalyzer {
    use tantivy::tokenizer::*;
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(StopWordFilter::remove(
            [
                "a", "an", "the", "how", "do", "i", "is", "to", "in", "of", "for", "and", "can",
                "what", "it",
            ]
            .map(str::to_owned),
        ))
        .filter(Stemmer::new(Language::English))
        .build()
}

/// Field names and tokenizer configuration are covered by format version 1.
pub fn schema() -> (Schema, Fields) {
    let mut builder = Schema::builder();
    let options = TextOptions::default().set_stored().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("wiki_en")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let title = builder.add_text_field("title", options.clone());
    let page_key = builder.add_text_field("page_key", STRING);
    let body = builder.add_text_field("body", options);
    let url = builder.add_text_field("url", STRING | STORED);
    (
        builder.build(),
        Fields {
            page_key,
            title,
            body,
            url,
        },
    )
}

/// Overlap preserves facts spanning chunk boundaries without retaining entire articles per hit.
pub fn chunks(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + 1800).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start = end - 200;
    }
    chunks
}

/// Write to a new directory only; incomplete indexes never receive a completion manifest.
pub fn build(corpus_path: &Path, output: &Path) -> Result<()> {
    let manifest = corpus::load_manifest(corpus_path, true)?;
    fs::create_dir(output)?;
    let aliases = aliases::load(corpus_path, manifest.batches)?;
    let (schema, fields) = schema();
    let index = Index::create_in_dir(output, schema)?;
    index.tokenizers().register("wiki_en", analyzer());
    let mut writer = index.writer_with_num_threads(1, 64 * 1024 * 1024)?;
    let mut sections = 0u64;
    let mut pages = 0u64;
    for batch in 0..manifest.batches {
        for page in corpus::read_batch(corpus_path, batch)? {
            pages += 1;
            // Transclusion source is archived, but templates and maintenance pages swamp gameplay ranking.
            if [4, 10, 14, 828].contains(&page.namespace)
                || page
                    .text
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("#redirect")
            {
                continue;
            }
            let url = format!(
                "https://minecraft.wiki/w/Special:Redirect/revision/{}",
                page.revision_id
            );
            let mut heading = String::new();
            for section in page.text.split("\n==") {
                let text = if let Some((title, body)) = section.split_once("==\n") {
                    heading = title.trim_matches('=').trim().chars().take(160).collect();
                    body
                } else {
                    section
                };
                for chunk in chunks(&crate::evidence::crafting(text)) {
                    let mut document = doc!(fields.title => format!("{} {heading}", page.title), fields.body => chunk, fields.url => url.clone(), fields.page_key => aliases::key(&page.title));
                    if let Some(names) = aliases.get(&page.title) {
                        for name in names {
                            document.add_text(fields.page_key, name);
                        }
                    }
                    writer.add_document(document)?;
                    sections += 1;
                }
            }
        }
    }
    if pages != manifest.pages || sections == 0 {
        return Err(Error::Invalid(
            "snapshot count mismatch or empty index".into(),
        ));
    }
    writer.commit()?;
    writer.wait_merging_threads()?;
    fs::write(
        output.join("wiki-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    tracing::info!(pages, sections, "Wiki index complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn unicode_chunks_overlap_without_losing_tail() {
        let text = "🪨".repeat(4001);
        let chunks = super::chunks(&text);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chars().count(), 1800);
        assert_eq!(chunks[2].chars().count(), 801);
        assert!(chunks.iter().all(|chunk| chunk.chars().all(|c| c == '🪨')));
    }
}
