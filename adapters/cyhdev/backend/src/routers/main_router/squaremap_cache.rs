//! Concurrent tile storage with reservations that outlive eviction while responses use the bytes.

use std::{fs::Metadata, io, os::unix::fs::MetadataExt, path::PathBuf, sync::Arc};

use axum::body::Bytes;
use scc::HashCache;
use tokio::{
    io::AsyncReadExt,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
};

const CACHE_BUDGET: usize = 4 * 1024 * 1024 * 1024;
// Bound the index independently and leave headroom for its buckets and resizing.
const INDEX_RESERVE: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 65_536;
const ENTRY_OVERHEAD: usize = 1024;
const MAX_TILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RECLAIM: usize = 256;

/// Metadata identity detects atomic replacement and same-size, subsecond rewrites.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Version {
    device: u64,
    inode: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Version {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    /// Metadata is a weak validator; it does not claim a content hash.
    pub(super) fn etag(self) -> String {
        format!(
            "W/\"{:x}-{:x}-{:x}-{:x}-{:x}-{:x}-{:x}\"",
            self.device,
            self.inode,
            self.length,
            self.modified.0,
            self.modified.1,
            self.changed.0,
            self.changed.1
        )
    }
}

/// A byte reservation remains held until both the cache and every HTTP body release this tile.
pub(super) struct Tile {
    pub(super) version: Version,
    data: Box<[u8]>,
    _reservation: OwnedSemaphorePermit,
}

struct TileBody(Arc<Tile>);

impl AsRef<[u8]> for TileBody {
    fn as_ref(&self) -> &[u8] {
        &self.0.data
    }
}

impl Tile {
    /// Share the allocation with HTTP without copying or losing reservation ownership.
    pub(super) fn body(self: Arc<Self>) -> Bytes {
        Bytes::from_owner(TileBody(self))
    }

    pub(super) fn len(&self) -> usize {
        self.data.len()
    }
}

/// Async scc access provides bucket-local LRU; the semaphore supplies its missing byte bound.
pub(super) struct TileCache {
    root: PathBuf,
    entries: HashCache<String, Arc<Tile>>,
    budget: Arc<Semaphore>,
    fills: Semaphore,
    reclaim: Mutex<()>,
}

impl TileCache {
    pub(super) fn new(root: PathBuf) -> Self {
        Self::with_budget(root, CACHE_BUDGET - INDEX_RESERVE)
    }

    fn with_budget(root: PathBuf, bytes: usize) -> Self {
        Self {
            root,
            entries: HashCache::with_capacity(0, MAX_ENTRIES),
            budget: Arc::new(Semaphore::new(bytes)),
            fills: Semaphore::new(16),
            reclaim: Mutex::new(()),
        }
    }

    /// Only canonical relative PNG tile paths are eligible; ServeDir handles all other requests.
    pub(super) fn eligible(path: &str) -> bool {
        path.len() <= 512
            && path.starts_with("tiles/")
            && path.ends_with(".png")
            && path.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
            })
    }

    /// Check disk metadata on every request; never return a cached deletion or old file version.
    pub(super) async fn get(&self, key: &str) -> io::Result<Option<Arc<Tile>>> {
        if !Self::eligible(key) {
            return Ok(None);
        }
        let path = self.root.join(key);
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                drop(self.entries.remove_async(key).await);
                return Err(error);
            }
        };
        let version = Version::from_metadata(&metadata);
        if let Some(entry) = self.entries.get_async(key).await {
            if entry.get().version == version && metadata.is_file() {
                return Ok(Some(Arc::clone(entry.get())));
            }
            drop(entry.remove());
        }
        if !metadata.is_file() || metadata.len() > MAX_TILE_BYTES {
            return Ok(None);
        }
        let _fill = match self.fills.try_acquire() {
            Ok(permit) => permit,
            Err(_) => return Ok(None),
        };
        let mut file = tokio::fs::File::open(path).await?;
        if Version::from_metadata(&file.metadata().await?) != version {
            return Ok(None);
        }
        let size = metadata.len() as usize;
        let cost = (size + ENTRY_OVERHEAD + key.len() * 2) as u32;
        let reservation = match self.reserve(cost).await {
            Some(reservation) => reservation,
            None => return Ok(None),
        };
        let mut data = vec![0; size].into_boxed_slice();
        file.read_exact(&mut data).await?;
        if Version::from_metadata(&file.metadata().await?) != version {
            return Ok(None);
        }
        drop(file);
        let tile = Arc::new(Tile {
            version,
            data,
            _reservation: reservation,
        });
        // A concurrent fill may have won; drop its rejected candidate or any evicted value now.
        match self
            .entries
            .put_async(key.to_owned(), Arc::clone(&tile))
            .await
        {
            Ok(evicted) => drop(evicted),
            Err(rejected) => drop(rejected),
        }
        Ok(Some(tile))
    }

    /// Never wait for clients to release memory. Reclaim bounded work, then bypass the cache.
    async fn reserve(&self, cost: u32) -> Option<OwnedSemaphorePermit> {
        if let Ok(permit) = Arc::clone(&self.budget).try_acquire_many_owned(cost) {
            return Some(permit);
        }
        let _reclaim = match self.reclaim.try_lock() {
            Ok(guard) => guard,
            Err(_) => return None,
        };
        let mut reservation = None;
        let mut removed = 0;
        self.entries
            .iter_mut_async(|entry| {
                drop(entry.consume());
                removed += 1;
                reservation = Arc::clone(&self.budget).try_acquire_many_owned(cost).ok();
                reservation.is_none() && removed < MAX_RECLAIM
            })
            .await;
        reservation
    }
}

#[cfg(test)]
#[path = "squaremap_cache_tests.rs"]
mod tests;
