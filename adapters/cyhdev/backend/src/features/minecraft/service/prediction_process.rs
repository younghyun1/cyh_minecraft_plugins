//! One bounded child invocation isolates generator failures and permits cancellation.

use super::prediction_wire::{Context, Preset};
use crate::features::minecraft::domain::{
    map_query::identifier,
    prediction::{GENERATOR_REVISION, PredictedBiome, PredictionQuery},
};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path, process::Stdio};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_REPLY: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    generator_revision: String,
    large_biomes: bool,
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    x: i32,
    z: i32,
    biome: String,
}

/// Seed material goes through stdin, never arguments, environment, or subprocess logs.
pub(super) async fn run(
    executable: &Path,
    context: &Context,
    query: &PredictionQuery,
) -> anyhow::Result<Vec<PredictedBiome>> {
    let region = query.region();
    let large_biomes = context.preset == Preset::LargeBiomes;
    let mut input = serde_json::to_vec(&serde_json::json!({
        "seed":context.seed,"large_biomes":large_biomes,"y":query.y,
        "min_x":region.chunk_x * 16,"min_z":region.chunk_z * 16,
        "width":32,"height":32,"step":4,
    }))?;
    input.push(b'\n');
    anyhow::ensure!(input.len() <= 4096, "Prediction input too large");
    let mut child = tokio::process::Command::new(executable)
        .env_clear()
        .env("RAYON_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Missing worker input"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Missing worker output"))?;
    let send = async {
        stdin.write_all(&input).await?;
        stdin.shutdown().await?;
        drop(stdin);
        Ok::<_, anyhow::Error>(())
    };
    let receive = async {
        let mut output = Vec::new();
        stdout
            .take((MAX_REPLY + 1) as u64)
            .read_to_end(&mut output)
            .await?;
        anyhow::ensure!(output.len() <= MAX_REPLY, "Prediction output too large");
        Ok::<_, anyhow::Error>(output)
    };
    let (_, output) = tokio::try_join!(send, receive)?;
    anyhow::ensure!(child.wait().await?.success(), "Prediction worker failed");
    parse(&output, query, large_biomes)
}

pub(super) fn parse(
    output: &[u8],
    query: &PredictionQuery,
    large_biomes: bool,
) -> anyhow::Result<Vec<PredictedBiome>> {
    anyhow::ensure!(
        output.last() == Some(&b'\n'),
        "Incomplete prediction output"
    );
    let reply: Reply = serde_json::from_slice(output)?;
    anyhow::ensure!(
        reply.generator_revision == GENERATOR_REVISION && reply.large_biomes == large_biomes,
        "Wrong prediction engine or preset"
    );
    let region = query.region();
    anyhow::ensure!(
        reply.cells.len() == 1024
            && reply.cells.iter().all(|cell| {
                region.contains(cell.x, cell.z)
                    && cell.x.rem_euclid(4) == 0
                    && cell.z.rem_euclid(4) == 0
                    && identifier(&cell.biome)
            }),
        "Invalid prediction cells"
    );
    anyhow::ensure!(
        reply
            .cells
            .iter()
            .map(|cell| (cell.x, cell.z))
            .collect::<BTreeSet<_>>()
            .len()
            == 1024,
        "Duplicate prediction cells"
    );
    Ok(reply
        .cells
        .into_iter()
        .map(|cell| PredictedBiome {
            x: cell.x,
            z: cell.z,
            biome: cell.biome,
        })
        .collect())
}
