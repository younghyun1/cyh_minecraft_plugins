//! Browser request contracts for generated terrain and public waypoints.

use crate::features::minecraft::domain::{
    map_query::{MapQuery, Region},
    waypoint::{Waypoint, WaypointInput},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MinecraftMapQuery {
    Catalog {},
    Area {
        world: String,
        chunk_x: i32,
        chunk_z: i32,
        width: u8,
        height: u8,
        y: Option<i32>,
    },
    Blocks {
        world: String,
        chunk_x: i32,
        chunk_z: i32,
        width: u8,
        height: u8,
        block: String,
        min_y: i32,
        max_y: i32,
    },
}

impl From<MinecraftMapQuery> for MapQuery {
    fn from(value: MinecraftMapQuery) -> Self {
        match value {
            MinecraftMapQuery::Catalog {} => Self::Catalog,
            MinecraftMapQuery::Area {
                world,
                chunk_x,
                chunk_z,
                width,
                height,
                y,
            } => Self::Area {
                world,
                region: Region {
                    chunk_x,
                    chunk_z,
                    width,
                    height,
                },
                y,
            },
            MinecraftMapQuery::Blocks {
                world,
                chunk_x,
                chunk_z,
                width,
                height,
                block,
                min_y,
                max_y,
            } => Self::Blocks {
                world,
                region: Region {
                    chunk_x,
                    chunk_z,
                    width,
                    height,
                },
                block,
                min_y,
                max_y,
            },
        }
    }
}

#[derive(Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct MinecraftWaypointQuery {
    pub world: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MinecraftWaypointInput {
    pub world: String,
    pub name: String,
    pub description: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<MinecraftWaypointInput> for WaypointInput {
    fn from(value: MinecraftWaypointInput) -> Self {
        Self {
            world: value.world,
            name: value.name,
            description: value.description,
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftWaypoint {
    pub id: Uuid,
    pub world: String,
    pub name: String,
    pub description: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<Waypoint> for MinecraftWaypoint {
    fn from(value: Waypoint) -> Self {
        let input = value.input;
        Self {
            id: value.id,
            world: input.world,
            name: input.name,
            description: input.description,
            x: input.x,
            y: input.y,
            z: input.z,
        }
    }
}
