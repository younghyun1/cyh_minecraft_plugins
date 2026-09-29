//! Bounded seed predictions, separate from observations of saved terrain.

use super::map_query::{Region, identifier};

pub const GENERATOR_REVISION: &str = minecraft_seed::GENERATOR_REVISION;
pub const PREDICTION_TTL_MS: i64 = 15_000;

pub struct PredictionQuery {
    pub world: String,
    pub min_x: i32,
    pub min_z: i32,
    pub y: i32,
}

impl PredictionQuery {
    /// Align toward negative infinity, including coordinates west/north of zero.
    pub fn region(&self) -> Region {
        Region {
            chunk_x: self.min_x.div_euclid(16),
            chunk_z: self.min_z.div_euclid(16),
            width: 8,
            height: 8,
        }
    }

    pub fn valid(&self) -> bool {
        identifier(&self.world) && self.region().valid(8) && (-64..=319).contains(&self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionPreset {
    Default,
    LargeBiomes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageState {
    Generated,
    Ungenerated,
    Unknown,
    Excluded,
}

pub struct PredictionCoverage {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub state: CoverageState,
}

#[derive(Clone)]
pub struct PredictedBiome {
    pub x: i32,
    pub z: i32,
    pub biome: String,
}

pub struct Prediction {
    pub world: String,
    pub sampled_at_ms: i64,
    pub expires_at_ms: i64,
    pub preset: PredictionPreset,
    pub y: i32,
    pub min_x: i32,
    pub min_z: i32,
    pub cells: Vec<PredictedBiome>,
    pub coverage: Vec<PredictionCoverage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_origins_align_and_complete_footprints_are_bounded() {
        let mut query = PredictionQuery {
            world: "minecraft:overworld".into(),
            min_x: -1,
            min_z: -17,
            y: -64,
        };
        assert!(query.valid());
        assert_eq!(query.region().chunk_x, -1);
        assert_eq!(query.region().chunk_z, -2);
        query.min_x = 29_999_999;
        assert!(!query.valid());
        query.min_x = i32::MIN;
        assert!(!query.valid());
        query.min_x = 0;
        query.y = 320;
        assert!(!query.valid());
    }
}
