//! RAM biome cache with byte reservations; coverage remains a fresh Paper query.

use super::prediction_wire::Context;
use crate::features::minecraft::domain::prediction::{PredictedBiome, PredictionQuery};
use scc::HashCache;
use std::{collections::BTreeSet, mem::size_of, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[cfg(test)]
use super::prediction_budget::MAX_BYTES;
const MAX_ENTRIES: usize = 65_536;
const ENTRY_OVERHEAD: usize = 1024;
const MAX_RECLAIM: usize = 64;

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct Key {
    world_id: uuid::Uuid,
    revision: String,
    seed: i64,
    large_biomes: bool,
    chunk_x: i32,
    chunk_z: i32,
    y: i32,
}

impl Key {
    /// The private revision changes on plugin restart; seed, preset and world resets cannot alias.
    pub fn new(context: &Context, query: &PredictionQuery) -> Self {
        Self {
            world_id: context.world_id,
            revision: context.profile_revision.clone(),
            seed: context.seed,
            large_biomes: context.preset == super::prediction_wire::Preset::LargeBiomes,
            chunk_x: query.region().chunk_x,
            chunk_z: query.region().chunk_z,
            y: query.y,
        }
    }
}

struct Entry {
    palette: Box<[Box<str>]>,
    indices: Box<[u16]>,
    _reservation: OwnedSemaphorePermit,
}

pub(super) struct PredictionCache {
    entries: HashCache<Key, Entry>,
    budget: Arc<Semaphore>,
}

impl PredictionCache {
    pub fn new() -> Self {
        Self {
            entries: HashCache::with_capacity(0, MAX_ENTRIES),
            budget: super::prediction_budget::shared(),
        }
    }

    #[cfg(test)]
    fn with_budget(bytes: usize) -> Self {
        Self {
            entries: HashCache::with_capacity(0, MAX_ENTRIES),
            budget: Arc::new(Semaphore::new(bytes)),
        }
    }

    /// A bounded response copy permits immediate cache reclamation after the lookup.
    pub async fn get(&self, key: &Key) -> Option<Vec<PredictedBiome>> {
        self.entries.get_async(key).await.map(|entry| {
            entry
                .get()
                .indices
                .iter()
                .enumerate()
                .map(|(index, palette)| PredictedBiome {
                    x: key.chunk_x * 16 + (index % 32) as i32 * 4,
                    z: key.chunk_z * 16 + (index / 32) as i32 * 4,
                    biome: entry.get().palette[usize::from(*palette)].to_string(),
                })
                .collect()
        })
    }

    pub async fn insert(&self, key: Key, cells: &[PredictedBiome]) -> bool {
        // Cache only a complete validated tile. Visibility masks must never persist here.
        if cells.len() != 1024 {
            return false;
        }
        let palette: Vec<&str> = cells
            .iter()
            .map(|cell| cell.biome.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let cost = Self::cost(&key, &palette);
        let Ok(cost) = u32::try_from(cost) else {
            return false;
        };
        let mut reservation = Arc::clone(&self.budget).try_acquire_many_owned(cost).ok();
        if reservation.is_none() {
            let mut reclaimed = 0;
            self.entries
                .iter_mut_async(|entry| {
                    drop(entry.consume());
                    reclaimed += 1;
                    reservation = Arc::clone(&self.budget).try_acquire_many_owned(cost).ok();
                    reservation.is_none() && reclaimed < MAX_RECLAIM
                })
                .await;
        }
        let Some(reservation) = reservation else {
            return false;
        };
        // Reserve before allocating retained data. Palette strings remove 1,024 small allocations per tile.
        let mut indices = vec![0u16; 1024].into_boxed_slice();
        let mut assigned = [false; 1024];
        for cell in cells {
            let x = i64::from(cell.x) - i64::from(key.chunk_x) * 16;
            let z = i64::from(cell.z) - i64::from(key.chunk_z) * 16;
            if !(0..128).contains(&x) || !(0..128).contains(&z) || x % 4 != 0 || z % 4 != 0 {
                return false;
            }
            let Ok(palette_index) = palette.binary_search(&cell.biome.as_str()) else {
                return false;
            };
            let index = (z / 4 * 32 + x / 4) as usize;
            if assigned[index] {
                return false;
            }
            assigned[index] = true;
            indices[index] = palette_index as u16;
        }
        let palette = palette
            .into_iter()
            .map(Box::<str>::from)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        match self
            .entries
            .put_async(
                key,
                Entry {
                    palette,
                    indices,
                    _reservation: reservation,
                },
            )
            .await
        {
            Ok(evicted) => {
                drop(evicted);
                true
            }
            Err(rejected) => {
                drop(rejected);
                false
            }
        }
    }

    fn cost(key: &Key, palette: &[&str]) -> usize {
        ENTRY_OVERHEAD
            + key.revision.capacity()
            + 1024 * size_of::<u16>()
            + palette.len() * size_of::<Box<str>>()
            // Each small allocation includes conservative allocator rounding/header headroom.
            + palette.iter().map(|biome| biome.len() + 64).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(index: i32) -> Key {
        Key {
            world_id: uuid::Uuid::nil(),
            revision: "a".repeat(64),
            seed: 1,
            large_biomes: false,
            chunk_x: index,
            chunk_z: 0,
            y: 64,
        }
    }
    fn cells() -> Vec<PredictedBiome> {
        (0..1024)
            .map(|index| PredictedBiome {
                x: (index % 32) * 4,
                z: (index / 32) * 4,
                biome: "minecraft:plains".into(),
            })
            .collect()
    }

    #[tokio::test]
    async fn budget_reclaims_old_tiles_and_charges_string_capacity() {
        let data = cells();
        let cost = PredictionCache::cost(&key(0), &["minecraft:plains"]);
        let cache = PredictionCache::with_budget(cost + 1);
        assert!(cache.insert(key(0), &data).await);
        assert!(cache.get(&key(0)).await.is_some());
        let mut another = key(0);
        another.y = 65;
        assert!(cache.insert(another.clone(), &data).await);
        assert!(cache.get(&key(0)).await.is_none());
        assert!(cache.get(&another).await.is_some());
        assert!(cache.budget.available_permits() <= 1);
        let too_small = PredictionCache::with_budget(cost - 1);
        assert!(!too_small.insert(key(0), &data).await);
        assert!(too_small.get(&key(0)).await.is_none());
        assert_eq!(MAX_BYTES, 536_870_912);
    }

    #[tokio::test]
    async fn packed_palette_preserves_shuffled_coordinates_and_biomes() -> anyhow::Result<()> {
        let cache = PredictionCache::new();
        let mut key = key(-1);
        key.chunk_z = -2;
        let mut data = cells();
        for cell in &mut data {
            cell.x -= 16;
            cell.z -= 32;
            if cell.x.rem_euclid(8) == 0 {
                cell.biome = "minecraft:river".into();
            }
        }
        data.reverse();
        assert!(cache.insert(key.clone(), &data).await);
        let cached = cache
            .get(&key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Missing cached tile"))?;
        assert_eq!(cached.len(), data.len());
        for (actual, expected) in cached.iter().zip(data.iter().rev()) {
            assert_eq!(
                (actual.x, actual.z, &actual.biome),
                (expected.x, expected.z, &expected.biome)
            );
        }
        data[0] = data[1].clone();
        let mut duplicate = key;
        duplicate.y = 70;
        assert!(!cache.insert(duplicate, &data).await);
        Ok(())
    }

    #[tokio::test]
    async fn seed_preset_epoch_slice_and_coordinates_are_independent() {
        let cache = PredictionCache::new();
        assert!(cache.insert(key(0), &cells()).await);
        let changes: [fn(&mut Key); 6] = [
            |k| k.seed = 2,
            |k| k.large_biomes = true,
            |k| k.world_id = uuid::Uuid::new_v4(),
            |k| k.revision = "b".repeat(64),
            |k| k.y = 65,
            |k| k.chunk_z = 1,
        ];
        for change in changes {
            let mut other = key(0);
            change(&mut other);
            assert!(cache.get(&other).await.is_none());
        }
        assert!(!cache.insert(key(2), &[]).await);
    }
}
