use super::*;
use serde_json::json;

fn catalog() -> serde_json::Value {
    json!({"kind":"catalog", "world":null, "sampled_at_ms":1, "scanned_chunks":0,"missing_chunks":0,"truncated":false,"worlds":[{"id":"minecraft:overworld","name":"World","map_id":"minecraft_overworld","min_y":-64,"max_y":319}],"blocks":["minecraft:stone"],"cells":[],"structures":[],"matches":[]})
}

fn parse(
    value: &serde_json::Value,
) -> anyhow::Result<crate::features::minecraft::domain::map_data::MapData> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    world_wire::parse(&bytes, &MapQuery::Catalog)
}

#[test]
fn invalid_configuration_and_unrelated_or_excessive_replies_are_rejected() -> anyhow::Result<()> {
    assert!(WorldQueryService::from_path(None).is_ok());
    assert!(WorldQueryService::from_path(Some("relative.sock".into())).is_err());
    assert!(WorldQueryService::from_path(Some(format!("/{}", "a".repeat(104)).into())).is_err());
    assert!(parse(&catalog()).is_ok());
    let mut absent_world = catalog();
    if let Some(fields) = absent_world.as_object_mut() {
        fields.remove("world");
    }
    assert!(parse(&absent_world).is_err());
    let mut value = catalog();
    value["blocks"] = json!(vec!["minecraft:stone"; 4097]);
    assert!(parse(&value).is_err());
    value = catalog();
    value["world"] = json!("minecraft:overworld");
    assert!(parse(&value).is_err());
    value = catalog();
    value["worlds"][0]["map_id"] = json!("other_world");
    assert!(parse(&value).is_err());
    assert!(parse(&json!({"error":"unavailable"})).is_err());
    assert!(world_wire::parse(&serde_json::to_vec(&catalog())?, &MapQuery::Catalog).is_err());
    Ok(())
}

#[tokio::test]
async fn socket_roundtrip_is_bounded_and_requests_cool_down() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("cyhdev-world-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService::from_path(Some(path.clone()))?;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = vec![0u8; b"{\"kind\":\"catalog\"}\n".len()];
        stream.read_exact(&mut request).await?;
        assert_eq!(request, b"{\"kind\":\"catalog\"}\n");
        let mut reply = serde_json::to_vec(&catalog())?;
        reply.push(b'\n');
        stream.write_all(&reply).await?;
        Ok::<_, anyhow::Error>(())
    });
    assert_eq!(service.query(MapQuery::Catalog).await?.worlds.len(), 1);
    assert!(matches!(
        service.query(MapQuery::Catalog).await,
        Err(MapError::Busy)
    ));
    server.await??;
    tokio::fs::remove_file(&path).await?;
    Ok(())
}

#[test]
fn area_completeness_and_positions_are_validated() -> anyhow::Result<()> {
    use crate::features::minecraft::domain::map_query::Region;
    let query = MapQuery::Area {
        world: "minecraft:overworld".into(),
        region: Region {
            chunk_x: 0,
            chunk_z: 0,
            width: 1,
            height: 1,
        },
        y: None,
    };
    let mut value = catalog();
    value["kind"] = json!("area");
    value["world"] = json!("minecraft:overworld");
    value["worlds"] = json!([]);
    value["blocks"] = json!([]);
    let parse_area = |value: &serde_json::Value| {
        let bytes = format!("{value}\n");
        world_wire::parse(bytes.as_bytes(), &query)
    };
    assert!(parse_area(&value).is_err());
    value["missing_chunks"] = json!(1);
    assert!(parse_area(&value).is_ok());
    value["missing_chunks"] = json!(0);
    value["scanned_chunks"] = json!(1);
    assert!(parse_area(&value).is_err());
    let cells: Vec<_> = (0..4)
        .flat_map(|x| {
            (0..4).map(move |z| json!({"x":x*4,"z":z*4,"y":64,"biome":"minecraft:plains"}))
        })
        .collect();
    value["cells"] = json!(cells);
    assert!(parse_area(&value).is_ok());
    value["cells"][0]["x"] = json!(16);
    assert!(parse_area(&value).is_err());
    value["cells"][0] = value["cells"][1].clone();
    assert!(parse_area(&value).is_err());
    Ok(())
}

#[tokio::test]
async fn socket_rejects_oversized_unterminated_and_disconnected_replies() -> anyhow::Result<()> {
    for reply in [
        vec![b' '; MAX_REPLY + 1],
        serde_json::to_vec(&catalog())?,
        Vec::new(),
    ] {
        let path = std::env::temp_dir().join(format!("cyhdev-world-{}.sock", uuid::Uuid::new_v4()));
        let listener = tokio::net::UnixListener::bind(&path)?;
        let service = WorldQueryService::from_path(Some(path.clone()))?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = vec![0u8; b"{\"kind\":\"catalog\"}\n".len()];
            stream.read_exact(&mut request).await?;
            // The client may close as soon as its size bound is exceeded.
            let _ = stream.write_all(&reply).await;
            Ok::<_, anyhow::Error>(())
        });
        assert!(matches!(
            service.query(MapQuery::Catalog).await,
            Err(MapError::Unavailable)
        ));
        server.await??;
        tokio::fs::remove_file(&path).await?;
        let disconnected = WorldQueryService::from_path(Some(path))?;
        assert!(matches!(
            disconnected.query(MapQuery::Catalog).await,
            Err(MapError::Unavailable)
        ));
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_query_preserves_the_full_operation_budget() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("cyhdev-world-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService::from_path(Some(path.clone()))?;
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = vec![0u8; b"{\"kind\":\"catalog\"}\n".len()];
        stream.read_exact(&mut request).await?;
        let _ = received_tx.send(());
        std::future::pending::<()>().await;
        Ok::<_, anyhow::Error>(())
    });
    let mut query = Box::pin(service.query(MapQuery::Catalog));
    tokio::select! {
        result = &mut query => { result?; anyhow::bail!("query completed before a response"); }
        received = received_rx => { received?; }
    }
    assert!(matches!(
        service.query(MapQuery::Catalog).await,
        Err(MapError::Busy)
    ));
    // Dropping the suspended read frees the operation slot but retains its deadline.
    drop(query);
    {
        let gate = service.gate.try_lock()?;
        assert!(gate.saturating_duration_since(Instant::now()) > Duration::from_secs(19));
    }
    assert!(matches!(
        service.query(MapQuery::Catalog).await,
        Err(MapError::Busy)
    ));
    server.abort();
    tokio::fs::remove_file(&path).await?;
    Ok(())
}

#[tokio::test]
async fn stalled_peer_is_rejected_at_the_operation_deadline() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("cyhdev-world-{}.sock", uuid::Uuid::new_v4()));
    let listener = tokio::net::UnixListener::bind(&path)?;
    let service = WorldQueryService::from_path(Some(path.clone()))?;
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await?;
        std::future::pending::<()>().await;
        Ok::<_, anyhow::Error>(())
    });
    let start = Instant::now();
    assert!(matches!(
        service.query(MapQuery::Catalog).await,
        Err(MapError::Unavailable)
    ));
    assert!(start.elapsed() >= Duration::from_secs(20));
    assert!(start.elapsed() < Duration::from_secs(22));
    assert!(matches!(
        service.query(MapQuery::Catalog).await,
        Err(MapError::Busy)
    ));
    server.abort();
    tokio::fs::remove_file(&path).await?;
    Ok(())
}
