//! Coverage-checked biome preview orchestration over the existing world-query reservation.

use super::{
    prediction_process,
    prediction_wire::{self, State},
    world_query::{WorldQueryService, exchange},
};
use crate::features::minecraft::{
    domain::prediction::{PREDICTION_TTL_MS, PredictedBiome, Prediction, PredictionQuery},
    error::MapError,
};
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::Instant;

impl WorldQueryService {
    /// Serialize prediction and observation work; cancellation retains the full reservation.
    pub async fn predict(&self, query: PredictionQuery) -> Result<Prediction, MapError> {
        if !query.valid() {
            return Err(MapError::Invalid);
        }
        let path = self.path.as_ref().ok_or(MapError::Disabled)?;
        let executable = self.predictor.as_ref().ok_or(MapError::Disabled)?;
        let mut gate = self.gate.try_lock().map_err(|_| MapError::Busy)?;
        if Instant::now() < *gate {
            return Err(MapError::Busy);
        }
        *gate = Instant::now() + Duration::from_secs(21);
        let result = tokio::time::timeout(Duration::from_secs(20), async {
            let request = prediction_wire::request(&query)?;
            let before =
                prediction_wire::parse(&exchange(path, &request).await?, &query, now_ms()?)?;
            let cells = if before
                .coverage
                .iter()
                .any(|cell| cell.state == State::Ungenerated)
            {
                let key = super::prediction_cache::Key::new(&before, &query);
                match self.prediction_cache.get(&key).await {
                    Some(cells) => {
                        tracing::debug!(cache_hit = true, "Minecraft prediction cache lookup");
                        cells
                    }
                    None => {
                        let cells = prediction_process::run(executable, &before, &query).await?;
                        let admitted = self.prediction_cache.insert(key, &cells).await;
                        tracing::debug!(
                            cache_hit = false,
                            admitted,
                            "Minecraft prediction cache lookup"
                        );
                        cells
                    }
                }
            } else {
                Vec::new()
            };
            // Recheck cached results too; geographic coverage is never a property of a cached biome.
            let after =
                prediction_wire::parse(&exchange(path, &request).await?, &query, now_ms()?)?;
            anyhow::ensure!(
                before.same_profile(&after) && after.sampled_at_ms >= before.sampled_at_ms,
                "Prediction profile changed"
            );
            let cells = filter_cells(cells, &before, &after);
            let region = query.region();
            Ok::<_, anyhow::Error>(Prediction {
                world: query.world,
                sampled_at_ms: after.sampled_at_ms,
                expires_at_ms: after.sampled_at_ms + PREDICTION_TTL_MS,
                preset: after.public_preset(),
                y: query.y,
                min_x: region.chunk_x * 16,
                min_z: region.chunk_z * 16,
                cells,
                coverage: after.public_coverage(),
            })
        })
        .await;
        *gate = Instant::now() + Duration::from_secs(1);
        match result {
            Ok(Ok(data)) => Ok(data),
            _ => {
                // Private wire errors may contain seed-bearing JSON fragments; never log their payloads.
                tracing::warn!("Minecraft prediction unavailable or unsupported");
                Err(MapError::Unavailable)
            }
        }
    }
}

fn now_ms() -> anyhow::Result<i64> {
    Ok(i64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn filter_cells(
    cells: Vec<PredictedBiome>,
    before: &prediction_wire::Context,
    after: &prediction_wire::Context,
) -> Vec<PredictedBiome> {
    let permitted = |context: &prediction_wire::Context| {
        context
            .coverage
            .iter()
            .filter(|cell| cell.state == State::Ungenerated)
            .map(|cell| (cell.chunk_x, cell.chunk_z))
            .collect::<BTreeSet<_>>()
    };
    let earlier = permitted(before);
    let latest = permitted(after);
    cells
        .into_iter()
        .filter(|cell| {
            let chunk = (cell.x.div_euclid(16), cell.z.div_euclid(16));
            earlier.contains(&chunk) && latest.contains(&chunk)
        })
        .collect()
}

#[cfg(test)]
#[path = "prediction_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prediction_integration_tests.rs"]
mod integration_tests;
