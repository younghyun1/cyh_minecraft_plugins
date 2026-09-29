//! Native image tiles with exact hover identities in a bounded private PNG chunk.

use super::{seed_tile_binary, seed_tile_colors, seed_tile_dto::MinecraftSeedTile};
use crate::features::minecraft::error::MapError;

pub const CONTENT_TYPE: &str = "image/png";
pub const MAX_BYTES: usize = 65_536;
/// Ancillary, private, reserved-bit-correct and unsafe to retain after image edits.
pub const METADATA_CHUNK: [u8; 4] = *b"cyBM";

/// Preserve the complete CYBM frame for hover; the browser decodes pixels natively.
pub fn encode(tile: &MinecraftSeedTile) -> Result<Vec<u8>, MapError> {
    encode_with_compression(tile, png::Compression::Balanced)
}

/// Keep compression comparisons on the exact production metadata and pixel path.
pub fn encode_with_compression(
    tile: &MinecraftSeedTile,
    compression: png::Compression,
) -> Result<Vec<u8>, MapError> {
    let metadata = seed_tile_binary::encode(tile)?;
    let colors = tile
        .palette
        .iter()
        .map(|name| seed_tile_colors::color(name))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(metadata.len() + 4096);
    {
        let mut encoder = png::Encoder::new(&mut output, 64, 64);
        encoder.set_compression(compression);
        let pixels = if tile.palette.len() <= 255 {
            let count = tile.palette.len() + 1;
            let depth = match count {
                1..=2 => png::BitDepth::One,
                3..=4 => png::BitDepth::Two,
                5..=16 => png::BitDepth::Four,
                _ => png::BitDepth::Eight,
            };
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(depth);
            let mut palette = vec![0_u8; count * 3];
            for (index, color) in colors.iter().enumerate() {
                palette[(index + 1) * 3..(index + 2) * 3].copy_from_slice(color);
            }
            encoder.set_palette(palette);
            encoder.set_trns(vec![0_u8]);
            let bits = depth as usize;
            let mut pixels = vec![0_u8; 4096 * bits / 8];
            for (index, symbol) in tile.indices.iter().enumerate() {
                let value = symbol.map_or(0, |value| value + 1) as u8;
                pixels[index * bits / 8] |= value << (8 - bits - (index * bits % 8));
            }
            pixels
        } else {
            // PNG palettes cannot hold 256 biomes plus transparent null. RGBA
            // covers that valid CYBM case without changing identity metadata.
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut pixels = vec![0_u8; 4096 * 4];
            for (index, symbol) in tile.indices.iter().enumerate() {
                if let Some(symbol) = symbol {
                    let pixel = &mut pixels[index * 4..index * 4 + 4];
                    pixel[..3].copy_from_slice(&colors[usize::from(*symbol)]);
                    pixel[3] = 255;
                }
            }
            pixels
        };
        let mut writer = encoder.write_header().map_err(|_| MapError::Unavailable)?;
        writer
            .write_chunk(png::chunk::ChunkType(METADATA_CHUNK), &metadata)
            .map_err(|_| MapError::Unavailable)?;
        writer
            .write_image_data(&pixels)
            .map_err(|_| MapError::Unavailable)?;
        writer.finish().map_err(|_| MapError::Unavailable)?;
    }
    if output.len() > MAX_BYTES {
        return Err(MapError::Unavailable);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "seed_tile_png_tests.rs"]
mod tests;
