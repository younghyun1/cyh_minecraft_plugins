//! Opt-in coverage/cache verification using the built worker and a private mock Paper socket.

use super::*;
use crate::features::minecraft::api::prediction_dto::MinecraftPrediction;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct FixtureDirectory(PathBuf);

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        // The directory contains only this test's copied binary and local socket.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn query() -> PredictionQuery {
    PredictionQuery {
        world: "minecraft:overworld".into(),
        min_x: -1,
        min_z: -1,
        y: 64,
    }
}

fn context(final_read: bool) -> anyhow::Result<Value> {
    let coverage: Vec<_> = (-1..7)
        .flat_map(|z| {
            (-1..7).map(move |x| {
                let state = if final_read && x == -1 && z == -1 {
                    "generated"
                } else {
                    "ungenerated"
                };
                json!({"chunk_x":x,"chunk_z":z,"state":state})
            })
        })
        .collect();
    Ok(json!({
        "kind":"prediction_context","world":"minecraft:overworld",
        "world_id":"f3746015-a963-49cd-bb3a-5f1e77f1c936","sampled_at_ms":now_ms()?,
        "seed":1,"preset":"large_biomes","profile_revision":"a".repeat(64),"coverage":coverage,
    }))
}

async fn serve_contexts(listener: tokio::net::UnixListener) -> anyhow::Result<usize> {
    let mut requests = 0;
    for read in 0..4 {
        let (stream, _) = listener.accept().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let request: Value = serde_json::from_str(&line)?;
        assert_eq!(
            request,
            json!({"kind":"prediction_context","world":"minecraft:overworld",
                "chunk_x":-1,"chunk_z":-1,"width":8,"height":8,"y":64})
        );
        let reply = context(read == 3)?;
        reader
            .get_mut()
            .write_all(format!("{reply}\n").as_bytes())
            .await?;
        requests += 1;
    }
    Ok(requests)
}

/// Build the worker in dev mode, then set MINECRAFT_TEST_SEED_WORKER to its absolute path.
#[tokio::test]
#[ignore = "requires an explicitly configured built Minecraft seed worker"]
async fn real_worker_cache_hit_rechecks_coverage_without_an_executable() -> anyhow::Result<()> {
    let source = PathBuf::from(std::env::var_os("MINECRAFT_TEST_SEED_WORKER").ok_or_else(
        || anyhow::anyhow!("Set MINECRAFT_TEST_SEED_WORKER to an absolute built worker path"),
    )?);
    anyhow::ensure!(
        source.is_absolute() && source.is_file(),
        "Invalid test worker path"
    );
    let directory = FixtureDirectory(
        std::env::temp_dir().join(format!("mp-{}", uuid::Uuid::new_v4().simple())),
    );
    tokio::fs::create_dir(&directory.0).await?;
    let executable = directory.0.join("worker");
    tokio::fs::copy(source, &executable).await?;
    let path = directory.0.join("world.sock");
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService {
        path: Some(path),
        predictor: Some(executable.clone()),
        prediction_cache: super::super::prediction_cache::PredictionCache::new(),
        gate: tokio::sync::Mutex::new(Instant::now()),
    };
    let server = tokio::spawn(serve_contexts(listener));
    let result = exercise(&service, &executable).await;
    if result.is_err() {
        server.abort();
    }
    result?;
    assert_eq!(
        server.await??,
        4,
        "Cache hits still require both live contexts"
    );
    Ok(())
}

async fn exercise(service: &WorldQueryService, executable: &Path) -> anyhow::Result<()> {
    let first = service.predict(query()).await?;
    assert_eq!(first.cells.len(), 1024);
    assert_eq!(first.min_x, -16);
    assert_eq!(first.min_z, -16);
    assert_eq!(first.expires_at_ms - first.sampled_at_ms, 15_000);
    let public = serde_json::to_value(MinecraftPrediction::from(first))?;
    assert_eq!(public["preset"], "large_biomes");
    for field in ["seed", "world_id", "profile_revision"] {
        assert!(public.get(field).is_none());
    }
    // Removing only the copied test binary makes a second worker invocation fail definitively.
    tokio::fs::remove_file(executable).await?;
    *service.gate.lock().await = Instant::now();
    let cached = service.predict(query()).await?;
    assert_eq!(cached.cells.len(), 1008);
    assert!(
        cached
            .cells
            .iter()
            .all(|cell| { cell.x.div_euclid(16) != -1 || cell.z.div_euclid(16) != -1 })
    );
    assert_eq!(cached.expires_at_ms - cached.sampled_at_ms, 15_000);
    assert!(cached.coverage.iter().any(|chunk| {
        chunk.chunk_x == -1
            && chunk.chunk_z == -1
            && chunk.state
                == crate::features::minecraft::domain::prediction::CoverageState::Generated
    }));
    Ok(())
}
