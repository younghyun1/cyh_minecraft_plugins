//! Stable, zoom-dependent addressing for bounded biome tiles.

pub const SIDE: usize = 64;
pub const CELLS: usize = SIDE * SIDE;
pub const WORLD_EDGE: i64 = 30_000_000;

pub struct SeedTile {
    pub query: SeedTileQuery,
    pub min_x: i32,
    pub min_z: i32,
    pub step: u32,
    pub preset: SeedPreset,
    pub profile_epoch: String,
    pub sampled_at_ms: i64,
    pub expires_at_ms: i64,
    pub palette: Vec<String>,
    pub indices: Vec<Option<u16>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedPreset {
    Default,
    LargeBiomes,
    Nether,
    End,
}

/// Fixed slots bound retained generator profiles independently of caller input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedDimension {
    Overworld,
    Nether,
    End,
}

impl SeedDimension {
    pub fn from_world(world: &str) -> Option<Self> {
        match world {
            "minecraft:overworld" => Some(Self::Overworld),
            "minecraft:the_nether" => Some(Self::Nether),
            "minecraft:the_end" => Some(Self::End),
            _ => None,
        }
    }
    pub const fn world(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::Nether => "minecraft:the_nether",
            Self::End => "minecraft:the_end",
        }
    }
    pub const fn index(self) -> usize {
        match self {
            Self::Overworld => 0,
            Self::Nether => 1,
            Self::End => 2,
        }
    }
    pub fn contains_y(self, y: i32) -> bool {
        match self {
            Self::Overworld => (-64..=319).contains(&y),
            Self::Nether | Self::End => (0..=255).contains(&y),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SeedTileQuery {
    pub world: String,
    pub tile_x: i32,
    pub tile_z: i32,
    pub level: u8,
    pub y: i32,
}

impl SeedTileQuery {
    /// Keep coordinate arithmetic widened until the complete tile is validated.
    pub fn geometry(&self) -> Option<(i32, i32, u32)> {
        let dimension = SeedDimension::from_world(&self.world)?;
        if self.level > 12 || !dimension.contains_y(self.y) {
            return None;
        }
        let step = 4_u32 << self.level;
        let span = i64::from(step) * SIDE as i64;
        let x = i64::from(self.tile_x) * span;
        let z = i64::from(self.tile_z) * span;
        if [x, z]
            .into_iter()
            .any(|origin| origin >= WORLD_EDGE || origin + span <= -WORLD_EDGE)
        {
            return None;
        }
        Some((i32::try_from(x).ok()?, i32::try_from(z).ok()?, step))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_tiles_and_zoom_levels_have_stable_origins() {
        let mut query = SeedTileQuery {
            world: "minecraft:overworld".into(),
            tile_x: -1,
            tile_z: 1,
            level: 0,
            y: 64,
        };
        assert_eq!(query.geometry(), Some((-256, 256, 4)));
        query.level = 12;
        assert_eq!(query.geometry(), Some((-1_048_576, 1_048_576, 16_384)));
        query.level = 13;
        assert!(query.geometry().is_none());
        query.level = 0;
        query.tile_x = i32::MAX;
        assert!(query.geometry().is_none());
        query.tile_x = 117187;
        assert!(query.geometry().is_some());
    }
}
