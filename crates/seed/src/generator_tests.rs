//! Predictions checked against independent saved Paper data and batch invariants.

use std::{collections::BTreeMap, error::Error, fs::File, io::Read};

use pumpkin_data::chunk::{BiomeTree, OVERWORLD_BIOME_SOURCE};

use crate::{Dimension, PredictionRequest, predict};

pub(super) fn biome_distance(tree: &BiomeTree, point: &[i64; 7], wanted: &str, best: &mut i64) {
    match tree {
        BiomeTree::Leaf { parameters, biome } if biome.registry_id == wanted => {
            let squared = parameters
                .iter()
                .zip(point)
                .map(|(range, value)| {
                    let d = range.calc_distance(*value);
                    d * d
                })
                .sum::<i64>();
            *best = (*best).min(squared);
        }
        BiomeTree::Branch { nodes, .. } => {
            for child in *nodes {
                biome_distance(child, point, wanted, best);
            }
        }
        BiomeTree::Leaf { .. } => {}
    }
}

#[test]
fn aligned_grid_matches_scalar_noise() {
    use super::{DensityVolume, MultiNoiseSampler, router};

    for large in [false, true] {
        let router = router(1, Dimension::Overworld, large, false);
        let mut scalar = MultiNoiseSampler::generate(&router);
        let mut volume = MultiNoiseSampler::generate(&router);
        volume.fill_volume(DensityVolume::new(32, 1, 32, 128, 64, 96, 4, 4, 4));
        for x in 32..64 {
            for z in 24..56 {
                assert_eq!(scalar.sample(x, 16, z), volume.sample(x, 16, z));
            }
        }
    }
}

#[test]
fn saved_shore_boundaries_have_equal_biome_distances() {
    use super::{MultiNoiseSampler, router};
    let router = router(1, Dimension::Overworld, false, false);
    let mut sampler = MultiNoiseSampler::generate(&router);
    for (x, y, z) in [(42, -16, 29), (44, 16, 37), (48, -3, 29)] {
        let point = sampler.sample(x, y, z).convert_to_list();
        let mut beach = i64::MAX;
        let mut shore = i64::MAX;
        biome_distance(&OVERWORLD_BIOME_SOURCE, &point, "beach", &mut beach);
        biome_distance(&OVERWORLD_BIOME_SOURCE, &point, "stony_shore", &mut shore);
        assert_ne!(beach, i64::MAX);
        assert_eq!(beach, shore);
    }
}

/// This saved vanilla boundary was misclassified as beach by Cubiomes 18edd56.
#[test]
fn matches_saved_paper_shore_boundary() -> Result<(), Box<dyn Error>> {
    let request = PredictionRequest {
        seed: 1,
        dimension: Dimension::Overworld,
        large_biomes: false,
        y: Some(64),
        min_x: 176,
        min_z: 148,
        width: 1,
        height: 1,
        step: 4,
    };
    let result = predict(&request)?;
    assert_eq!(result.cells[0].biome, "minecraft:stony_shore");
    Ok(())
}

#[test]
fn overlapping_grids_and_scalar_queries_are_stable() -> Result<(), Box<dyn Error>> {
    for large_biomes in [false, true] {
        let mut request = PredictionRequest {
            seed: 1,
            dimension: Dimension::Overworld,
            large_biomes,
            y: Some(64),
            min_x: 128,
            min_z: 96,
            width: 32,
            height: 32,
            step: 4,
        };
        let original = predict(&request)?
            .cells
            .into_iter()
            .map(|cell| ((cell.x, cell.z), cell.biome))
            .collect::<BTreeMap<_, _>>();
        request.min_x += 4;
        request.min_z += 4;
        request.width = 31;
        request.height = 31;
        for cell in predict(&request)?.cells {
            assert_eq!(original.get(&(cell.x, cell.z)), Some(&cell.biome));
        }
        request.width = 1;
        request.height = 1;
        request.step = 1;
        for (x, z) in [(168, 116), (176, 148), (192, 116), (248, 216)] {
            request.min_x = x;
            request.min_z = z;
            assert_eq!(
                original.get(&(x, z)),
                Some(&predict(&request)?.cells[0].biome)
            );
        }
    }
    Ok(())
}

#[test]
fn large_biomes_matches_saved_negative_coordinate_boundaries() -> Result<(), Box<dyn Error>> {
    for (x, z, expected) in [(940, -1296, "river"), (868, -1228, "beach")] {
        let mut request = PredictionRequest {
            seed: 1,
            dimension: Dimension::Overworld,
            large_biomes: true,
            y: Some(-64),
            min_x: x,
            min_z: z,
            width: 1,
            height: 1,
            step: 4,
        };
        let large = predict(&request)?;
        assert_eq!(large.cells[0].biome, format!("minecraft:{expected}"));
        request.large_biomes = false;
        assert_ne!(large.cells[0].biome, predict(&request)?.cells[0].biome);
    }
    Ok(())
}

