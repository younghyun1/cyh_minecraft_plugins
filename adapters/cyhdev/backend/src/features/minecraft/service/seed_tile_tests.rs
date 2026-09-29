use super::super::{prediction_cache::PredictionCache, seed_tile_cache::Tile};
use super::*;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(super) fn query() -> SeedTileQuery {
    SeedTileQuery {
        world: "minecraft:overworld".into(),
        tile_x: -1,
        tile_z: 0,
        level: 0,
        y: 64,
    }
}

pub(super) fn wire(now: i64) -> serde_json::Value {
    json!({"kind":"seed_profile","world":"minecraft:overworld","world_id":"00000000-0000-0000-0000-000000000001",
        "seed":1,"preset":"large_biomes","profile_revision":"a".repeat(64),"sampled_at_ms":now,
        "world_border":{"min_x":-30_000_000,"min_z":-30_000_000,"max_x":30_000_000,"max_z":30_000_000},"visibility":[]})
}

pub(super) fn profile(value: &serde_json::Value, now: i64) -> anyhow::Result<Profile> {
    Profile::parse(format!("{value}\n").as_bytes(), "minecraft:overworld", now)
}

pub(super) fn service(path: std::path::PathBuf) -> anyhow::Result<SeedTileService> {
    SeedTileService::new(Arc::new(WorldQueryService {
        path: Some(path),
        predictor: None,
        prediction_cache: PredictionCache::new(),
        gate: Mutex::new(tokio::time::Instant::now()),
    }))
}

#[test]
fn profiles_reject_stale_unknown_or_oversized_permissions() -> anyhow::Result<()> {
    let now = 10_000;
    let value = wire(now);
    assert!(profile(&value, now).is_ok());
    assert!(profile(&value, now + 5_000).is_err());
    assert!(profile(&value, now - 1).is_err());
    for (key, invalid) in [
        ("preset", json!("custom")),
        ("seed", json!("1")),
        ("profile_revision", json!("private-not-hex")),
        ("visibility", json!([{"kind":"polygon","points":[]}])),
    ] {
        let mut changed = value.clone();
        changed[key] = invalid;
        assert!(profile(&changed, now).is_err());
    }
    let mut changed = value;
    changed["visibility"] = json!(vec![
        json!({"kind":"circle","center_x":0,"center_z":0,"radius":10});
        65
    ]);
    assert!(profile(&changed, now).is_err());
    Ok(())
}

#[test]
fn complete_coarse_cells_must_fit_the_union_and_world_border() -> anyhow::Result<()> {
    let now = 10_000;
    let mut value = wire(now);
    value["world_border"] = json!({"min_x":-20,"min_z":-20,"max_x":20,"max_z":20});
    value["visibility"] = json!([
        {"kind":"rectangle","min_x":-16,"min_z":-16,"max_x":-1,"max_z":-1},
        {"kind":"circle","center_x":8,"center_z":8,"radius":8}]);
    let data = profile(&value, now)?;
    assert!(data.permits(-16, -16, 16));
    assert!(data.permits(8, 8, 4));
    assert!(!data.permits(-1, -1, 4));
    assert!(!data.permits(15, 8, 4));
    assert!(!data.permits(-16, -16, 32));
    Ok(())
}

