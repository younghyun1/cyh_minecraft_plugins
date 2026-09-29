//! Load immutable index files and complete wiki revisions into bounded resident memory.
use crate::{
    corpus::{self, Page},
    error::{Error, Result},
};
use std::{collections::HashMap, fs, io::Read, path::Path};
use tantivy::directory::{Directory, INDEX_WRITER_LOCK, META_LOCK, RamDirectory};

pub const MAX_INDEX_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TEXT_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_PAGES: usize = 100_000;

pub struct Pages {
    pub pages: Vec<Page>,
    revisions: HashMap<u64, usize>,
    pub text_bytes: u64,
}

impl Pages {
    /// Index is loaded first, so index-copy scratch space does not overlap full-text loading.
    pub fn load(path: &Path) -> Result<Self> {
        let manifest = corpus::load_manifest(path, true)?;
        if manifest.pages > MAX_PAGES as u64 {
            return Err(Error::Invalid("wiki exceeds 100,000 resident pages".into()));
        }
        let mut pages = Vec::with_capacity(manifest.pages as usize);
        let mut revisions = HashMap::with_capacity(manifest.pages as usize);
        let mut text_bytes = 0u64;
        let mut metadata_bytes = 0u64;
        for batch in 0..manifest.batches {
            for page in corpus::read_batch(path, batch)? {
                text_bytes += page.text.capacity() as u64;
                metadata_bytes += (page.title.capacity() + page.timestamp.capacity()) as u64;
                if text_bytes > MAX_TEXT_BYTES
                    || metadata_bytes > 16 * 1024 * 1024
                    || pages.len() >= MAX_PAGES
                {
                    return Err(Error::Invalid(
                        "resident wiki exceeds text, metadata, or page budget".into(),
                    ));
                }
                if revisions.insert(page.revision_id, pages.len()).is_some() {
                    return Err(Error::Invalid("duplicate wiki revision".into()));
                }
                pages.push(page);
            }
        }
        if pages.len() as u64 != manifest.pages {
            return Err(Error::Invalid("snapshot count mismatch".into()));
        }
        Ok(Self {
            pages,
            revisions,
            text_bytes,
        })
    }

    pub fn revision(&self, revision: u64) -> Option<&Page> {
        self.revisions
            .get(&revision)
            .and_then(|i| self.pages.get(*i))
    }
}

/// Copy every search structure into Tantivy's RAM directory; no mmap/page-cache dependency remains.
pub fn index(path: &Path) -> Result<RamDirectory> {
    let mut size = 0u64;
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        // Disk lock files are inert OS-lock handles; copying them creates held RAM locks.
        if Path::new(&entry.file_name()) == META_LOCK.filepath
            || Path::new(&entry.file_name()) == INDEX_WRITER_LOCK.filepath
        {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(Error::Invalid("index contains a non-regular file".into()));
        }
        let length = entry.metadata()?.len();
        size = size
            .checked_add(length)
            .ok_or_else(|| Error::Invalid("index size overflow".into()))?;
        if size > MAX_INDEX_BYTES {
            return Err(Error::Invalid("resident index exceeds 128 MiB".into()));
        }
        files.push((entry.file_name(), entry.path(), length));
    }
    let directory = RamDirectory::create();
    for (name, path, length) in files {
        let mut bytes = Vec::with_capacity(length as usize);
        fs::File::open(path)?
            .take(length + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != length {
            return Err(Error::Invalid("index changed while loading".into()));
        }
        directory.atomic_write(Path::new(&name), &bytes)?;
    }
    Ok(directory)
}
