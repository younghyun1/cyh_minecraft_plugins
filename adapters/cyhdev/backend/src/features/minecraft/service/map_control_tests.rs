use super::*;
use tokio::net::UnixListener;

#[test]
fn status_distinguishes_unknown_hidden_and_visible() -> anyhow::Result<()> {
    let id = Uuid::new_v4();
    assert_eq!(
        parse_status(&format!("OK\n{id} 1\n"))?.get(&id),
        Some(&true)
    );
    assert_eq!(
        parse_status(&format!("OK\n{id} 0\n"))?.get(&id),
        Some(&false)
    );
    assert!(parse_status("OK\n")?.is_empty());
    for reply in [
        "ERROR\n".to_owned(),
        format!("OK\n{id} 0"),
        format!("OK\n{id} 2\n"),
        format!("OK\n{id} 0\n{id} 1\n"),
        "OK\nnot-a-uuid 0\n".to_owned(),
    ] {
        assert!(parse_status(&reply).is_err());
    }
    Ok(())
}

async fn mock(response: String, hidden: bool) -> anyhow::Result<anyhow::Result<()>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("map.sock");
    let listener = UnixListener::bind(&path)?;
    let id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut request = Vec::new();
        loop {
            let byte = stream.read_u8().await?;
            request.push(byte);
            if byte == b'\n' {
                break;
            }
            anyhow::ensure!(request.len() < 64, "Oversized request");
        }
        assert_eq!(
            String::from_utf8(request)?,
            format!("{} {id}\n", if hidden { "HIDE" } else { "SHOW" })
        );
        // An oversized response may be rejected while the peer is still writing.
        let _ = stream.write_all(response.as_bytes()).await;
        Ok::<_, anyhow::Error>(())
    });
    let result = MapControl { path }.set_hidden(id, hidden).await;
    task.await??;
    Ok(result)
}

#[tokio::test]
async fn mutations_require_exact_acknowledgement() -> anyhow::Result<()> {
    mock("OK\n".into(), true).await??;
    mock("OK\n".into(), false).await??;
    for response in [
        "ERROR\n".into(),
        "OK".into(),
        "OK\nextra\n".into(),
        "x".repeat(MAX_RESPONSE + 1),
    ] {
        assert!(mock(response, true).await?.is_err());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires the Paper map control plugin; read-only"]
async fn inspect_live_map_control() -> anyhow::Result<()> {
    let control = MapControl::from_environment()?
        .ok_or_else(|| anyhow::anyhow!("Missing map control configuration"))?;
    control.status().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires an isolated Paper instance with no online players"]
async fn inspect_empty_plugin_rejections() -> anyhow::Result<()> {
    let control = MapControl::from_environment()?
        .ok_or_else(|| anyhow::anyhow!("Missing map control configuration"))?;
    anyhow::ensure!(
        control.status().await?.is_empty(),
        "Use an isolated server with no players"
    );
    assert!(control.set_hidden(Uuid::new_v4(), true).await.is_err());
    for request in ["STOP\n".to_owned(), "HIDE @a\n".to_owned(), "x".repeat(65)] {
        assert_eq!(control.call(&request).await?, "ERROR\n");
    }
    // A partial frame expires on the plugin and does not prevent the following request.
    assert_ne!(control.call("STATUS").await?, "OK\n");
    assert!(control.status().await?.is_empty());
    Ok(())
}
