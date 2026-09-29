//! Development-only comparison of seed-tile transport formats with real synthetic terrain.

#[path = "minecraft_tile_benchmark/mod.rs"]
mod benchmark;

use benchmark::{
    formats::{Format, Transport},
    samples, stats,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(debug_assertions) {
        return Err("run this diagnostic with the development profile".into());
    }
    let tiles = samples::generate().await?;
    if let Some(directory) = std::env::args().nth(2) {
        benchmark::browser_samples::export(std::path::Path::new(&directory), &tiles).await?;
    }
    let mut results = Vec::new();
    for format in [
        Format::Json,
        Format::Binary,
        Format::NativePng,
        Format::PngFast,
        Format::PngHigh,
    ] {
        for transport in [Transport::Identity, Transport::Gzip, Transport::Zstd] {
            results.push(stats::measure(&tiles, format, transport)?);
        }
    }
    let report = serde_json::json!({
        "profile": "development", "arch": std::env::consts::ARCH,
        "generator": minecraft_seed::GENERATOR_REVISION,
        "samples": tiles.len(), "repetitions": stats::REPETITIONS,
        "description": "Synthetic seed 1, all four presets, levels 0/3/8, origin and negative terrain; full, masked and empty tiles. NativePng calls the production endpoint codec: balanced PNG with actual biome colors and an uncompressed cyBM chunk preserving exact hover indices. Generation, HTTP framing/network and browser rendering excluded. JSON uses DTO without its response envelope. gzip level 6; zstd level 3. HTTP image responses ordinarily use PNG native compression only.",
        "results": results,
    });
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, &json)?;
    }
    println!("{json}");
    Ok(())
}
