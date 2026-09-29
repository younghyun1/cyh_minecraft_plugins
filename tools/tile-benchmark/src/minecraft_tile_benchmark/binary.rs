//! Benchmark decoder for the production CYBM transport.

use super::formats::{Decoded, Result};

/// Decode the complete header before reconstructing exact optional palette indices.
pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    if bytes.len() < 33 || &bytes[..4] != b"CYBM" || !matches!(bytes[4], 1 | 2) {
        return Err("invalid binary benchmark tile".into());
    }
    let y = i16::from_le_bytes([bytes[9], bytes[10]]);
    if y == i16::MIN && (bytes[4] != 2 || bytes[6] != 0) {
        return Err("invalid surface benchmark tile".into());
    }
    let codec = bytes[5];
    let count = u16::from_le_bytes([bytes[29], bytes[30]]) as usize;
    let bits = (usize::BITS - count.leading_zeros()) as usize;
    let mut offset = 31;
    let _epoch = string(bytes, &mut offset)?;
    let _revision = string(bytes, &mut offset)?;
    let palette = (0..count)
        .map(|_| string(bytes, &mut offset))
        .collect::<Result<Vec<_>>>()?;
    let mut indices = Vec::with_capacity(4096);
    match codec {
        0 => {
            let (mut reservoir, mut used) = (0_u32, 0_usize);
            for _ in 0..4096 {
                while used < bits {
                    reservoir |= u32::from(*bytes.get(offset).ok_or("short packed tile")?) << used;
                    offset += 1;
                    used += 8;
                }
                let value = reservoir & ((1 << bits) - 1);
                reservoir >>= bits;
                used -= bits;
                indices.push(index(value, count)?);
            }
        }
        1 => {
            while indices.len() < 4096 {
                let mut value = 0_u32;
                let mut shift = 0;
                loop {
                    let byte = *bytes.get(offset).ok_or("short run tile")?;
                    offset += 1;
                    value |= u32::from(byte & 127) << shift;
                    if byte < 128 {
                        break;
                    }
                    shift += 7;
                    if shift > 28 {
                        return Err("invalid run tile".into());
                    }
                }
                let index = index(value & ((1 << bits) - 1), count)?;
                let length = ((value >> bits) + 1) as usize;
                if indices.len() + length > 4096 {
                    return Err("oversized run tile".into());
                }
                indices.resize(indices.len() + length, index);
            }
        }
        _ => return Err("unsupported binary tile codec".into()),
    }
    Ok(Decoded { palette, indices })
}

fn index(symbol: u32, count: usize) -> Result<Option<u16>> {
    match symbol {
        0 => Ok(None),
        value if value as usize <= count => Ok(Some((value - 1) as u16)),
        _ => Err("invalid palette index".into()),
    }
}

fn string(bytes: &[u8], offset: &mut usize) -> Result<String> {
    let length = usize::from(*bytes.get(*offset).ok_or("missing tile string")?);
    *offset += 1;
    let value = bytes
        .get(*offset..*offset + length)
        .ok_or("short tile string")?;
    *offset += length;
    Ok(std::str::from_utf8(value)?.into())
}
