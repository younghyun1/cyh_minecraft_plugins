//! Database-current administrator authority for public marker mutations.

use crate::features::{
    accounts::service::account_service::AccountService,
    minecraft::{
        domain::{
            map_query::identifier,
            waypoint::{Waypoint, WaypointInput},
        },
        error::MapError,
        repository::waypoints::WaypointRepository,
    },
};
use std::sync::Arc;
use uuid::Uuid;

pub struct WaypointService {
    repository: WaypointRepository,
    accounts: Arc<AccountService>,
}

impl WaypointService {
    pub fn new(repository: WaypointRepository, accounts: Arc<AccountService>) -> Self {
        Self {
            repository,
            accounts,
        }
    }

    pub async fn list(&self, world: &str) -> Result<Vec<Waypoint>, MapError> {
        if !identifier(world) {
            return Err(MapError::Invalid);
        }
        self.repository.list(world).await
    }

    pub async fn save(
        &self,
        actor: Uuid,
        id: Option<Uuid>,
        input: WaypointInput,
    ) -> Result<Waypoint, MapError> {
        if !input.valid() || id.is_some_and(|id| id.is_nil()) {
            return Err(MapError::Invalid);
        }
        let _authority = self
            .accounts
            .acquire_current_younghyun_authority(actor)
            .await?;
        self.repository.save(id, input).await
    }

    pub async fn delete(&self, actor: Uuid, id: Uuid) -> Result<(), MapError> {
        if id.is_nil() {
            return Err(MapError::Invalid);
        }
        let _authority = self
            .accounts
            .acquire_current_younghyun_authority(actor)
            .await?;
        self.repository.delete(id).await
    }
}
