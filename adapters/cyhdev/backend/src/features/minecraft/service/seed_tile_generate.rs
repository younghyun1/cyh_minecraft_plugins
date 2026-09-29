//! Generate a complete bounded palette tile independently of permission masking.

use super::{
    seed_profile::{Preset, Profile},
    seed_tile_cache::Tile,
};
use crate::features::minecraft::{
    domain::seed_tile::{CELLS, SIDE, SeedTileQuery, WORLD_EDGE},
    error::MapError,
};

pub(super) async fn generate(
    predictor: &minecraft_seed::Predictor,
    profile: &Profile,
    query: &SeedTileQuery,
    min_x: i32,
    min_z: i32,
    step: u32,
) -> Result<Tile, MapError> {
    let crop = |origin: i32| {
        let first = ((-WORLD_EDGE - i64::from(origin) + i64::from(step) - 1) / i64::from(step))
            .clamp(0, SIDE as i64);
        let end = ((WORLD_EDGE - i64::from(origin)) / i64::from(step) + 1).clamp(0, SIDE as i64);
        (first as u32, end as u32)
    };
    let (first_x, end_x) = crop(min_x);
    let (first_z, end_z) = crop(min_z);
    let mut tile = Tile {
        palette: Vec::new(),
        indices: vec![None; CELLS],
    };
    if first_x >= end_x || first_z >= end_z {
        return Ok(tile);
    }
    let any_permitted = (first_z..end_z).any(|row| {
        (first_x..end_x).any(|column| {
            profile.permits(
                i64::from(min_x) + i64::from(column) * i64::from(step),
                i64::from(min_z) + i64::from(row) * i64::from(step),
                step,
            )
        })
    });
    if !any_permitted {
        return Ok(tile);
    }
    let request = minecraft_seed::PredictionRequest {
        seed: profile.seed,
        dimension: match profile.preset {
            Preset::Default | Preset::LargeBiomes => minecraft_seed::Dimension::Overworld,
            Preset::Nether => minecraft_seed::Dimension::Nether,
            Preset::End => minecraft_seed::Dimension::End,
        },
        large_biomes: profile.preset == Preset::LargeBiomes,
        y: query.y,
        min_x: min_x + (first_x * step) as i32,
        min_z: min_z + (first_z * step) as i32,
        width: end_x - first_x,
        height: end_z - first_z,
        step,
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        predictor.predict(request),
    )
    .await
    .map_err(|_| MapError::Unavailable)?
    .map_err(|error| match error {
        minecraft_seed::Error::Busy => MapError::Busy,
        _ => MapError::Unavailable,
    })?;
    if output.cells.len() != ((end_x - first_x) * (end_z - first_z)) as usize
        || output.large_biomes != (profile.preset == Preset::LargeBiomes)
        || output.generator_revision != minecraft_seed::GENERATOR_REVISION
    {
        return Err(MapError::Unavailable);
    }
    for cell in output.cells {
        let x = i64::from(cell.x) - i64::from(min_x);
        let z = i64::from(cell.z) - i64::from(min_z);
        if x < 0
            || z < 0
            || x % i64::from(step) != 0
            || z % i64::from(step) != 0
            || !(i64::from(first_x)..i64::from(end_x)).contains(&(x / i64::from(step)))
            || !(i64::from(first_z)..i64::from(end_z)).contains(&(z / i64::from(step)))
            || !crate::features::minecraft::domain::map_query::identifier(&cell.biome)
        {
            return Err(MapError::Unavailable);
        }
        let index = (z / i64::from(step)) as usize * SIDE + (x / i64::from(step)) as usize;
        if tile.indices[index].is_some() {
            return Err(MapError::Unavailable);
        }
        let palette = match tile.palette.iter().position(|biome| biome == &cell.biome) {
            Some(index) => index,
            None if tile.palette.len() < 256 => {
                tile.palette.push(cell.biome);
                tile.palette.len() - 1
            }
            None => return Err(MapError::Unavailable),
        };
        tile.indices[index] = Some(palette as u16);
    }
    Ok(tile)
}
