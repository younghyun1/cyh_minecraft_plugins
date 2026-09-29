//! Versioned palette tiles preserve exact hover values without JSON numeric arrays.

use crate::error::Error as MapError;
use crate::types::{MinecraftSeedPreset, MinecraftSeedTile};

pub const CONTENT_TYPE: &str = "application/vnd.cyhdev.biome-tile";
pub const MAX_BYTES: usize = 40_000;

/// Choose bit packing or variable-length runs before ordinary HTTP compression.
pub fn encode(tile: &MinecraftSeedTile) -> Result<Vec<u8>, MapError> {
    validate(tile)?;
    let count = tile.palette.len() as u16;
    let bits = (u16::BITS - count.leading_zeros()) as u8;
    let mut symbols = Vec::with_capacity(4096);
    for index in &tile.indices {
        symbols.push(match index {
            Some(index) if *index < count => index + 1,
            None => 0,
            _ => return Err(MapError::Unavailable),
        });
    }
    let packed = pack(&symbols, bits);
    let runs = runs(&symbols, bits);
    let (codec, payload) = if runs.len() < packed.len() {
        (1, runs)
    } else {
        (0, packed)
    };
    let world = match tile.world.as_str() {
        "minecraft:overworld" => 0,
        "minecraft:the_nether" => 1,
        "minecraft:the_end" => 2,
        _ => return Err(MapError::Unavailable),
    };
    let preset = match tile.preset {
        MinecraftSeedPreset::Default => 0,
        MinecraftSeedPreset::LargeBiomes => 1,
        MinecraftSeedPreset::Nether => 2,
        MinecraftSeedPreset::End => 3,
    };
    let ttl = tile
        .expires_at_ms
        .checked_sub(tile.sampled_at_ms)
        .filter(|ttl| (1..=15_000).contains(ttl))
        .ok_or(MapError::Unavailable)? as u16;
    let y = i16::try_from(tile.y).map_err(|_| MapError::Unavailable)?;
    let sampled = u64::try_from(tile.sampled_at_ms).map_err(|_| MapError::Unavailable)?;
    let mut bytes = Vec::with_capacity(256 + payload.len());
    bytes.extend_from_slice(b"CYBM");
    bytes.extend_from_slice(&[1, codec, world, preset, tile.level]);
    bytes.extend_from_slice(&y.to_le_bytes());
    bytes.extend_from_slice(&tile.tile_x.to_le_bytes());
    bytes.extend_from_slice(&tile.tile_z.to_le_bytes());
    bytes.extend_from_slice(&sampled.to_le_bytes());
    bytes.extend_from_slice(&ttl.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    string(&mut bytes, &tile.profile_epoch)?;
    string(&mut bytes, &tile.generator_revision)?;
    for name in &tile.palette {
        string(&mut bytes, name)?;
    }
    bytes.extend_from_slice(&payload);
    if bytes.len() > MAX_BYTES {
        return Err(MapError::Unavailable);
    }
    Ok(bytes)
}

/// Reject metadata that would change when the browser derives canonical geometry.
pub(crate) fn validate(tile: &MinecraftSeedTile) -> Result<(), MapError> {
    if tile.indices.len() != 4096
        || tile.palette.len() > 256
        || tile.level > 12
        || tile.width != 64
        || tile.height != 64
    {
        return Err(MapError::Unavailable);
    }
    let (min_y, max_y) = match (tile.world.as_str(), &tile.preset) {
        (
            "minecraft:overworld",
            MinecraftSeedPreset::Default | MinecraftSeedPreset::LargeBiomes,
        ) => (-64, 319),
        ("minecraft:the_nether", MinecraftSeedPreset::Nether)
        | ("minecraft:the_end", MinecraftSeedPreset::End) => (0, 255),
        _ => return Err(MapError::Unavailable),
    };
    let step = 4_u32 << tile.level;
    let span = i64::from(step) * 64;
    let x = i64::from(tile.tile_x) * span;
    let z = i64::from(tile.tile_z) * span;
    if tile.step != step
        || i64::from(tile.min_x) != x
        || i64::from(tile.min_z) != z
        || !(min_y..=max_y).contains(&tile.y)
        || [x, z]
            .into_iter()
            .any(|origin| origin >= 30_000_000 || origin + span <= -30_000_000)
        || tile.sampled_at_ms < 0
        || tile.expires_at_ms > 9_007_199_254_740_991
        || !matches!(
            tile.expires_at_ms.checked_sub(tile.sampled_at_ms),
            Some(1..=15_000)
        )
        || !(16..=128).contains(&tile.profile_epoch.len())
        || !tile
            .profile_epoch
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || tile.palette.iter().any(|name| !identifier(name))
    {
        return Err(MapError::Unavailable);
    }
    Ok(())
}

fn identifier(name: &str) -> bool {
    let Some((namespace, path)) = name.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && name.len() <= 128
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/".contains(&b))
}

fn string(bytes: &mut Vec<u8>, value: &str) -> Result<(), MapError> {
    if value.is_empty() || value.len() > 128 {
        return Err(MapError::Unavailable);
    }
    bytes.push(value.len() as u8);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn pack(symbols: &[u16], bits: u8) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((symbols.len() * usize::from(bits)).div_ceil(8));
    let (mut buffer, mut used) = (0_u32, 0_u8);
    for symbol in symbols {
        buffer |= u32::from(*symbol) << used;
        used += bits;
        while used >= 8 {
            bytes.push(buffer as u8);
            buffer >>= 8;
            used -= 8;
        }
    }
    if used > 0 {
        bytes.push(buffer as u8);
    }
    bytes
}

fn runs(symbols: &[u16], bits: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut offset = 0;
    while offset < symbols.len() {
        let symbol = symbols[offset];
        let length = symbols[offset..]
            .iter()
            .take_while(|value| **value == symbol)
            .count();
        let mut value = (((length - 1) as u32) << bits) | u32::from(symbol);
        while value >= 128 {
            bytes.push(value as u8 | 128);
            value >>= 7;
        }
        bytes.push(value as u8);
        offset += length;
    }
    bytes
}

#[cfg(test)]
#[path = "seed_tile_binary_tests.rs"]
mod tests;
