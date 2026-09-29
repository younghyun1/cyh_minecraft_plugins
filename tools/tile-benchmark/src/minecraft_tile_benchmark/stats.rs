//! Repeated development measurements and a complete per-tile size receipt.

use super::{
    formats::{self, Format, Result, Transport},
    samples::Sample,
};
use serde::Serialize;
use std::{hint::black_box, time::Instant};

pub const REPETITIONS: usize = 7;

#[derive(Serialize)]
pub struct Summary {
    format: String,
    transport: String,
    total_bytes: usize,
    median_bytes: usize,
    p95_bytes: usize,
    encode_median_us: f64,
    encode_p95_us: f64,
    decode_median_us: f64,
    decode_p95_us: f64,
    tiles: Vec<TileSize>,
}

#[derive(Serialize)]
struct TileSize {
    name: String,
    bytes: usize,
}

/// Sampling is excluded; compare only equivalent transport bodies and hover decoding.
pub fn measure(samples: &[Sample], format: Format, transport: Transport) -> Result<Summary> {
    let mut sizes = Vec::with_capacity(samples.len());
    let mut tiles = Vec::with_capacity(samples.len());
    let mut encode_times = Vec::with_capacity(samples.len() * REPETITIONS);
    let mut decode_times = Vec::with_capacity(samples.len() * REPETITIONS);
    for sample in samples {
        let warm = formats::encode(&sample.tile, format, transport)?;
        let decoded = formats::decode(&warm, format, transport)?;
        if decoded.palette != sample.tile.palette || decoded.indices != sample.tile.indices {
            return Err(format!("{format:?}/{transport:?} changed {}", sample.name).into());
        }
        sizes.push(warm.len());
        tiles.push(TileSize {
            name: sample.name.clone(),
            bytes: warm.len(),
        });
        for _ in 0..REPETITIONS {
            let start = Instant::now();
            let bytes = black_box(formats::encode(black_box(&sample.tile), format, transport)?);
            encode_times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
            let start = Instant::now();
            black_box(formats::decode(black_box(&bytes), format, transport)?);
            decode_times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        }
    }
    sizes.sort_unstable();
    encode_times.sort_by(f64::total_cmp);
    decode_times.sort_by(f64::total_cmp);
    Ok(Summary {
        format: format!("{format:?}"),
        transport: format!("{transport:?}"),
        total_bytes: sizes.iter().sum(),
        median_bytes: *percentile(&sizes, 50)?,
        p95_bytes: *percentile(&sizes, 95)?,
        encode_median_us: *percentile(&encode_times, 50)?,
        encode_p95_us: *percentile(&encode_times, 95)?,
        decode_median_us: *percentile(&decode_times, 50)?,
        decode_p95_us: *percentile(&decode_times, 95)?,
        tiles,
    })
}

fn percentile<T>(values: &[T], percent: usize) -> Result<&T> {
    values
        .get((values.len() * percent).div_ceil(100).saturating_sub(1))
        .ok_or_else(|| "empty benchmark corpus".into())
}
