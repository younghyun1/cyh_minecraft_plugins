//! Equivalent lossless bodies with optional ordinary HTTP content compression.

use minecraft_tile_codec::{seed_tile_binary, types::MinecraftSeedTile};
use serde::Deserialize;
use std::io::{Read, Write};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug)]
pub enum Format {
    Json,
    Binary,
    NativePng,
    PngFast,
    PngHigh,
}
#[derive(Clone, Copy, Debug)]
pub enum Transport {
    Identity,
    Gzip,
    Zstd,
}

#[derive(Deserialize)]
pub struct Decoded {
    pub palette: Vec<String>,
    pub indices: Vec<Option<u16>>,
}

/// Encode all metadata and exact per-cell biome identities, including null cells.
pub fn encode(tile: &MinecraftSeedTile, format: Format, transport: Transport) -> Result<Vec<u8>> {
    let raw = match format {
        Format::Json => serde_json::to_vec(tile)?,
        Format::Binary => seed_tile_binary::encode(tile)?,
        Format::NativePng => super::png::encode(tile)?,
        Format::PngFast => super::png::encode_with_compression(tile, png::Compression::Fast)?,
        Format::PngHigh => super::png::encode_with_compression(tile, png::Compression::High)?,
    };
    match transport {
        Transport::Identity => Ok(raw),
        Transport::Gzip => {
            let mut writer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
            writer.write_all(&raw)?;
            Ok(writer.finish()?)
        }
        Transport::Zstd => Ok(zstd::stream::encode_all(raw.as_slice(), 3)?),
    }
}

/// Decode palette and indices for hover, including content decompression costs.
pub fn decode(bytes: &[u8], format: Format, transport: Transport) -> Result<Decoded> {
    let raw = match transport {
        Transport::Identity => bytes.to_vec(),
        Transport::Gzip => {
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .take(100_000)
                .read_to_end(&mut raw)?;
            raw
        }
        Transport::Zstd => zstd::bulk::decompress(bytes, 100_000)?,
    };
    match format {
        Format::Json => Ok(serde_json::from_slice(&raw)?),
        Format::Binary => super::binary::decode(&raw),
        Format::NativePng | Format::PngFast | Format::PngHigh => super::png::decode(&raw),
    }
}
