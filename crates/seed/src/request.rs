//! Request validation and the bounded prediction wire types.

use serde::{Deserialize, Serialize};

use crate::Error;

const WORLD_BORDER: i64 = 30_000_000;
const MAX_POINTS: u32 = 4096;

/// Supported vanilla dimensions; world identity remains the backend's responsibility.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Vanilla Overworld, including the large-biomes preset.
    #[default]
    Overworld,
    /// Vanilla Nether with its legacy-seeded temperature and vegetation noise.
    Nether,
    /// Vanilla End central and outer-island biome distribution.
    End,
}

/// One fixed-height dimension grid; seed input stays inside the worker pipe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionRequest {
    /// Signed Minecraft seed, preserving all 64 bits.
    pub seed: i64,
    /// Defaults to the Overworld for compatibility with existing pipe requests.
    #[serde(default)]
    pub dimension: Dimension,
    /// Selects vanilla's distinct large-biomes noise router.
    pub large_biomes: bool,
    /// Block height: -64..=319 in the Overworld, 0..=255 elsewhere.
    pub y: i32,
    /// First sample's block X coordinate.
    pub min_x: i32,
    /// First sample's block Z coordinate.
    pub min_z: i32,
    /// Samples along X.
    pub width: u32,
    /// Samples along Z.
    pub height: u32,
    /// Positive distance between samples, in blocks.
    pub step: u32,
}

impl PredictionRequest {
    /// Rejects oversized grids and overflow before allocating or initializing noise.
    pub(crate) fn validate(&self) -> Result<(), Error> {
        let count = self.width.checked_mul(self.height).ok_or(Error::Bounds)?;
        let min_y = match self.dimension {
            Dimension::Overworld => -64,
            Dimension::Nether | Dimension::End => 0,
        };
        let max_y = match self.dimension {
            Dimension::Overworld => 319,
            Dimension::Nether | Dimension::End => 255,
        };
        if !(1..=MAX_POINTS).contains(&count)
            || self.step == 0
            || !(min_y..=max_y).contains(&self.y)
            || (self.dimension != Dimension::Overworld && self.large_biomes)
        {
            return Err(Error::Bounds);
        }
        for (origin, size) in [(self.min_x, self.width), (self.min_z, self.height)] {
            let start = i64::from(origin);
            let end = start + i64::from(size - 1) * i64::from(self.step);
            if !(-WORLD_BORDER..=WORLD_BORDER).contains(&start)
                || !(-WORLD_BORDER..=WORLD_BORDER).contains(&end)
            {
                return Err(Error::Bounds);
            }
        }
        Ok(())
    }
}

/// Raw quart-biome sample at the requested block coordinate, without Voronoi blending.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// Requested block X coordinate.
    pub x: i32,
    /// Requested block Z coordinate.
    pub z: i32,
    /// Namespaced biome identifier.
    pub biome: String,
}

/// Cells ordered by Z row, then X column.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionResponse {
    /// Pinned engine version, checked by the supervising backend.
    pub generator_revision: String,
    /// Preset used for the returned grid.
    pub large_biomes: bool,
    /// No more than 4096 cells.
    pub cells: Vec<Cell>,
}

#[cfg(test)]
mod tests {
    use super::{Dimension, PredictionRequest};

    /// A small valid request for exercising independent bounds.
    fn request() -> PredictionRequest {
        PredictionRequest {
            seed: 1,
            dimension: Dimension::Overworld,
            large_biomes: false,
            y: 64,
            min_x: 0,
            min_z: 0,
            width: 32,
            height: 32,
            step: 4,
        }
    }

    /// Count and coordinate arithmetic must reject overflow rather than wrap.
    #[test]
    fn rejects_oversized_or_overflowing_grids() {
        let mut input = request();
        assert!(input.validate().is_ok());
        input.width = 129;
        assert!(input.validate().is_err());
        input.width = u32::MAX;
        input.height = u32::MAX;
        assert!(input.validate().is_err());
        input.width = 2;
        input.height = 1;
        input.step = u32::MAX;
        assert!(input.validate().is_err());
    }

    /// Every requested sample, including the last row, must remain in the world.
    #[test]
    fn validates_last_sample_and_vertical_limits() {
        let mut input = request();
        input.min_z = 30_000_000;
        assert!(input.validate().is_err());
        input.height = 1;
        assert!(input.validate().is_ok());
        input.y = 320;
        assert!(input.validate().is_err());
        input.y = -65;
        assert!(input.validate().is_err());
        input.y = -64;
        input.width = 0;
        assert!(input.validate().is_err());
        input.width = 1;
        input.step = 0;
        assert!(input.validate().is_err());
    }

    #[test]
    fn dimension_bounds_and_preset_are_checked() {
        for dimension in [Dimension::Nether, Dimension::End] {
            let mut input = request();
            input.dimension = dimension;
            for y in [0, 255] {
                input.y = y;
                assert!(input.validate().is_ok());
            }
            for y in [-64, -1, 256, 319] {
                input.y = y;
                assert!(input.validate().is_err());
            }
            input.y = 64;
            input.large_biomes = true;
            assert!(input.validate().is_err());
        }
    }
}
