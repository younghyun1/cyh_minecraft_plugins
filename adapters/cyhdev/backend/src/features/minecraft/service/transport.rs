//! Bounded, authenticated JSON-RPC calls over loopback only.

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    client_async_with_config,
    tungstenite::{
        client::IntoClientRequest,
        protocol::{Message, WebSocketConfig},
    },
};
use zeroize::Zeroizing;

/// Configuration intentionally accepts a port, never an arbitrary remote host.
pub struct ManagementTransport {
    port: u16,
    secret: Zeroizing<String>,
}

impl ManagementTransport {
    /// Missing configuration disables controls; partial or invalid configuration fails startup.
    pub fn from_environment() -> anyhow::Result<Option<Self>> {
        match (
            std::env::var("MINECRAFT_MANAGEMENT_PORT"),
            std::env::var("MINECRAFT_MANAGEMENT_SECRET"),
        ) {
            (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => Ok(None),
            (Ok(port), Ok(secret)) => {
                let port: u16 = port.parse()?;
                anyhow::ensure!(
                    port > 0
                        && secret.len() == 40
                        && secret.bytes().all(|b| b.is_ascii_alphanumeric()),
                    "Invalid Minecraft management configuration"
                );
                Ok(Some(Self {
                    port,
                    secret: Zeroizing::new(secret),
                }))
            }
            _ => anyhow::bail!("Set both Minecraft management port and secret"),
        }
    }

    /// A fresh connection isolates cancellation and ambiguous writes; calls are never retried.
    pub async fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, self.port)).await?;
        stream.set_nodelay(true)?;
        let mut request = format!("ws://127.0.0.1:{}/", self.port).into_client_request()?;
        let mut authorization =
            format!("Bearer {}", self.secret.as_str()).parse::<axum::http::HeaderValue>()?;
        authorization.set_sensitive(true);
        request
            .headers_mut()
            .insert(axum::http::header::AUTHORIZATION, authorization);
        let config = WebSocketConfig::default()
            .max_message_size(Some(256 * 1024))
            .max_frame_size(Some(256 * 1024));
        let (mut socket, _) = client_async_with_config(request, stream, Some(config)).await?;
        socket
            .send(Message::Text(
                json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))
            .await?;
        // Notifications can interleave with replies. Bound both their count and total call time.
        for _ in 0..64 {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let response: Value = serde_json::from_str(&text)?;
                    if response.get("id") != Some(&json!(1)) {
                        continue;
                    }
                    anyhow::ensure!(
                        response.get("jsonrpc") == Some(&json!("2.0")),
                        "Invalid JSON-RPC version"
                    );
                    anyhow::ensure!(
                        response.get("error").is_none(),
                        "Minecraft rejected management request"
                    );
                    let result = response
                        .get("result")
                        .ok_or_else(|| anyhow::anyhow!("Missing management result"))?;
                    anyhow::ensure!(
                        result.get("error").is_none(),
                        "Minecraft rejected management request"
                    );
                    return Ok(result.clone());
                }
                Some(Ok(Message::Ping(_))) => socket.flush().await?,
                Some(Ok(Message::Pong(_))) => {}
                Some(Err(_)) | None | Some(Ok(_)) => {
                    anyhow::bail!("Management connection ended without acknowledgement")
                }
            }
        }
        anyhow::bail!("Too many management notifications")
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
