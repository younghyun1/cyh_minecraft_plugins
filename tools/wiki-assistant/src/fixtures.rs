//! Shared synthetic snapshot with redirects, Unicode, and transclusion source.
use crate::{
    corpus::{self, Manifest, Page},
    error::Result,
    index,
    memory::Pages,
    retrieval::Search,
};
use std::fs;

/// Remove the source files after loading to prove both retrieval paths are memory-resident.
pub fn resident() -> Result<(Search, Pages)> {
    let root = tempfile::tempdir()?;
    let corpus = root.path().join("corpus");
    let index_path = root.path().join("index");
    fs::create_dir(&corpus)?;
    let pages: Vec<_> = [
        ("Bucket", "Three iron ingots craft a bucket.".into(), 0),
        ("Pail", "#REDIRECT [[Bucket]]".into(), 0),
        (
            "Template:HiddenRecipe",
            format!(
                "🪨\nsecret recipe {} Verification marker: CopperCactus9382. {}",
                "鉄".repeat(1000),
                "鉄".repeat(6000)
            ),
            10,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (title, text, namespace))| Page {
        page_id: id as u64 + 1,
        namespace,
        title: title.into(),
        revision_id: id as u64 + 100,
        timestamp: "2026-09-29T00:00:00Z".into(),
        text,
    })
    .collect();
    let bytes = serde_json::to_vec(&pages)?;
    fs::write(
        corpus.join("000000.json.zst"),
        zstd::encode_all(bytes.as_slice(), 1)?,
    )?;
    corpus::save_manifest(
        &corpus,
        &Manifest {
            format: 1,
            source: corpus::API.into(),
            license: serde_json::json!({"text":"fixture"}),
            namespaces: vec![0, 10],
            namespace_cursor: 2,
            continuation: None,
            batches: 1,
            pages: 3,
            compressed_bytes: 0,
            started_unix_seconds: 0,
            complete: true,
        },
    )?;
    index::build(&corpus, &index_path)?;
    let search = Search::open(&index_path)?;
    let pages = Pages::load(&corpus)?;
    root.close()?;
    Ok((search, pages))
}
