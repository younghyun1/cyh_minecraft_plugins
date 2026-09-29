//! Public prediction contracts deliberately contain no generator seed or private profile digest.

use crate::features::minecraft::domain::prediction as domain;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MinecraftPredictionQuery {
    pub world: String,
    pub min_x: i32,
    pub min_z: i32,
    pub y: i32,
}

impl From<MinecraftPredictionQuery> for domain::PredictionQuery {
    fn from(value: MinecraftPredictionQuery) -> Self {
        Self {
            world: value.world,
            min_x: value.min_x,
            min_z: value.min_z,
            y: value.y,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftPredictionPreset {
    Default,
    LargeBiomes,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftPredictionCoverageState {
    Generated,
    Ungenerated,
    Unknown,
    Excluded,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftPredictedBiome {
    pub x: i32,
    pub z: i32,
    pub biome: String,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftPredictionCoverage {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub state: MinecraftPredictionCoverageState,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftPrediction {
    pub world: String,
    pub sampled_at_ms: i64,
    pub expires_at_ms: i64,
    pub generator_revision: String,
    pub preset: MinecraftPredictionPreset,
    pub y: i32,
    pub step: u8,
    pub min_x: i32,
    pub min_z: i32,
    pub cells: Vec<MinecraftPredictedBiome>,
    pub coverage: Vec<MinecraftPredictionCoverage>,
}

impl From<domain::Prediction> for MinecraftPrediction {
    fn from(value: domain::Prediction) -> Self {
        Self {
            world: value.world,
            sampled_at_ms: value.sampled_at_ms,
            expires_at_ms: value.expires_at_ms,
            generator_revision: domain::GENERATOR_REVISION.to_owned(),
            preset: match value.preset {
                domain::PredictionPreset::Default => MinecraftPredictionPreset::Default,
                domain::PredictionPreset::LargeBiomes => MinecraftPredictionPreset::LargeBiomes,
            },
            y: value.y,
            step: 4,
            min_x: value.min_x,
            min_z: value.min_z,
            cells: value
                .cells
                .into_iter()
                .map(|cell| MinecraftPredictedBiome {
                    x: cell.x,
                    z: cell.z,
                    biome: cell.biome,
                })
                .collect(),
            coverage: value
                .coverage
                .into_iter()
                .map(|cell| MinecraftPredictionCoverage {
                    chunk_x: cell.chunk_x,
                    chunk_z: cell.chunk_z,
                    state: match cell.state {
                        domain::CoverageState::Generated => {
                            MinecraftPredictionCoverageState::Generated
                        }
                        domain::CoverageState::Ungenerated => {
                            MinecraftPredictionCoverageState::Ungenerated
                        }
                        domain::CoverageState::Unknown => MinecraftPredictionCoverageState::Unknown,
                        domain::CoverageState::Excluded => {
                            MinecraftPredictionCoverageState::Excluded
                        }
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_cannot_override_seed_or_preset() {
        for field in ["seed", "preset", "large_biomes"] {
            let mut value =
                serde_json::json!({"world":"minecraft:overworld","min_x":0,"min_z":0,"y":64});
            value[field] = serde_json::json!(1);
            assert!(serde_json::from_value::<MinecraftPredictionQuery>(value).is_err());
        }
    }
}
