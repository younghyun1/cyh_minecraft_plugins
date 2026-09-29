//! Surface-only nearest-climate lookup using Pumpkin's existing bounded spatial tree.

use pumpkin_data::chunk::{Biome, BiomeTree, OVERWORLD_BIOME_SOURCE};

use crate::Error;

/// Java 26.3's cave-only entries can win even at depth zero on other climate axes.
pub(crate) fn is_cave(biome: &Biome) -> bool {
    matches!(
        biome.registry_id,
        "lush_caves" | "dripstone_caves" | "sulfur_caves" | "deep_dark"
    )
}

/// Keep the ordinary indexed lookup unless its winner is underground-only.
pub(crate) fn get(point: &[i64; 7]) -> Result<&'static Biome, Error> {
    let nearest = OVERWORLD_BIOME_SOURCE.get(point, &mut None);
    if !is_cave(nearest) {
        return Ok(nearest);
    }
    let mut best_distance = i64::MAX;
    let mut best = None;
    search(
        &OVERWORLD_BIOME_SOURCE,
        point,
        &mut best_distance,
        &mut best,
    );
    best.ok_or(Error::BiomeLookup)
}

/// Preserve Pumpkin's distance pruning and first-leaf tie order without allocating
/// a duplicate biome index. Excluding leaves cannot invalidate branch lower bounds.
fn search(
    tree: &'static BiomeTree,
    point: &[i64; 7],
    best_distance: &mut i64,
    best: &mut Option<&'static Biome>,
) {
    let parameters = match tree {
        BiomeTree::Leaf { parameters, .. } | BiomeTree::Branch { parameters, .. } => parameters,
    };
    let distance = parameters
        .iter()
        .zip(point)
        .map(|(range, value)| {
            let d = range.calc_distance(*value);
            d * d
        })
        .sum();
    if distance >= *best_distance {
        return;
    }
    match tree {
        BiomeTree::Leaf { biome, .. } if !is_cave(biome) => {
            *best = Some(biome);
            *best_distance = distance;
        }
        BiomeTree::Leaf { .. } => {}
        BiomeTree::Branch { nodes, .. } => {
            for node in *nodes {
                search(node, point, best_distance, best);
            }
        }
    }
}
