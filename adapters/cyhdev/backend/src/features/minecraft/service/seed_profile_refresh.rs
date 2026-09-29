//! A single retained refresh finishes even when its triggering viewport tile is cancelled.

use super::{
    seed_profile::Profile,
    seed_tile::{ActiveProfile, now_ms},
    world_query::{WorldQueryService, exchange},
};
use crate::features::minecraft::domain::seed_tile::SeedDimension;
use crate::features::minecraft::error::MapError;
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, watch};

#[derive(Clone, Copy)]
pub(super) enum Failure {
    Disabled,
    Busy,
    Unavailable,
}

impl From<Failure> for MapError {
    fn from(value: Failure) -> Self {
        match value {
            Failure::Disabled => Self::Disabled,
            Failure::Busy => Self::Busy,
            Failure::Unavailable => Self::Unavailable,
        }
    }
}

pub(super) type Receiver = watch::Receiver<Option<Result<Arc<ActiveProfile>, Failure>>>;

pub(super) async fn refresh(
    world: &WorldQueryService,
    cached: &RwLock<Option<Arc<ActiveProfile>>>,
    dimension: SeedDimension,
) -> Result<Arc<ActiveProfile>, Failure> {
    let path = world.path.as_ref().ok_or(Failure::Disabled)?;
    let mut gate = world.gate.try_lock().map_err(|_| Failure::Busy)?;
    let now = tokio::time::Instant::now();
    if now < *gate {
        return Err(Failure::Busy);
    }
    *gate = now + Duration::from_secs(21);
    let request = format!(
        "{{\"kind\":\"seed_profile\",\"world\":\"{}\"}}\n",
        dimension.world()
    );
    let result =
        tokio::time::timeout(Duration::from_secs(16), exchange(path, request.as_bytes())).await;
    *gate = tokio::time::Instant::now() + Duration::from_secs(1);
    let bytes = match result {
        Ok(Ok(bytes)) => bytes,
        _ => return Err(Failure::Unavailable),
    };
    let profile = Profile::parse(
        &bytes,
        dimension.world(),
        now_ms().map_err(|_| Failure::Unavailable)?,
    )
    .map_err(|_| Failure::Unavailable)?;
    let mut cached = cached.write().await;
    let epoch = match cached.as_ref() {
        Some(previous) if previous.profile.same(&profile) => previous.epoch.clone(),
        _ => uuid::Uuid::new_v4().to_string(),
    };
    let active = Arc::new(ActiveProfile { profile, epoch });
    *cached = Some(Arc::clone(&active));
    Ok(active)
}