/// Compare an external raw-quart TSV fixture without checking private data into Git.
/// Set MINECRAFT_SEED_FIXTURE and optionally MINECRAFT_SEED_LARGE_BIOMES=1.
#[test]
#[ignore = "requires an independently extracted synthetic vanilla biome fixture"]
fn compares_external_vanilla_fixture() -> Result<(), Box<dyn Error>> {
    let path = std::env::var("MINECRAFT_SEED_FIXTURE")?;
    let large = std::env::var("MINECRAFT_SEED_LARGE_BIOMES").is_ok_and(|value| value == "1");
    let mut input = String::new();
    File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 16 * 1024 * 1024 {
        return Err("fixture exceeds 16 MiB".into());
    }
    let mut samples = 0;
    let mut mismatches = 0;
    let mut tied_mismatches = 0;
    let router = super::router(1, Dimension::Overworld, large, false);
    let mut sampler = super::MultiNoiseSampler::generate(&router);
    let mut examples = Vec::new();
    let mut slices = BTreeMap::<(i32, i32, i32), BTreeMap<(i32, i32), &str>>::new();
    for line in input.lines().skip(1) {
        samples += 1;
        if samples > 100_000 {
            return Err("fixture exceeds 100000 samples".into());
        }
        let mut columns = line.split_whitespace();
        let x: i32 = columns.next().ok_or("missing X")?.parse()?;
        let y: i32 = columns.next().ok_or("missing Y")?.parse()?;
        let z: i32 = columns.next().ok_or("missing Z")?.parse()?;
        let expected = columns.next().ok_or("missing biome")?;
        if !(-16..=79).contains(&y)
            || !(-7_500_000..=7_500_000).contains(&x)
            || !(-7_500_000..=7_500_000).contains(&z)
        {
            return Err("fixture coordinate out of bounds".into());
        }
        let tile = slices
            .entry((y, x.div_euclid(32), z.div_euclid(32)))
            .or_default();
        if tile.insert((x, z), expected).is_some() {
            return Err("duplicate fixture coordinate".into());
        }
    }
    for ((y, _, _), expected) in slices {
        let min_x = expected
            .keys()
            .map(|(x, _)| *x)
            .min()
            .ok_or("empty slice")?;
        let max_x = expected
            .keys()
            .map(|(x, _)| *x)
            .max()
            .ok_or("empty slice")?;
        let min_z = expected
            .keys()
            .map(|(_, z)| *z)
            .min()
            .ok_or("empty slice")?;
        let max_z = expected
            .keys()
            .map(|(_, z)| *z)
            .max()
            .ok_or("empty slice")?;
        let actual = predict(&PredictionRequest {
            seed: 1,
            dimension: Dimension::Overworld,
            large_biomes: large,
            y: Some(y * 4),
            min_x: min_x * 4,
            min_z: min_z * 4,
            width: (max_x - min_x + 1) as u32,
            height: (max_z - min_z + 1) as u32,
            step: 4,
        })?;
        for cell in actual.cells {
            if let Some(wanted) = expected.get(&(cell.x / 4, cell.z / 4))
                && *wanted != cell.biome
            {
                mismatches += 1;
                let point = sampler.sample(cell.x / 4, y, cell.z / 4).convert_to_list();
                let mut wanted_distance = i64::MAX;
                let mut actual_distance = i64::MAX;
                biome_distance(
                    &OVERWORLD_BIOME_SOURCE,
                    &point,
                    wanted.strip_prefix("minecraft:").ok_or("invalid biome")?,
                    &mut wanted_distance,
                );
                biome_distance(
                    &OVERWORLD_BIOME_SOURCE,
                    &point,
                    cell.biome
                        .strip_prefix("minecraft:")
                        .ok_or("invalid biome")?,
                    &mut actual_distance,
                );
                if wanted_distance == actual_distance && wanted_distance != i64::MAX {
                    tied_mismatches += 1;
                }
                if examples.len() < 8 {
                    examples.push((cell.x, y * 4, cell.z, *wanted, cell.biome));
                }
            }
        }
    }
    assert!(samples > 0);
    println!(
        "{samples} samples, {mismatches} mismatches, {tied_mismatches} equal-distance ties; examples: {examples:?}"
    );
    assert_eq!(
        mismatches, tied_mismatches,
        "{mismatches} mismatches across {samples} samples: {examples:?}"
    );
    if std::env::var("MINECRAFT_SEED_REQUIRE_EXACT").is_ok_and(|value| value == "1") {
        assert_eq!(mismatches, 0, "exact saved-palette comparison failed");
    }
    Ok(())
}
