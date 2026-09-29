//! Independent Paper climate projection and surface/slice isolation regressions.

use std::{collections::BTreeSet, error::Error};

use pumpkin_data::chunk::OVERWORLD_BIOME_SOURCE;

use super::{DensityVolume, MultiNoiseSampler, Sampler, router, sample, tests::biome_distance};
use crate::{Dimension, PredictionRequest};

const CAVES: [&str; 4] = ["lush_caves", "dripstone_caves", "deep_dark", "sulfur_caves"];

/// Vanilla's previous-leaf tie handling can differ without changing nearest distance.
fn assert_nearest(point: &[i64; 7], expected: &str, surface: bool) -> Result<String, crate::Error> {
    let actual = if surface {
        crate::surface_biome::get(point)?
    } else {
        OVERWORLD_BIOME_SOURCE.get(point, &mut None)
    }
    .registry_id;
    let expected = expected.trim_start_matches("minecraft:");
    if actual != expected {
        let mut wanted_distance = i64::MAX;
        let mut actual_distance = i64::MAX;
        biome_distance(
            &OVERWORLD_BIOME_SOURCE,
            point,
            expected,
            &mut wanted_distance,
        );
        biome_distance(&OVERWORLD_BIOME_SOURCE, point, actual, &mut actual_distance);
        assert_ne!(wanted_distance, i64::MAX);
        assert_eq!(wanted_distance, actual_distance, "{actual} != {expected}");
    }
    Ok(actual.to_owned())
}

#[test]
fn projected_climate_matches_independent_paper_and_preserves_slices() -> Result<(), Box<dyn Error>>
{
    let rows = include_str!("../tests/fixtures/paper26_3_surface.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut count = 0;
    let mut unfiltered_caves = 0;
    let mut surface_biomes = BTreeSet::new();
    for seed in [1, -1, i64::MIN, i64::MAX] {
        for large in [false, true] {
            let surface_router = router(seed, Dimension::Overworld, large, true);
            let slice_router = router(seed, Dimension::Overworld, large, false);
            let mut surface = MultiNoiseSampler::generate(&surface_router);
            let mut slice = MultiNoiseSampler::generate(&slice_router);
            let mut cave_transitions = BTreeSet::new();
            for row in &rows {
                assert_eq!(row.len(), 14);
                if row[0].parse::<i64>()? != seed || row[1].parse::<bool>()? != large {
                    continue;
                }
                count += 1;
                let x = row[2].parse::<i32>()? >> 2;
                let y = row[3].parse::<i32>()? >> 2;
                let z = row[4].parse::<i32>()? >> 2;
                let mut projected = surface.sample(x, 0, z).convert_to_list();
                let fixed = slice.sample(x, y, z).convert_to_list();
                assert_eq!(projected[4], 0);
                for (index, column) in [(0, 5), (1, 6), (2, 7), (3, 8), (5, 9)] {
                    let expected: i64 = row[column].parse()?;
                    assert!(
                        (projected[index] - expected).abs() <= 1,
                        "seed={seed} large={large} x={x} z={z} channel={index}"
                    );
                    assert_eq!(projected[index], fixed[index]);
                }
                // The existing f32 spline differs from Paper by up to two depth
                // quantization units; non-depth channels differ by at most one.
                assert!(
                    (fixed[4] - row[10].parse::<i64>()?).abs() <= 2,
                    "seed={seed} large={large} x={x} y={y} z={z} depth={} expected={}",
                    fixed[4],
                    row[10]
                );
                let surface_biome = assert_nearest(&projected, row[12], true)?;
                let fixed_biome = assert_nearest(&fixed, row[11], false)?;
                let unfiltered = assert_nearest(&projected, row[13], false)?;
                if CAVES.contains(&unfiltered.as_str()) {
                    unfiltered_caves += 1;
                    assert_eq!(unfiltered, "sulfur_caves");
                    assert_eq!(surface_biome, "forest");
                }
                assert!(!CAVES.contains(&surface_biome.as_str()));
                surface_biomes.insert(surface_biome);
                if CAVES.contains(&fixed_biome.as_str()) {
                    cave_transitions.insert(fixed_biome);
                }
                // Selecting depth zero only must equal the optimized router.
                projected[4] = fixed[4];
                assert_eq!(projected, fixed);
            }
            assert!(cave_transitions.contains("lush_caves"));
            assert!(cave_transitions.contains("dripstone_caves"));
            assert!(cave_transitions.contains("deep_dark"));
            assert!(cave_transitions.contains("sulfur_caves"));
        }
    }
    assert_eq!(count, 1520);
    assert_eq!(unfiltered_caves, 2);
    for biome in [
        "frozen_river",
        "river",
        "warm_ocean",
        "frozen_ocean",
        "forest",
        "snowy_plains",
        "jagged_peaks",
        "desert",
        "mushroom_fields",
        "swamp",
    ] {
        assert!(surface_biomes.contains(biome), "missing surface {biome}");
    }
    Ok(())
}

#[tokio::test]
async fn surface_fallback_and_fixed_cave_slice_survive_worker_mode_switches()
-> Result<(), crate::Error> {
    let predictor = crate::Predictor::new()?;
    for (y, expected) in [
        (None, "forest"),
        (Some(-32), "sulfur_caves"),
        (None, "forest"),
        (Some(64), "forest"),
        (Some(-32), "sulfur_caves"),
    ] {
        let response = predictor
            .predict(PredictionRequest {
                seed: -1,
                dimension: Dimension::Overworld,
                large_biomes: false,
                y,
                min_x: -2563,
                min_z: -6145,
                width: 1,
                height: 1,
                step: 4,
            })
            .await?;
        assert_eq!(response.cells[0].biome, format!("minecraft:{expected}"));
    }
    Ok(())
}

#[test]
fn surface_volume_matches_scalar_at_negative_and_unaligned_coordinates()
-> Result<(), Box<dyn Error>> {
    for large in [false, true] {
        let router = router(-1, Dimension::Overworld, large, true);
        let mut scalar = MultiNoiseSampler::generate(&router);
        let mut volume = MultiNoiseSampler::generate(&router);
        volume.fill_volume(DensityVolume::new(32, 1, 32, -256, 0, -512, 4, 4, 4));
        for x in -64..-32 {
            for z in -128..-96 {
                assert_eq!(scalar.sample(x, 0, z), volume.sample(x, 0, z));
            }
        }
        let mut sampler = Sampler::new(&router, Dimension::Overworld, -1);
        for step in [3, 4, 64, 1024] {
            let request = PredictionRequest {
                seed: -1,
                dimension: Dimension::Overworld,
                large_biomes: large,
                y: None,
                min_x: -253,
                min_z: -509,
                width: 8,
                height: 8,
                step,
            };
            for cell in sample(&request, &mut sampler, || false)?.cells {
                let point = scalar.sample(cell.x >> 2, 0, cell.z >> 2).convert_to_list();
                let expected = crate::surface_biome::get(&point)?.registry_id;
                assert_eq!(cell.biome, format!("minecraft:{expected}"));
            }
        }
    }
    Ok(())
}
