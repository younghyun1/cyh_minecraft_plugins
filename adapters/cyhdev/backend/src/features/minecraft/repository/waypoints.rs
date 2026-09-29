//! World-row locks serialize slot allocation; unique bounded slots enforce the cap in PostgreSQL.

use crate::{
    features::minecraft::{
        domain::waypoint::{Waypoint, WaypointInput},
        error::MapError,
    },
    schema::{minecraft_map_world as world, minecraft_waypoint as marker},
};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl, pooled_connection::bb8::Pool};
use uuid::Uuid;

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = marker, check_for_backend(diesel::pg::Pg))]
struct Row {
    minecraft_waypoint_id: Uuid,
    minecraft_waypoint_world_id: String,
    minecraft_waypoint_name: String,
    minecraft_waypoint_description: String,
    minecraft_waypoint_x: i32,
    minecraft_waypoint_y: i16,
    minecraft_waypoint_z: i32,
}

impl From<Row> for Waypoint {
    fn from(value: Row) -> Self {
        Self {
            id: value.minecraft_waypoint_id,
            input: WaypointInput {
                world: value.minecraft_waypoint_world_id,
                name: value.minecraft_waypoint_name,
                description: value.minecraft_waypoint_description,
                x: value.minecraft_waypoint_x,
                y: i32::from(value.minecraft_waypoint_y),
                z: value.minecraft_waypoint_z,
            },
        }
    }
}

pub struct WaypointRepository {
    pool: Pool<AsyncPgConnection>,
}

impl WaypointRepository {
    pub fn new(pool: Pool<AsyncPgConnection>) -> Self {
        Self { pool }
    }

    pub async fn list(&self, key: &str) -> Result<Vec<Waypoint>, MapError> {
        let mut connection = self.pool.get().await.map_err(MapError::Pool)?;
        Ok(marker::table
            .filter(marker::minecraft_waypoint_world_id.eq(key))
            .order(marker::minecraft_waypoint_slot.asc())
            .limit(256)
            .select(Row::as_select())
            .load::<Row>(&mut connection)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// A replacement may move worlds; locking its target world keeps capacity decisions atomic.
    pub async fn save(&self, id: Option<Uuid>, input: WaypointInput) -> Result<Waypoint, MapError> {
        if !input.valid() {
            return Err(MapError::Invalid);
        }
        let y = i16::try_from(input.y).map_err(|_| MapError::Invalid)?;
        let mut connection = self.pool.get().await.map_err(MapError::Pool)?;
        connection
            .transaction::<Waypoint, MapError, _>(async move |connection| {
                diesel::insert_into(world::table)
                    .values(world::minecraft_map_world_id.eq(&input.world))
                    .on_conflict_do_nothing()
                    .execute(connection)
                    .await?;
                world::table
                    .find(&input.world)
                    .select(world::minecraft_map_world_id)
                    .for_update()
                    .first::<String>(connection)
                    .await?;
                let current = match id {
                    Some(id) => Some(
                        marker::table
                            .find(id)
                            .select((
                                marker::minecraft_waypoint_world_id,
                                marker::minecraft_waypoint_slot,
                            ))
                            .for_update()
                            .first::<(String, i16)>(connection)
                            .await
                            .optional()?
                            .ok_or(MapError::NotFound)?,
                    ),
                    None => None,
                };
                let slot = match current {
                    Some((key, slot)) if key == input.world => slot,
                    _ => {
                        let occupied = marker::table
                            .filter(marker::minecraft_waypoint_world_id.eq(&input.world))
                            .select(marker::minecraft_waypoint_slot)
                            .limit(256)
                            .load::<i16>(connection)
                            .await?;
                        (0..256)
                            .find(|slot| !occupied.contains(slot))
                            .ok_or(MapError::Full)?
                    }
                };
                let values = (
                    marker::minecraft_waypoint_world_id.eq(&input.world),
                    marker::minecraft_waypoint_slot.eq(slot),
                    marker::minecraft_waypoint_name.eq(&input.name),
                    marker::minecraft_waypoint_description.eq(&input.description),
                    marker::minecraft_waypoint_x.eq(input.x),
                    marker::minecraft_waypoint_y.eq(y),
                    marker::minecraft_waypoint_z.eq(input.z),
                );
                let row = match id {
                    Some(id) => {
                        diesel::update(marker::table.find(id))
                            .set(values)
                            .returning(Row::as_returning())
                            .get_result::<Row>(connection)
                            .await?
                    }
                    None => {
                        diesel::insert_into(marker::table)
                            .values(values)
                            .returning(Row::as_returning())
                            .get_result::<Row>(connection)
                            .await?
                    }
                };
                Ok(row.into())
            })
            .await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), MapError> {
        let mut connection = self.pool.get().await.map_err(MapError::Pool)?;
        let deleted = diesel::delete(marker::table.find(id))
            .execute(&mut connection)
            .await?;
        if deleted == 0 {
            Err(MapError::NotFound)
        } else {
            Ok(())
        }
    }
}
