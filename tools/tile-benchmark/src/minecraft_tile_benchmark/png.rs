//! Benchmark the actual PNG endpoint codec and its native-decoder workload.

use super::formats::{Decoded, Result};
use minecraft_tile_codec::{seed_tile_png, types::MinecraftSeedTile};
use std::io::Cursor;

pub fn encode(tile: &MinecraftSeedTile) -> Result<Vec<u8>> {
    Ok(seed_tile_png::encode(tile)?)
}

pub fn encode_with_compression(
    tile: &MinecraftSeedTile,
    compression: png::Compression,
) -> Result<Vec<u8>> {
    Ok(seed_tile_png::encode_with_compression(tile, compression)?)
}

/// The image is inflated by the PNG library; identity metadata needs no inflation.
pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info()?;
    let size = reader
        .output_buffer_size()
        .ok_or("PNG output size unavailable")?;
    if size > 16384 {
        return Err("oversized PNG benchmark tile".into());
    }
    let mut pixels = vec![0_u8; size];
    reader.next_frame(&mut pixels)?;
    let mut offset = 8;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into()?) as usize;
        let body = bytes
            .get(offset + 8..offset + 8 + length)
            .ok_or("short PNG chunk")?;
        if bytes[offset + 4..offset + 8] == seed_tile_png::METADATA_CHUNK {
            return super::binary::decode(body);
        }
        offset += length + 12;
    }
    Err("PNG metadata missing".into())
}
