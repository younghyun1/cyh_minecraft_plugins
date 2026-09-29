use super::*;
use crate::features::minecraft::domain::prediction::GENERATOR_REVISION;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn query() -> PredictionQuery {
    PredictionQuery {
        world: "minecraft:overworld".into(),
        min_x: -16,
        min_z: -16,
        y: 64,
    }
}

fn context(state: &str, now: i64) -> Value {
    let coverage: Vec<_> = (-1..7)
        .flat_map(|x| (-1..7).map(move |z| json!({"chunk_x":x,"chunk_z":z,"state":state})))
        .collect();
    json!({"kind":"prediction_context","world":"minecraft:overworld",
        "world_id":"f3746015-a963-49cd-bb3a-5f1e77f1c936","sampled_at_ms":now,"seed":-1234,
        "preset":"large_biomes","profile_revision":"0123456789abcdef","coverage":coverage})
}

fn parse_context(value: &Value, now: i64) -> anyhow::Result<prediction_wire::Context> {
    prediction_wire::parse(format!("{value}\n").as_bytes(), &query(), now)
}

#[test]
fn coverage_requires_complete_unique_fresh_coordinates_and_known_presets() -> anyhow::Result<()> {
    let now = 100_000;
    assert!(parse_context(&context("ungenerated", now), now).is_ok());
    for mutate in [
        |v: &mut Value| {
            v["coverage"][0] = v["coverage"][1].clone();
        },
        |v: &mut Value| {
            v["coverage"][0]["chunk_x"] = json!(-2);
        },
        |v: &mut Value| {
            v["coverage"][0]["state"] = json!("missing");
        },
        |v: &mut Value| {
            v["sampled_at_ms"] = json!(94_999);
        },
    ] {
        let mut value = context("ungenerated", now);
        mutate(&mut value);
        assert!(parse_context(&value, now).is_err());
    }
    let mut value = context("ungenerated", now);
    value["coverage"] = json!([]);
    assert!(parse_context(&value, now).is_err());
    value = context("ungenerated", now);
    value["preset"] = json!("amplified");
    assert!(parse_context(&value, now).is_err());
    value = context("ungenerated", now + 1);
    assert!(parse_context(&value, now).is_err());
    assert!(
        prediction_wire::parse(
            &serde_json::to_vec(&context("ungenerated", now))?,
            &query(),
            now
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn newly_generated_or_previously_unknown_chunks_never_receive_predictions() -> anyhow::Result<()> {
    let now = 100_000;
    let mut first = context("ungenerated", now);
    let mut second = first.clone();
    first["coverage"][1]["state"] = json!("unknown");
    second["coverage"][0]["state"] = json!("generated");
    second["coverage"][2]["state"] = json!("excluded");
    let before = parse_context(&first, now)?;
    let after = parse_context(&second, now)?;
    let cells = (-1..3)
        .map(|z| PredictedBiome {
            x: -16,
            z: z * 16,
            biome: "minecraft:plains".into(),
        })
        .collect();
    let cells = filter_cells(cells, &before, &after);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].z, 32);
    for (field, replacement) in [
        ("seed", json!(5)),
        ("preset", json!("default")),
        ("profile_revision", json!("1123456789abcdef")),
        ("world_id", json!("3eaf8ea2-a6e5-4b62-8f30-38c8c9a4c91e")),
    ] {
        let mut changed = second.clone();
        changed[field] = replacement;
        assert!(!before.same_profile(&parse_context(&changed, now)?));
    }
    Ok(())
}

#[test]
fn worker_output_requires_exact_grid_and_matching_engine_and_preset() -> anyhow::Result<()> {
    let cells: Vec<_> = (0..32)
        .flat_map(|x| {
            (0..32).map(move |z| json!({"x":-16+x*4,"z":-16+z*4,"biome":"minecraft:plains"}))
        })
        .collect();
    let output = json!({"generator_revision":GENERATOR_REVISION,"large_biomes":true,"cells":cells});
    let parse =
        |value: &Value| prediction_process::parse(format!("{value}\n").as_bytes(), &query(), true);
    assert_eq!(parse(&output)?.len(), 1024);
    for (field, value) in [
        ("large_biomes", json!(false)),
        ("generator_revision", json!("old")),
        ("cells", json!([])),
    ] {
        let mut changed = output.clone();
        changed[field] = value;
        assert!(parse(&changed).is_err());
    }
    let mut changed = output.clone();
    changed["cells"][0] = changed["cells"][1].clone();
    assert!(parse(&changed).is_err());
    changed = output.clone();
    changed["cells"][0]["x"] = json!(-17);
    assert!(parse(&changed).is_err());
    changed = output.clone();
    changed["cells"][0]["biome"] = json!("<script>");
    assert!(parse(&changed).is_err());
    assert!(prediction_process::parse(&serde_json::to_vec(&output)?, &query(), true).is_err());
    Ok(())
}

#[tokio::test]
async fn all_generated_preview_skips_worker_and_never_serializes_seed() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("cyhdev-pred-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService {
        path: Some(path.clone()),
        predictor: Some("/nonexistent/worker".into()),
        prediction_cache: super::super::prediction_cache::PredictionCache::new(),
        gate: tokio::sync::Mutex::new(Instant::now()),
    };
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            let request: Value = serde_json::from_str(&line)?;
            assert_eq!(request["kind"], "prediction_context");
            assert_eq!(request["chunk_x"], -1);
            assert!(request.get("seed").is_none());
            let reply = context("generated", now_ms()?);
            reader
                .get_mut()
                .write_all(format!("{reply}\n").as_bytes())
                .await?;
        }
        Ok::<_, anyhow::Error>(())
    });
    let data = service.predict(query()).await?;
    assert!(data.cells.is_empty());
    let public = crate::features::minecraft::api::prediction_dto::MinecraftPrediction::from(data);
    let public = serde_json::to_value(public)?;
    assert_eq!(public["preset"], "large_biomes");
    assert_eq!(public["min_x"], -16);
    for field in ["seed", "world_id", "profile_revision"] {
        assert!(public.get(field).is_none());
    }
    assert!(matches!(
        service.predict(query()).await,
        Err(MapError::Busy)
    ));
    server.await??;
    tokio::fs::remove_file(path).await?;
    Ok(())
}

#[tokio::test]
async fn cancelled_preview_preserves_shared_reservation() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("cyhdev-pred-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService {
        path: Some(path.clone()),
        predictor: Some("/unused".into()),
        prediction_cache: super::super::prediction_cache::PredictionCache::new(),
        gate: tokio::sync::Mutex::new(Instant::now()),
    };
    let (sent, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut line = String::new();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut line).await?;
        let _ = sent.send(());
        std::future::pending::<()>().await;
        Ok::<_, anyhow::Error>(())
    });
    let mut operation = Box::pin(service.predict(query()));
    tokio::select! {
        result = &mut operation => { result?; anyhow::bail!("Preview completed without context"); }
        result = received => { result?; }
    }
    drop(operation);
    assert!(
        service
            .gate
            .try_lock()?
            .saturating_duration_since(Instant::now())
            > Duration::from_secs(19)
    );
    assert!(matches!(
        service
            .query(crate::features::minecraft::domain::map_query::MapQuery::Catalog)
            .await,
        Err(MapError::Busy)
    ));
    server.abort();
    tokio::fs::remove_file(path).await?;
    Ok(())
}
