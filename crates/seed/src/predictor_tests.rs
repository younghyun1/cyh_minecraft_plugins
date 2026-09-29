//! Persistent sampling parity, admission bounds, cancellation and development timings.

use super::*;
use crate::Dimension;
use std::{future::Future, task::Poll, time::Instant};

fn request(large_biomes: bool, x: i32, z: i32, step: u32) -> PredictionRequest {
    PredictionRequest {
        seed: 1,
        dimension: Dimension::Overworld,
        large_biomes,
        y: Some(64),
        min_x: x,
        min_z: z,
        width: 64,
        height: 64,
        step,
    }
}

#[tokio::test]
async fn persistent_sampler_matches_fresh_sampling_across_tiles_and_presets() -> Result<(), Error> {
    let predictor = Predictor::new()?;
    for (large, x, z, step, y) in [
        (false, -256, -512, 4, Some(64)),
        (false, 1024, -512, 8, None),
        (false, -256, -512, 4, Some(64)),
        (false, -253, -509, 3, Some(-63)),
        (false, -256, -512, 4, None),
        (false, -256, -512, 4, Some(319)),
        (true, 768, -1536, 4, None),
        (true, 768, -1536, 4, Some(64)),
        (false, 0, 0, 64, None),
    ] {
        let mut input = request(large, x, z, step);
        input.y = y;
        let expected = crate::predict(&input)?;
        let actual = predictor.predict(input).await?;
        assert_eq!(actual.cells.len(), 4096);
        for (actual, expected) in actual.cells.iter().zip(expected.cells) {
            assert_eq!(
                (actual.x, actual.z, &actual.biome),
                (expected.x, expected.z, &expected.biome)
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn persistent_sampler_rebuilds_when_dimensions_or_seed_change() -> Result<(), Error> {
    let predictor = Predictor::new()?;
    for (dimension, seed) in [
        (Dimension::Nether, 1),
        (Dimension::End, 1),
        (Dimension::Overworld, 1),
        (Dimension::End, -1),
        (Dimension::End, 1),
        (Dimension::Nether, 1),
    ] {
        let mut input = request(false, -4096, -4096, 128);
        input.dimension = dimension;
        input.seed = seed;
        let expected = crate::predict(&input)?;
        let actual = predictor.predict(input).await?;
        assert_eq!(
            actual
                .cells
                .iter()
                .map(|cell| &cell.biome)
                .collect::<Vec<_>>(),
            expected
                .cells
                .iter()
                .map(|cell| &cell.biome)
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}

#[tokio::test]
async fn admission_is_bounded_and_disconnection_is_explicit() {
    let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
    for _ in 0..QUEUE_CAPACITY {
        let (reply, _response) = oneshot::channel();
        assert!(
            sender
                .try_send(Job {
                    request: request(false, 0, 0, 4),
                    reply
                })
                .is_ok()
        );
    }
    let predictor = Predictor { sender };
    assert!(matches!(
        predictor.predict(request(false, 0, 0, 4)).await,
        Err(Error::Busy)
    ));
    drop(receiver);
    assert!(matches!(
        predictor.predict(request(false, 0, 0, 4)).await,
        Err(Error::Unavailable)
    ));
}

#[tokio::test]
async fn dropping_a_waiter_marks_queued_work_cancelled() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let predictor = Predictor { sender };
    let mut pending = Box::pin(predictor.predict(request(false, 0, 0, 4)));
    std::future::poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    drop(predictor);
    assert!(next(&receiver).is_none());
}

#[test]
fn active_sampling_observes_cancellation_before_density_work() {
    for (dimension, surface) in [
        (Dimension::Overworld, false),
        (Dimension::Overworld, true),
        (Dimension::Nether, false),
        (Dimension::End, false),
    ] {
        let router = generator::router(1, dimension, false, surface);
        let mut sampler = generator::Sampler::new(&router, dimension, 1);
        let mut input = request(false, 0, 0, 4);
        input.dimension = dimension;
        if surface {
            input.y = None;
        }
        assert!(matches!(
            generator::sample(&input, &mut sampler, || true),
            Err(Error::Cancelled)
        ));
        let row = std::cell::Cell::new(0);
        assert!(matches!(
            generator::sample(&input, &mut sampler, || {
                row.set(row.get() + 1);
                row.get() == 3
            }),
            Err(Error::Cancelled)
        ));
    }
}

#[tokio::test]
#[ignore = "development timing, run explicitly without a live server"]
async fn measure_surface_and_slice_tiles() -> Result<(), Error> {
    let predictor = Predictor::new()?;
    for large in [false, true] {
        for step in [4, 64, 1024] {
            for y in [Some(64), None] {
                let mut elapsed = Vec::with_capacity(9);
                for i in 0..10 {
                    let mut input = request(large, -16384 + i * 256, -16384, step);
                    input.y = y;
                    let start = Instant::now();
                    let result = predictor.predict(input).await?;
                    assert_eq!(result.cells.len(), 4096);
                    if i > 0 {
                        elapsed.push(start.elapsed());
                    }
                }
                elapsed.sort();
                println!(
                    "large={large} step={step} y={y:?} warm_median={:?} warm_max={:?}",
                    elapsed[4], elapsed[8]
                );
            }
        }
    }
    for y in [Some(64), None] {
        let mut elapsed = Vec::with_capacity(9);
        for i in 0..10 {
            let mut input = request(false, -2688, -6272, 4);
            input.seed = -1;
            input.y = y;
            let start = Instant::now();
            let result = predictor.predict(input).await?;
            assert_eq!(result.cells.len(), 4096);
            if i > 0 {
                elapsed.push(start.elapsed());
            }
        }
        elapsed.sort();
        println!(
            "sulfur_patch y={y:?} warm_median={:?} warm_max={:?}",
            elapsed[4], elapsed[8]
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "development timing, run explicitly without a live server"]
async fn measure_persistent_tiles() -> Result<(), Error> {
    let predictor = Predictor::new()?;
    for (dimension, large) in [
        (Dimension::Overworld, false),
        (Dimension::Overworld, true),
        (Dimension::Nether, false),
        (Dimension::End, false),
    ] {
        let mut elapsed = Vec::new();
        for i in 0..17 {
            let start = Instant::now();
            let mut input = request(large, i * 256, -1024, 4);
            input.dimension = dimension;
            let result = predictor.predict(input).await?;
            assert_eq!(result.cells.len(), 4096);
            elapsed.push(start.elapsed());
        }
        let cold = elapsed.remove(0);
        elapsed.sort();
        println!(
            "dimension={dimension:?} large_biomes={large} cold={cold:?} warm_median={:?} warm_max={:?} samples=4096",
            elapsed[elapsed.len() / 2],
            elapsed.last()
        );
    }
    Ok(())
}
