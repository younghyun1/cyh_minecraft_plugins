//! Bounded same-user IPC to the Paper plugin; no public listener or console commands.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};
use uuid::Uuid;

const MAX_RESPONSE: usize = 39_003; // OK newline, then at most 1000 UUID/boolean rows.

pub struct MapControl {
    path: PathBuf,
}

impl MapControl {
    /// A missing socket disables map controls without disabling other Minecraft operations.
    pub fn from_environment() -> anyhow::Result<Option<Self>> {
        match std::env::var_os("MINECRAFT_MAP_CONTROL_SOCKET") {
            None => Ok(None),
            Some(path) => {
                let path = PathBuf::from(path);
                anyhow::ensure!(path.is_absolute(), "Map control socket must be absolute");
                Ok(Some(Self { path }))
            }
        }
    }

    pub async fn status(&self) -> anyhow::Result<BTreeMap<Uuid, bool>> {
        parse_status(&self.call("STATUS\n").await?)
    }

    pub async fn set_hidden(&self, id: Uuid, hidden: bool) -> anyhow::Result<()> {
        let action = if hidden { "HIDE" } else { "SHOW" };
        let response = self.call(&format!("{action} {id}\n")).await?;
        anyhow::ensure!(
            response == "OK\n",
            "Map control did not acknowledge mutation"
        );
        Ok(())
    }

    /// One connection per operation; cancellation cannot result in an automatic retry.
    async fn call(&self, request: &str) -> anyhow::Result<String> {
        tokio::time::timeout(Duration::from_secs(4), async {
            let mut stream = UnixStream::connect(&self.path).await?;
            stream.write_all(request.as_bytes()).await?;
            let mut response = Vec::new();
            stream
                .take((MAX_RESPONSE + 1) as u64)
                .read_to_end(&mut response)
                .await?;
            anyhow::ensure!(
                response.len() <= MAX_RESPONSE,
                "Map control response too large"
            );
            Ok::<_, anyhow::Error>(String::from_utf8(response)?)
        })
        .await?
    }
}

fn parse_status(response: &str) -> anyhow::Result<BTreeMap<Uuid, bool>> {
    anyhow::ensure!(response.ends_with('\n'), "Incomplete map status");
    let mut lines = response.lines();
    anyhow::ensure!(lines.next() == Some("OK"), "Map control unavailable");
    let mut players = BTreeMap::new();
    for line in lines {
        let (id, hidden) = line
            .split_once(' ')
            .ok_or_else(|| anyhow::anyhow!("Invalid map status"))?;
        let id: Uuid = id.parse()?;
        let hidden = match hidden {
            "1" => true,
            "0" => false,
            _ => anyhow::bail!("Invalid map visibility"),
        };
        anyhow::ensure!(
            players.len() < 1000 && players.insert(id, hidden).is_none(),
            "Duplicate or excessive map players"
        );
    }
    Ok(players)
}

#[cfg(test)]
#[path = "map_control_tests.rs"]
mod tests;
