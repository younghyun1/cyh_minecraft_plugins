//! Real on-disk index checks with synthetic content and revision metadata.
use crate::{
    corpus::{self, Manifest, Page},
    error::Result,
    index,
    retrieval::Search,
};
use std::fs;

#[test]
fn snapshot_round_trip_ranks_recipe_and_rejects_incomplete() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let corpus_path = directory.path().join("corpus");
    fs::create_dir(&corpus_path)?;
    let pages: Vec<_> = [
        (
            "Bucket",
            "== Crafting ==\nCraft a bucket using three iron ingots in a V shape.",
        ),
        ("Lava", "Lava is hot and can be carried inside a bucket."),
        ("Cow", "Use a bucket on a cow to obtain milk."),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (title, text))| Page {
        page_id: id as u64 + 1,
        namespace: 0,
        title: title.into(),
        revision_id: id as u64 + 100,
        timestamp: "2026-09-29T00:00:00Z".into(),
        text: text.into(),
    })
    .collect();
    let bytes = serde_json::to_vec(&pages)?;
    fs::write(
        corpus_path.join("000000.json.zst"),
        zstd::encode_all(bytes.as_slice(), 1)?,
    )?;
    let mut manifest = Manifest {
        format: 1,
        source: corpus::API.into(),
        license: serde_json::json!({"text":"fixture"}),
        namespaces: vec![0],
        namespace_cursor: 1,
        continuation: None,
        batches: 1,
        pages: 3,
        compressed_bytes: 0,
        started_unix_seconds: 0,
        complete: false,
    };
    corpus::save_manifest(&corpus_path, &manifest)?;
    let index_path = directory.path().join("index");
    assert!(index::build(&corpus_path, &index_path).is_err());
    assert!(!index_path.exists());
    manifest.complete = true;
    corpus::save_manifest(&corpus_path, &manifest)?;
    index::build(&corpus_path, &index_path)?;
    let search = Search::open(&index_path)?;
    search.verify_corpus(&corpus_path)?;
    manifest.started_unix_seconds += 1;
    corpus::save_manifest(&corpus_path, &manifest)?;
    assert!(search.verify_corpus(&corpus_path).is_err());
    let results = search.search("How do I craft a bucket?")?;
    assert!(!results.hits.is_empty());
    assert!(results.hits[0].title.starts_with("Bucket"));
    assert!(results.hits[0].text.contains("three iron ingots"));
    assert!(results.hits[0].url.ends_with("/100"));
    assert!(
        search
            .search("qzxnonexistenttoken")
            .is_ok_and(|result| result.hits.is_empty())
    );
    assert!(search.search("title:* OR body:[0 TO 999999]").is_ok());
    assert!(index::build(&corpus_path, &index_path).is_err());
    Ok(())
}
