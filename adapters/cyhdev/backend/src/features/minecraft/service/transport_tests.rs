use super::*;

/// Read-only deployment check; configuration may point through an SSH loopback tunnel.
#[tokio::test]
#[ignore = "requires a running Minecraft management endpoint and secret"]
async fn inspect_live_management() -> anyhow::Result<()> {
    let transport = ManagementTransport::from_environment()?
        .ok_or_else(|| anyhow::anyhow!("Missing management configuration"))?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let schema = transport.call("rpc.discover", json!([])).await?;
        let methods = schema["methods"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing RPC schema"))?;
        for required in [
            "minecraft:server/system_message",
            "minecraft:server/save",
            "minecraft:server/stop",
            "minecraft:players/kick",
            "minecraft:allowlist/add",
            "minecraft:allowlist/remove",
            "minecraft:serversettings/use_allowlist/set",
        ] {
            anyhow::ensure!(
                methods.iter().any(|m| m["name"] == required),
                "Missing required Minecraft method"
            );
        }
        let players = transport.call("minecraft:players", json!([])).await?;
        let whitelist = transport.call("minecraft:allowlist", json!([])).await?;
        let enabled = transport
            .call("minecraft:serversettings/use_allowlist", json!([]))
            .await?;
        anyhow::ensure!(
            players.is_array() && whitelist.is_array() && enabled.is_boolean(),
            "Unexpected management response shape"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}

// Tungstenite fixes the callback error type to an HTTP response; it cannot be boxed here.
#[allow(clippy::result_large_err)]
async fn mock_call(response: Value) -> anyhow::Result<anyhow::Result<Value>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_hdr_async(
            stream,
            |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                assert_eq!(
                    request.headers()["authorization"],
                    format!("Bearer {}", "a".repeat(40))
                );
                Ok(response)
            },
        )
        .await?;
        let request = socket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Missing request"))??;
        let request: Value = serde_json::from_str(request.to_text()?)?;
        assert_eq!(request["method"], "minecraft:players");
        socket
            .send(Message::Text(
                json!({"jsonrpc":"2.0","method":"notification"})
                    .to_string()
                    .into(),
            ))
            .await?;
        socket
            .send(Message::Text(response.to_string().into()))
            .await?;
        Ok::<_, anyhow::Error>(())
    });
    let transport = ManagementTransport {
        port,
        secret: Zeroizing::new("a".repeat(40)),
    };
    let result = transport.call("minecraft:players", json!([])).await;
    server.await??;
    Ok(result)
}

#[tokio::test]
async fn notifications_do_not_consume_the_response() -> anyhow::Result<()> {
    assert_eq!(
        mock_call(json!({"jsonrpc":"2.0","id":1,"result":[]})).await??,
        json!([])
    );
    Ok(())
}

#[tokio::test]
async fn errors_and_malformed_responses_are_rejected() -> anyhow::Result<()> {
    for response in [
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-1}}),
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"2.0","id":1,"result":{"error":{}}}),
        json!({"jsonrpc":"1.0","id":1,"result":true}),
    ] {
        assert!(mock_call(response).await?.is_err());
    }
    Ok(())
}

#[tokio::test]
async fn oversized_responses_are_rejected() -> anyhow::Result<()> {
    assert!(
        mock_call(json!({"jsonrpc":"2.0","id":1,"result":"x".repeat(256 * 1024)}))
            .await?
            .is_err()
    );
    Ok(())
}
