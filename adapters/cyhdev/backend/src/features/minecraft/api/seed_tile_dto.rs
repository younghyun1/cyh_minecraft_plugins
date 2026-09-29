//! Public tile data contains biome IDs and permission masks, never seed material.

use crate::features::minecraft::domain::seed_tile::{SeedPreset, SeedTileQuery};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftSeedPreset {
    Default,
    LargeBiomes,
    Nether,
    End,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MinecraftSeedTileQuery {
    pub world: String,
    pub tile_x: i32,
    pub tile_z: i32,
    pub level: u8,
    pub y: i32,
}

impl From<MinecraftSeedTileQuery> for SeedTileQuery {
    fn from(value: MinecraftSeedTileQuery) -> Self {
        Self {
            world: value.world,
            tile_x: value.tile_x,
            tile_z: value.tile_z,
            level: value.level,
            y: value.y,
        }
    }
}

#[derive(Serialize, ToSchema)]
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

impl From<crate::features::minecraft::domain::seed_tile::SeedTile> for MinecraftSeedTile {
    fn from(tile: crate::features::minecraft::domain::seed_tile::SeedTile) -> Self {
        use crate::features::minecraft::domain::prediction::GENERATOR_REVISION;
        Self {
            world: tile.query.world,
            tile_x: tile.query.tile_x,
            tile_z: tile.query.tile_z,
            level: tile.query.level,
            y: tile.query.y,
            min_x: tile.min_x,
            min_z: tile.min_z,
            step: tile.step,
            width: 64,
            height: 64,
            preset: match tile.preset {
                SeedPreset::Default => MinecraftSeedPreset::Default,
                SeedPreset::LargeBiomes => MinecraftSeedPreset::LargeBiomes,
                SeedPreset::Nether => MinecraftSeedPreset::Nether,
                SeedPreset::End => MinecraftSeedPreset::End,
            },
            generator_revision: GENERATOR_REVISION.to_owned(),
            profile_epoch: tile.profile_epoch,
            sampled_at_ms: tile.sampled_at_ms,
            expires_at_ms: tile.expires_at_ms,
            palette: tile.palette,
            indices: tile.indices,
        }
    }
}
