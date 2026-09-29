//! Public tile metadata contains no private world seed.

use serde::{Deserialize, Serialize};

/// Effective supported vanilla generator preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftSeedPreset {
    Default,
    LargeBiomes,
    Nether,
    End,
}

/// A fixed 64 by 64 grid with null cells excluded by authoritative visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSeedTile {
    pub world: String,
    pub tile_x: i32,
    pub tile_z: i32,
    pub level: u8,
    pub min_x: i32,
    pub min_z: i32,
    pub y: i32,
    pub step: u32,
    pub width: u8,
    pub height: u8,
    pub preset: MinecraftSeedPreset,
    pub generator_revision: String,
    pub profile_epoch: String,
    pub sampled_at_ms: i64,
    pub expires_at_ms: i64,
    pub palette: Vec<String>,
    pub indices: Vec<Option<u16>>,
}
