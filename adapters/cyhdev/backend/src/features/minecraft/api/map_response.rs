//! Terrain response contracts, separate from service and plugin representations.

use crate::features::minecraft::domain::map_data as domain;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftMapKind {
    Catalog,
    Area,
    Blocks,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftMapData {
    pub kind: MinecraftMapKind,
    pub world: Option<String>,
    pub sampled_at_ms: i64,
    pub scanned_chunks: u16,
    pub missing_chunks: u16,
    pub truncated: bool,
    pub worlds: Vec<MinecraftMapWorld>,
    pub blocks: Vec<String>,
    pub cells: Vec<MinecraftMapCell>,
    pub structures: Vec<MinecraftMapStructure>,
    pub matches: Vec<MinecraftMapMatch>,
}
#[derive(Serialize, ToSchema)]
pub struct MinecraftMapWorld {
    pub id: String,
    pub name: String,
    pub map_id: String,
    pub min_y: i32,
    pub max_y: i32,
}
#[derive(Serialize, ToSchema)]
pub struct MinecraftMapCell {
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub biome: String,
}
#[derive(Serialize, ToSchema)]
pub struct MinecraftMapStructure {
    pub kind: String,
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}
#[derive(Serialize, ToSchema)]
pub struct MinecraftMapMatch {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<domain::MapData> for MinecraftMapData {
    fn from(value: domain::MapData) -> Self {
        Self {
            kind: match value.kind {
                domain::MapKind::Catalog => MinecraftMapKind::Catalog,
                domain::MapKind::Area => MinecraftMapKind::Area,
                domain::MapKind::Blocks => MinecraftMapKind::Blocks,
            },
            world: value.world,
            sampled_at_ms: value.sampled_at_ms,
            scanned_chunks: value.scanned_chunks,
            missing_chunks: value.missing_chunks,
            truncated: value.truncated,
            worlds: value
                .worlds
                .into_iter()
                .map(|v| MinecraftMapWorld {
                    id: v.id,
                    name: v.name,
                    map_id: v.map_id,
                    min_y: v.min_y,
                    max_y: v.max_y,
                })
                .collect(),
            blocks: value.blocks,
            cells: value
                .cells
                .into_iter()
                .map(|v| MinecraftMapCell {
                    x: v.x,
                    y: v.y,
                    z: v.z,
                    biome: v.biome,
                })
                .collect(),
            structures: value
                .structures
                .into_iter()
                .map(|v| MinecraftMapStructure {
                    kind: v.kind,
                    min_x: v.min_x,
                    min_y: v.min_y,
                    min_z: v.min_z,
                    max_x: v.max_x,
                    max_y: v.max_y,
                    max_z: v.max_z,
                })
                .collect(),
            matches: value
                .matches
                .into_iter()
                .map(|v| MinecraftMapMatch {
                    x: v.x,
                    y: v.y,
                    z: v.z,
                })
                .collect(),
        }
    }
}
