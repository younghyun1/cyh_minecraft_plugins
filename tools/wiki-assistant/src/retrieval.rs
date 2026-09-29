//! A single BM25 lookup returns bounded, title-boosted passages before inference.
use crate::{
    corpus::Manifest,
    error::{Error, Result},
    index::{self, Fields},
};
use serde::Serialize;
use std::{collections::HashMap, fs, path::Path, time::Instant};
use tantivy::{
    Index, IndexReader, Term,
    collector::TopDocs,
    query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery},
    schema::{IndexRecordOption, TantivyDocument, Value},
};

pub struct Search {
    reader: IndexReader,
    fields: Fields,
    manifest: Manifest,
}

#[derive(Debug, Serialize)]
pub struct Hit {
    pub revision_id: u64,
    pub title: String,
    pub url: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct Results {
    pub retrieval_micros: u128,
    pub hits: Vec<Hit>,
}

impl Search {
    /// Open only a completed compatible index and copy its immutable segments into RAM.
    pub fn open(path: &Path) -> Result<Self> {
        let manifest: Manifest =
            serde_json::from_reader(fs::File::open(path.join("wiki-manifest.json"))?)?;
        if !manifest.complete || manifest.format != 1 || manifest.source != crate::corpus::API {
            return Err(Error::Invalid(
                "index lacks a complete compatible snapshot".into(),
            ));
        }
        let (schema, fields) = index::schema();
        let index = Index::open(crate::memory::index(path)?)?;
        if index.schema() != schema {
            return Err(Error::Invalid("index schema mismatch".into()));
        }
        index.tokenizers().register("wiki_en", index::analyzer());
        Ok(Self {
            reader: index
                .reader_builder()
                .doc_store_cache_num_blocks(8)
                .try_into()?,
            fields,
            manifest,
        })
    }

    /// Full-page reads must come from the same immutable snapshot as ranked excerpts.
    pub fn verify_corpus(&self, path: &Path) -> Result<()> {
        if self.manifest != crate::corpus::load_manifest(path, true)? {
            return Err(Error::Invalid(
                "wiki corpus and index snapshots differ".into(),
            ));
        }
        Ok(())
    }

    /// Literal token queries prevent player-supplied query syntax and expensive wildcard expansion.
    pub fn search(&self, question: &str) -> Result<Results> {
        let started = Instant::now();
        let mut analyzer = index::analyzer();
        let mut stream = analyzer.token_stream(question);
        let mut words = Vec::new();
        while stream.advance() {
            let word = stream.token().text.clone();
            if matches!(
                word.as_str(),
                "mani" | "best" | "much" | "which" | "where" | "when" | "get" | "need" | "use"
            ) {
                continue;
            }
            if !words.contains(&word) {
                words.push(word);
            }
            if words.len() == 24 {
                break;
            }
        }
        let mut terms: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for length in 1..=words.len().min(5) {
            for phrase in words.windows(length) {
                let query = TermQuery::new(
                    Term::from_field_text(self.fields.page_key, &phrase.join(" ")),
                    IndexRecordOption::Basic,
                );
                terms.push((
                    Occur::Should,
                    Box::new(tantivy::query::ConstScoreQuery::new(
                        Box::new(query),
                        12.0 * length as f32,
                    )),
                ));
            }
        }
        for word in &words {
            for (field, boost) in [(self.fields.title, 1.5), (self.fields.body, 1.0)] {
                let query = TermQuery::new(
                    Term::from_field_text(field, word),
                    IndexRecordOption::WithFreqs,
                );
                terms.push((
                    Occur::Should,
                    Box::new(BoostQuery::new(Box::new(query), boost)),
                ));
            }
        }
        let searcher = self.reader.searcher();
        let candidates = searcher.search(
            &BooleanQuery::new(terms),
            &TopDocs::with_limit(24).order_by_score(),
        )?;
        let mut hits = Vec::new();
        let mut per_page = HashMap::new();
        for (_, address) in candidates {
            let doc: TantivyDocument = searcher.doc(address)?;
            let read = |field| {
                doc.get_first(field)
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| Error::Invalid("index document missing text".into()))
            };
            let url = read(self.fields.url)?;
            let count = per_page.entry(url.clone()).or_insert(0);
            if *count >= 2 {
                continue;
            }
            *count += 1;
            hits.push(Hit {
                revision_id: url
                    .rsplit('/')
                    .next()
                    .and_then(|id| id.parse().ok())
                    .ok_or_else(|| Error::Invalid("invalid revision URL".into()))?,
                title: read(self.fields.title)?,
                url,
                text: read(self.fields.body)?,
            });
            if hits.len() == 6 {
                break;
            }
        }
        Ok(Results {
            retrieval_micros: started.elapsed().as_micros(),
            hits,
        })
    }
}