#[tokio::test]
async fn warmed_tiles_use_one_profile_read_and_reuse_cached_data() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("seed-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let backend = service(path.clone())?;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        reader.read_line(&mut request).await?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request)?,
            json!({"kind":"seed_profile","world":"minecraft:overworld"})
        );
        reader
            .get_mut()
            .write_all(format!("{}\n", wire(now_ms()?)).as_bytes())
            .await?;
        Ok::<_, anyhow::Error>(())
    });
    let first = backend.tile(query()).await?;
    assert_eq!(first.indices.len(), 4096);
    assert!(first.indices.iter().all(Option::is_some));
    assert_eq!(first.expires_at_ms - first.sampled_at_ms, 15_000);
    server.await??;
    tokio::fs::remove_file(&path).await?;
    let active = backend.profile("minecraft:overworld").await?;
    let mut next = query();
    next.tile_x = 0;
    let key = Key::new(&active.profile, &next);
    // A fixture-only biome proves this reply came from RAM instead of another generator call.
    backend
        .cache
        .insert(
            key,
            Arc::new(Tile {
                palette: vec!["minecraft:cache_fixture".into()],
                indices: vec![Some(0); 4096],
            }),
        )
        .await;
    let warm = backend.tile(next).await?;
    assert_eq!(warm.palette, vec!["minecraft:cache_fixture"]);
    assert_eq!(warm.profile_epoch, first.profile_epoch);
    let public = serde_json::to_value(
        crate::features::minecraft::api::seed_tile_dto::MinecraftSeedTile::from(warm),
    )?;
    for field in [
        "seed",
        "world_id",
        "profile_revision",
        "visibility",
        "world_border",
    ] {
        assert!(public.get(field).is_none());
    }
    Ok(())
}

#[tokio::test]
async fn admission_is_bounded_and_expired_profiles_never_authorize_cached_tiles()
-> anyhow::Result<()> {
    let backend =
        service(std::env::temp_dir().join(format!("absent-{}.sock", uuid::Uuid::new_v4())))?;
    let admission = backend.admission.acquire_many(64).await?;
    assert!(matches!(backend.tile(query()).await, Err(MapError::Busy)));
    drop(admission);
    let now = now_ms()?;
    let old = profile(&wire(now - 20_000), now - 20_000)?;
    let key = Key::new(&old, &query());
    backend
        .cache
        .insert(
            key,
            Arc::new(Tile {
                palette: vec!["minecraft:plains".into()],
                indices: vec![Some(0); 4096],
            }),
        )
        .await;
    *backend.profile[0].write().await = Some(Arc::new(ActiveProfile {
        profile: old,
        epoch: "old".into(),
    }));
    assert!(matches!(
        backend.tile(query()).await,
        Err(MapError::Unavailable)
    ));
    Ok(())
}

#[tokio::test]
async fn canceled_cold_tiles_do_not_abandon_the_shared_profile_refresh() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("seed-cancel-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let backend = service(path.clone())?;
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let _ = received_tx.send(());
        finish_rx.await?;
        reader
            .get_mut()
            .write_all(format!("{}\n", wire(now_ms()?)).as_bytes())
            .await?;
        Ok::<_, anyhow::Error>(())
    });
    let mut pending = Box::pin(backend.tile(query()));
    tokio::select! { result=&mut pending => { result?; anyhow::bail!("Unexpected completed profile"); }, result=received_rx => { result?; } }
    drop(pending);
    assert_eq!(backend.admission.available_permits(), 64);
    let _ = finish_tx.send(());
    let tile =
        tokio::time::timeout(std::time::Duration::from_secs(2), backend.tile(query())).await??;
    assert_eq!(tile.indices.len(), 4096);
    server.await??;
    tokio::fs::remove_file(&path).await?;
    Ok(())
}

#[tokio::test]
async fn world_edge_tiles_crop_generator_samples_and_mask_whole_cells() -> anyhow::Result<()> {
    let backend = service(std::env::temp_dir().join("unused-seed-profile.sock"))?;
    let now = now_ms()?;
    let active = profile(&wire(now), now)?;
    *backend.profile[0].write().await = Some(Arc::new(ActiveProfile {
        profile: active,
        epoch: "fixture".into(),
    }));
    let mut edge = query();
    edge.tile_x = -117188;
    let tile = backend.tile(edge).await?;
    assert_eq!(tile.min_x, -30_000_128);
    assert!(tile.indices[..32].iter().all(Option::is_none));
    assert!(tile.indices[32..64].iter().all(Option::is_some));
    Ok(())
}
