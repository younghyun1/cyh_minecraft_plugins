//! Palette tiles share the process seed-cache byte reservation with legacy previews.

use super::{prediction_budget, seed_profile::Profile};
use crate::features::minecraft::domain::seed_tile::{CELLS, SeedTileQuery};
use scc::HashCache;
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    world: uuid::Uuid,
    revision: String,
    seed: i64,
    preset: super::seed_profile::Preset,
    pub query: SeedTileQuery,
}

impl Key {
    pub fn new(profile: &Profile, query: &SeedTileQuery) -> Self {
        Self {
            world: profile.world_id,
            revision: profile.profile_revision.clone(),
            seed: profile.seed,
            preset: profile.preset,
            query: query.clone(),
        }
    }
}

pub(super) struct Tile {
    pub palette: Vec<String>,
    pub indices: Vec<Option<u16>>,
}

struct Entry {
    tile: Arc<Tile>,
    _permit: OwnedSemaphorePermit,
}

pub(super) struct TileCache {
    entries: HashCache<Key, Entry>,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            entries: HashCache::with_capacity(0, 16_384),
        }
    }
    pub async fn get(&self, key: &Key) -> Option<Arc<Tile>> {
        self.entries
            .get_async(key)
            .await
            .map(|entry| Arc::clone(&entry.get().tile))
    }
    pub async fn insert(&self, key: Key, tile: Arc<Tile>) {
        if tile.indices.len() != CELLS || tile.palette.len() > 256 {
            return;
        }
        let bytes = 2048
            + key.revision.capacity()
            + key.query.world.capacity()
            + tile.indices.capacity() * size_of::<Option<u16>>()
            + tile.palette.capacity() * size_of::<String>()
            + tile
                .palette
                .iter()
                .map(|s| s.capacity() + 64)
                .sum::<usize>();
        let Ok(bytes) = u32::try_from(bytes) else {
            return;
        };
        let budget = prediction_budget::shared();
        let mut permit = Arc::clone(&budget).try_acquire_many_owned(bytes).ok();
        if permit.is_none() {
            let mut removed = 0;
            self.entries
                .iter_mut_async(|entry| {
                    drop(entry.consume());
                    removed += 1;
                    permit = Arc::clone(&budget).try_acquire_many_owned(bytes).ok();
                    permit.is_none() && removed < 64
                })
                .await;
        }
        if let Some(permit) = permit {
            // Uncached in-flight tiles are independently bounded by request and worker admission.
            let _ = self
                .entries
                .put_async(
                    key,
                    Entry {
                        tile,
                        _permit: permit,
                    },
                )
                .await;
        }
    }
}
