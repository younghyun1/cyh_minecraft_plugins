//! Private JSON representation of the Paper socket protocol.

use crate::features::minecraft::domain::{map_data as domain, map_query::MapQuery};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Catalog,
    Area,
    Blocks,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    kind: Kind,
    #[serde(deserialize_with = "Option::deserialize")]
    world: Option<String>,
    sampled_at_ms: i64,
    scanned_chunks: u16,
    missing_chunks: u16,
    truncated: bool,
    worlds: Vec<World>,
    blocks: Vec<String>,
    cells: Vec<Cell>,
    structures: Vec<Structure>,
    matches: Vec<Match>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct World {
    id: String,
    name: String,
    map_id: String,
    min_y: i32,
    max_y: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    x: i32,
    z: i32,
    y: i32,
    biome: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Structure {
    kind: String,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Match {
    x: i32,
    y: i32,
    z: i32,
}

pub(super) fn request(query: &MapQuery) -> Result<Vec<u8>, serde_json::Error> {
    use serde_json::json;
    let value = match query {
        MapQuery::Catalog => json!({"kind":"catalog"}),
        MapQuery::Area { world, region, y } => {
            json!({"kind":"area", "world":world,"chunk_x":region.chunk_x,"chunk_z":region.chunk_z,"width":region.width,"height":region.height,"y":y})
        }
        MapQuery::Blocks {
            world,
            region,
            block,
            min_y,
            max_y,
        } => {
            json!({"kind":"blocks", "world":world,"chunk_x":region.chunk_x,"chunk_z":region.chunk_z,"width":region.width,"height":region.height,"block":block,"min_y":min_y,"max_y":max_y})
        }
    };
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn parse(bytes: &[u8], query: &MapQuery) -> anyhow::Result<domain::MapData> {
    anyhow::ensure!(bytes.last() == Some(&b'\n'), "Incomplete world response");
    let value: Reply = serde_json::from_slice(bytes)?;
    let data = domain::MapData {
        kind: match value.kind {
            Kind::Catalog => domain::MapKind::Catalog,
            Kind::Area => domain::MapKind::Area,
            Kind::Blocks => domain::MapKind::Blocks,
        },
        world: value.world,
        sampled_at_ms: value.sampled_at_ms,
        scanned_chunks: value.scanned_chunks,
        missing_chunks: value.missing_chunks,
        truncated: value.truncated,
        worlds: value
            .worlds
            .into_iter()
            .map(|v| domain::MapWorld {
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
            .map(|v| domain::MapCell {
                x: v.x,
                y: v.y,
                z: v.z,
                biome: v.biome,
            })
            .collect(),
        structures: value
            .structures
            .into_iter()
            .map(|v| domain::MapStructure {
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
            .map(|v| domain::MapMatch {
                x: v.x,
                y: v.y,
                z: v.z,
            })
            .collect(),
    };
    anyhow::ensure!(data.valid_for(query), "Invalid world response");
    Ok(data)
}
