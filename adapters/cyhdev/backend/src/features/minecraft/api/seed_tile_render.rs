//! Bound image compression independently of the already completed tile service call.

use super::{seed_tile_dto::MinecraftSeedTile, seed_tile_png};
use crate::features::minecraft::error::MapError;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

/// Four admitted jobs cap queued blocking work, even after HTTP cancellation.
pub async fn png(tile: MinecraftSeedTile) -> Result<Vec<u8>, MapError> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let slots = SLOTS.get_or_init(|| Arc::new(Semaphore::new(4)));
    let permit = Arc::clone(slots)
        .try_acquire_owned()
        .map_err(|_| MapError::Busy)?;
    tokio::task::spawn_blocking(move || {
        // The CPU job retains admission when a disconnected client drops its waiter.
        let _permit = permit;
        seed_tile_png::encode(&tile)
    })
    .await
    .map_err(|_| MapError::Unavailable)?
}
