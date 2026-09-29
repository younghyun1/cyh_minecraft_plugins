//! Fresh generated-terrain observations and their completeness metadata.

use super::map_query::{MapQuery, horizontal, identifier, vertical};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    Catalog,
    Area,
    Blocks,
}

pub struct MapData {
    pub kind: MapKind,
    pub world: Option<String>,
    pub sampled_at_ms: i64,
    pub scanned_chunks: u16,
    pub missing_chunks: u16,
    pub truncated: bool,
    pub worlds: Vec<MapWorld>,
    pub blocks: Vec<String>,
    pub cells: Vec<MapCell>,
    pub structures: Vec<MapStructure>,
    pub matches: Vec<MapMatch>,
}

pub struct MapWorld {
    pub id: String,
    pub name: String,
    pub map_id: String,
    pub min_y: i32,
    pub max_y: i32,
}
pub struct MapCell {
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub biome: String,
}
pub struct MapStructure {
    pub kind: String,
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}
pub struct MapMatch {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl MapData {
    /// Treat the local plugin as a protocol boundary: reject oversized or unrelated results.
    pub fn valid_for(&self, query: &MapQuery) -> bool {
        if self.sampled_at_ms < 0
            || self.worlds.len() > 16
            || self.blocks.len() > 4096
            || self.cells.len() > 1024
            || self.structures.len() > 256
            || self.matches.len() > 512
        {
            return false;
        }
        if self
            .worlds
            .iter()
            .map(|v| &v.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.worlds.len()
            || self
                .cells
                .iter()
                .map(|v| (v.x, v.z))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.cells.len()
            || self
                .matches
                .iter()
                .map(|v| (v.x, v.y, v.z))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.matches.len()
        {
            return false;
        }
        match query {
            MapQuery::Catalog => {
                self.kind == MapKind::Catalog
                    && self.world.is_none()
                    && self.scanned_chunks == 0
                    && self.missing_chunks == 0
                    && self.cells.is_empty()
                    && self.structures.is_empty()
                    && self.matches.is_empty()
                    && self.blocks.iter().all(|value| identifier(value))
                    && self.worlds.iter().all(|value| {
                        identifier(&value.id)
                            && !value.name.is_empty()
                            && value.name.chars().count() <= 128
                            && !value.name.chars().any(char::is_control)
                            && value.map_id == value.id.replace(':', "_")
                            && vertical(value.min_y)
                            && vertical(value.max_y)
                            && value.min_y <= value.max_y
                    })
            }
            MapQuery::Area { world, region, .. } => {
                self.kind == MapKind::Area
                    && self.valid_region(world, *region)
                    && self.matches.is_empty()
                    && self.cells.len() == usize::from(self.scanned_chunks) * 16
                    && self.cells.iter().all(|cell| {
                        region.contains(cell.x, cell.z)
                            && cell.x.rem_euclid(4) == 0
                            && cell.z.rem_euclid(4) == 0
                            && vertical(cell.y)
                            && identifier(&cell.biome)
                    })
                    && self.structures.iter().all(|value| {
                        identifier(&value.kind)
                            && [value.min_x, value.min_z, value.max_x, value.max_z]
                                .into_iter()
                                .all(horizontal)
                            && vertical(value.min_y)
                            && vertical(value.max_y)
                            && value.min_x <= value.max_x
                            && value.min_y <= value.max_y
                            && value.min_z <= value.max_z
                    })
            }
            MapQuery::Blocks {
                world,
                region,
                min_y,
                max_y,
                ..
            } => {
                self.kind == MapKind::Blocks
                    && self.valid_region(world, *region)
                    && self.cells.is_empty()
                    && self.structures.is_empty()
                    && self.matches.iter().all(|value| {
                        region.contains(value.x, value.z) && (*min_y..=*max_y).contains(&value.y)
                    })
            }
        }
    }

    fn valid_region(&self, world: &str, region: super::map_query::Region) -> bool {
        let chunks = u32::from(self.scanned_chunks) + u32::from(self.missing_chunks);
        let requested = u32::from(region.width) * u32::from(region.height);
        self.world.as_deref() == Some(world)
            && self.worlds.is_empty()
            && self.blocks.is_empty()
            && chunks <= requested
            && (self.truncated || chunks == requested)
    }
}
