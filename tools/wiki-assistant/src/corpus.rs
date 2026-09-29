//! Versioned, resumable, compressed copies of public wiki revisions.
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

pub const API: &str = "https://minecraft.wiki/api.php";
pub const MAX_BATCH: u64 = 32 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
    pub page_id: u64,
    pub namespace: i64,
    pub title: String,
    pub revision_id: u64,
    pub timestamp: String,
    pub text: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub source: String,
    pub license: serde_json::Value,
    pub namespaces: Vec<i64>,
    pub namespace_cursor: usize,
    pub continuation: Option<serde_json::Value>,
    pub batches: u64,
    pub pages: u64,
    pub compressed_bytes: u64,
    pub started_unix_seconds: u64,
    pub complete: bool,
}

/// Rename within one directory so a failed refresh preserves the last checkpoint.
pub fn save_manifest(root: &Path, manifest: &Manifest) -> Result<()> {
    let temp = root.join("manifest.json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(manifest)?)?;
    fs::rename(temp, root.join("manifest.json"))?;
    Ok(())
}

/// Refuse unknown formats and incomplete snapshots at serving/indexing boundaries.
pub fn load_manifest(root: &Path, require_complete: bool) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_reader(fs::File::open(root.join("manifest.json"))?)?;
    if manifest.format != 1 || manifest.source != API || (require_complete && !manifest.complete) {
        return Err(Error::Invalid(
            "unsupported or incomplete wiki snapshot".into(),
        ));
    }
    Ok(manifest)
}

/// Each batch has an independent compression frame for resume and bounded decoding.
pub fn read_batch(root: &Path, batch: u64) -> Result<Vec<Page>> {
    let file = fs::File::open(root.join(format!("{batch:06}.json.zst")))?;
    let mut bytes = Vec::new();
    zstd::Decoder::new(file)?
        .take(MAX_BATCH + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BATCH {
        return Err(Error::Invalid("wiki batch exceeds 32 MiB".into()));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Report actual UTF-8 content bytes separately from serialization and search structures.
pub fn stats(root: &Path, index: &Path) -> Result<()> {
    let manifest = load_manifest(root, true)?;
    let mut text_bytes = 0u64;
    let mut pages = 0u64;
    let mut max_page_bytes = 0usize;
    for batch in 0..manifest.batches {
        for page in read_batch(root, batch)? {
            pages += 1;
            text_bytes += page.text.len() as u64;
            max_page_bytes = max_page_bytes.max(page.text.len());
        }
    }
    if pages != manifest.pages {
        return Err(Error::Invalid("snapshot page count mismatch".into()));
    }
    let mut index_bytes = 0u64;
    for entry in fs::read_dir(index)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            index_bytes += entry.metadata()?.len();
        }
    }
    println!(
        "{}",
        serde_json::json!({"pages":pages,"text_bytes":text_bytes,"text_mib":text_bytes as f64 / 1048576.0,
        "archive_bytes":manifest.compressed_bytes,"archive_mib":manifest.compressed_bytes as f64 / 1048576.0,
        "index_bytes":index_bytes,"index_mib":index_bytes as f64 / 1048576.0,"max_page_bytes":max_page_bytes})
    );
    Ok(())
}
