use super::tests::{query, service, wire};
use super::*;
use crate::features::minecraft::domain::seed_tile::{SeedDimension, SeedPreset};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn dimension_wire(dimension: SeedDimension, now: i64) -> serde_json::Value {
    let mut value = wire(now);
    value["world"] = json!(dimension.world());
    value["preset"] = json!(match dimension {
        SeedDimension::Overworld => "large_biomes",
        SeedDimension::Nether => "nether",
        SeedDimension::End => "end",
    });
    value
}

#[test]
fn dimension_and_preset_must_agree() {
    let now = 10_000;
    for dimension in [
        SeedDimension::Overworld,
        SeedDimension::Nether,
        SeedDimension::End,
    ] {
        let value = dimension_wire(dimension, now);
        assert!(Profile::parse(format!("{value}\n").as_bytes(), dimension.world(), now).is_ok());
        for wrong in [
            "custom:world",
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
        ] {
            if wrong == dimension.world() {
                continue;
            }
            let mut mismatch = value.clone();
            mismatch["world"] = json!(wrong);
            assert!(Profile::parse(format!("{mismatch}\n").as_bytes(), wrong, now).is_err());
        }
    }
}

#[tokio::test]
async fn profiles_and_cached_tiles_remain_independent_across_dimensions() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("seed-dimensions-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let backend = service(path.clone())?;
    let worlds = [
        SeedDimension::Overworld,
        SeedDimension::Nether,
        SeedDimension::End,
    ];
    let server = tokio::spawn(async move {
        for dimension in worlds {
            let (stream, _) = listener.accept().await?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&line)?,
                json!({"kind":"seed_profile","world":dimension.world()})
            );
            let response = dimension_wire(dimension, now_ms()?);
            reader
                .get_mut()
                .write_all(format!("{response}\n").as_bytes())
                .await?;
        }
        Ok::<_, anyhow::Error>(())
    });
    let mut epochs = Vec::new();
    for dimension in worlds {
        // Advance only the fixture's observation gate; production retains its cooldown.
        *backend.world.gate.lock().await = tokio::time::Instant::now();
        let mut request = query();
        request.world = dimension.world().into();
        let tile = backend.tile(request).await?;
        assert_eq!(
            tile.preset,
            match dimension {
                SeedDimension::Overworld => SeedPreset::LargeBiomes,
                SeedDimension::Nether => SeedPreset::Nether,
                SeedDimension::End => SeedPreset::End,
            }
        );
        let names: &[&str] = match dimension {
            SeedDimension::Overworld => &[],
            SeedDimension::Nether => &[
                "minecraft:nether_wastes",
                "minecraft:crimson_forest",
                "minecraft:warped_forest",
                "minecraft:soul_sand_valley",
                "minecraft:basalt_deltas",
            ],
            SeedDimension::End => &[
                "minecraft:the_end",
                "minecraft:end_highlands",
                "minecraft:end_midlands",
                "minecraft:end_barrens",
                "minecraft:small_end_islands",
            ],
        };
        assert!(
            tile.palette
                .iter()
                .all(|name| names.is_empty() || names.contains(&name.as_str()))
        );
        assert_eq!(tile.indices.len(), 4096);
        epochs.push(tile.profile_epoch);
    }
    server.await??;
    tokio::fs::remove_file(&path).await?;
    for dimension in worlds {
        let mut request = query();
        request.world = dimension.world().into();
        let tile = backend.tile(request).await?;
        assert_eq!(tile.profile_epoch, epochs[dimension.index()]);
    }
    assert_eq!(
        backend.generated.load(std::sync::atomic::Ordering::Relaxed),
        3
    );
    Ok(())
}

#[test]
fn dimensions_reject_wrong_heights_and_accept_coarse_far_tiles() {
    let mut request = query();
    request.level = 12;
    request.tile_x = -28;
    request.tile_z = 28;
    for dimension in [
        SeedDimension::Overworld,
        SeedDimension::Nether,
        SeedDimension::End,
    ] {
        request.world = dimension.world().into();
        request.y = 64;
        assert!(request.geometry().is_some());
        request.y = -64;
        assert_eq!(
            request.geometry().is_some(),
            dimension == SeedDimension::Overworld
        );
        request.y = 256;
        assert_eq!(
            request.geometry().is_some(),
            dimension == SeedDimension::Overworld
        );
        request.y = 320;
        assert!(request.geometry().is_none());
    }
}

#[tokio::test]
async fn caller_waiting_for_refresh_slot_rechecks_newly_published_profile() -> anyhow::Result<()> {
    let backend = service(std::env::temp_dir().join("unused-refreshed-profile.sock"))?;
    let guard = backend.refresh[0].lock().await;
    let mut waiting = Box::pin(backend.profile("minecraft:overworld"));
    tokio::select! {
        result = &mut waiting => { result?; anyhow::bail!("Refresh lock was not held"); }
        () = tokio::task::yield_now() => {}
    }
    let now = now_ms()?;
    *backend.profile[0].write().await = Some(Arc::new(ActiveProfile {
        profile: Profile::parse(
            format!("{}\n", wire(now)).as_bytes(),
            "minecraft:overworld",
            now,
        )?,
        epoch: "already-published".into(),
    }));
    drop(guard);
    assert_eq!(waiting.await?.epoch, "already-published");
    Ok(())
}
