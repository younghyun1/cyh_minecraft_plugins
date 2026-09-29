//! Dimension coverage, batch independence and offline Paper noise fixtures.

use std::{collections::BTreeSet, error::Error};

use crate::{Dimension, PredictionRequest, predict};

fn request(dimension: Dimension, x: i32, z: i32) -> PredictionRequest {
    PredictionRequest {
        seed: 1,
        dimension,
        large_biomes: false,
        y: Some(64),
        min_x: x,
        min_z: z,
        width: 64,
        height: 64,
        step: 128,
    }
}

#[test]
fn nether_and_end_tiles_cover_all_vanilla_biomes() -> Result<(), Box<dyn Error>> {
    for (dimension, expected) in [
        (
            Dimension::Nether,
            [
                "nether_wastes",
                "soul_sand_valley",
                "crimson_forest",
                "warped_forest",
                "basalt_deltas",
            ],
        ),
        (
            Dimension::End,
            [
                "the_end",
                "end_highlands",
                "end_midlands",
                "small_end_islands",
                "end_barrens",
            ],
        ),
    ] {
        let actual = predict(&request(dimension, -4096, -4096))?
            .cells
            .into_iter()
            .map(|cell| cell.biome)
            .collect::<BTreeSet<_>>();
        let expected = expected
            .into_iter()
            .map(|name| format!("minecraft:{name}"))
            .collect();
        assert_eq!(actual, expected, "{dimension:?}");
    }
    Ok(())
}

#[test]
fn dimension_samples_remain_stable_at_negative_coordinates_and_world_edge()
-> Result<(), Box<dyn Error>> {
    for dimension in [Dimension::Nether, Dimension::End] {
        for (x, z) in [
            (-1057, -1033),
            (-30_000_000, -30_000_000),
            (29_999_976, 29_999_976),
        ] {
            let mut input = request(dimension, x, z);
            input.width = 8;
            input.height = 8;
            input.step = 3;
            let batch = predict(&input)?;
            input.width = 1;
            input.height = 1;
            for cell in batch.cells.into_iter().rev() {
                input.min_x = cell.x;
                input.min_z = cell.z;
                input.y = Some(255);
                assert_eq!(cell.biome, predict(&input)?.cells[0].biome);
            }
        }
    }
    Ok(())
}

#[test]
fn end_center_uses_chunk_coordinates_and_wide_squared_distance() -> Result<(), Box<dyn Error>> {
    let mut input = request(Dimension::End, 0, 0);
    input.width = 1;
    input.height = 1;
    for (x, z) in [(1024, 0), (1039, 0), (-1024, 0), (-1, -1)] {
        input.min_x = x;
        input.min_z = z;
        assert_eq!(predict(&input)?.cells[0].biome, "minecraft:the_end");
    }
    for (x, z) in [
        (-1040, 0),
        (1040, 0),
        (-1048576, 0),
        (1048576, 0),
        (30_000_000, 30_000_000),
    ] {
        input.min_x = x;
        input.min_z = z;
        assert_ne!(predict(&input)?.cells[0].biome, "minecraft:the_end");
    }
    Ok(())
}

#[test]
fn compares_paper_dimension_noise_fixture() -> Result<(), Box<dyn Error>> {
    use pumpkin_world::generation::noise::router::multi_noise_sampler::MultiNoiseSampler;
    let text = include_str!("../tests/fixtures/paper26_3_dimensions.tsv");
    let mut count = 0;
    let mut seed = None;
    let mut fixtures = Vec::new();
    for line in text.lines().skip(1) {
        let values = line.split_whitespace().collect::<Vec<_>>();
        if values.len() != 6 {
            return Err("invalid fixture row".into());
        }
        fixtures.push((
            values[0].parse::<i64>()?,
            values[1].parse::<i32>()?,
            values[2].parse::<i32>()?,
            values[3].parse::<f32>()?,
            values[4].parse::<f32>()?,
            values[5].parse::<f32>()?,
        ));
    }
    for &(next_seed, ..) in &fixtures {
        if seed == Some(next_seed) {
            continue;
        }
        seed = Some(next_seed);
        let nether = super::generator::router(next_seed, Dimension::Nether, false, false);
        let mut nether = MultiNoiseSampler::generate(&nether);
        let end = super::end_sampler::EndSampler::new(next_seed);
        for &(sample_seed, x, z, temperature, vegetation, erosion) in &fixtures {
            if sample_seed != next_seed {
                continue;
            }
            let actual = nether.sample(x >> 2, 16, z >> 2);
            let mut expected = actual;
            expected.temperature = i64::from((temperature * 10_000.0) as i32);
            expected.humidity = i64::from((vegetation * 10_000.0) as i32);
            // Pumpkin retains double intermediates while Paper's updated noise
            // path rounds floats. Require at most one quantization unit and the
            // same biome, rather than silently accepting different regions.
            assert!(
                (actual.temperature - expected.temperature).abs() <= 1,
                "temperature at {x},{z}"
            );
            assert!(
                (actual.humidity - expected.humidity).abs() <= 1,
                "vegetation at {x},{z}"
            );
            let source = &pumpkin_data::chunk::NETHER_BIOME_SOURCE;
            assert_eq!(
                source.get(&actual.convert_to_list(), &mut None).registry_id,
                source
                    .get(&expected.convert_to_list(), &mut None)
                    .registry_id,
                "Nether biome at {x},{z}"
            );
            let actual = end.erosion(((x >> 4) * 2 + 1) * 8, ((z >> 4) * 2 + 1) * 8);
            assert_eq!(actual, erosion, "End erosion at {x},{z}");
            count += 1;
        }
    }
    assert_eq!(count, 1024);
    Ok(())
}
