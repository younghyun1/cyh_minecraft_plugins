use super::super::seed_tile_cache::Tile;
use super::tests::{profile, query, service, wire};
use super::*;
use serde_json::json;

#[tokio::test]
async fn concurrent_tiles_coalesce_and_cached_permissions_hide_unused_palette_entries()
-> anyhow::Result<()> {
    let backend = Arc::new(service(
        std::env::temp_dir().join("unused-concurrent-profile.sock"),
    )?);
    let now = now_ms()?;
    *backend.profile[0].write().await = Some(Arc::new(ActiveProfile {
        profile: profile(&wire(now), now)?,
        epoch: "fixture".into(),
    }));
    let mut jobs = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let backend = Arc::clone(&backend);
        jobs.spawn(async move { backend.tile(query()).await });
    }
    while let Some(result) = jobs.join_next().await {
        assert_eq!(result??.indices.len(), 4096);
    }
    assert_eq!(
        backend.generated.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert!(
        backend
            .flights
            .lock()
            .await
            .values()
            .all(|flight| flight.strong_count() == 0)
    );
    let started = std::time::Instant::now();
    for _ in 0..100 {
        backend.tile(query()).await?;
    }
    println!(
        "100 warm seed tiles without Paper or generator: {:?}",
        started.elapsed()
    );
    assert_eq!(
        backend.generated.load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    let mut changed = wire(now_ms()?);
    changed["profile_revision"] = json!("b".repeat(64));
    changed["visibility"] =
        json!([{"kind":"rectangle","min_x":-256,"min_z":0,"max_x":-129,"max_z":255}]);
    let changed = profile(&changed, now_ms()?)?;
    let key = Key::new(&changed, &query());
    backend
        .cache
        .insert(
            key,
            Arc::new(Tile {
                palette: vec!["minecraft:plains".into(), "minecraft:hidden_fixture".into()],
                indices: (0..4096)
                    .map(|index| Some(if index % 64 < 32 { 0 } else { 1 }))
                    .collect(),
            }),
        )
        .await;
    *backend.profile[0].write().await = Some(Arc::new(ActiveProfile {
        profile: changed,
        epoch: "new-policy".into(),
    }));
    let masked = backend.tile(query()).await?;
    assert_eq!(masked.profile_epoch, "new-policy");
    assert_eq!(masked.palette, vec!["minecraft:plains"]);
    assert_eq!(
        masked
            .indices
            .iter()
            .filter(|value| value.is_some())
            .count(),
        2048
    );
    assert_eq!(
        backend.generated.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    Ok(())
}
