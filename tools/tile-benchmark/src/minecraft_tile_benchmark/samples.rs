//! Bounded synthetic terrain corpus, including permission-mask edge cases.

use minecraft_seed::{Dimension, PredictionRequest, Predictor};
use minecraft_tile_codec::types::{MinecraftSeedPreset, MinecraftSeedTile};
use std::collections::BTreeMap;

pub struct Sample {
    pub name: String,
    pub tile: MinecraftSeedTile,
}

/// Generate each base grid once, then apply independently reproducible masks.
pub async fn generate() -> Result<Vec<Sample>, Box<dyn std::error::Error>> {
    let predictor = Predictor::new()?;
    let mut result = Vec::with_capacity(72);
    for (dimension, large, label, world) in [
        (
            Dimension::Overworld,
            false,
            "default",
            "minecraft:overworld",
        ),
        (
            Dimension::Overworld,
            true,
            "large_biomes",
            "minecraft:overworld",
        ),
        (Dimension::Nether, false, "nether", "minecraft:the_nether"),
        (Dimension::End, false, "end", "minecraft:the_end"),
    ] {
        for level in [0, 3, 8] {
            for (tile_x, tile_z) in [(0, 0), (-3, -7)] {
                let step = 4_u32 << level;
                let min_x = tile_x * (step as i32 * 64);
                let min_z = tile_z * (step as i32 * 64);
                let generated = predictor
                    .predict(PredictionRequest {
                        seed: 1,
                        dimension,
                        large_biomes: large,
                        y: Some(64),
                        min_x,
                        min_z,
                        width: 64,
                        height: 64,
                        step,
                    })
                    .await?;
                for mask in ["full", "masked", "empty"] {
                    let mut lookup = BTreeMap::new();
                    let mut palette = Vec::new();
                    let mut indices = Vec::with_capacity(4096);
                    for (index, cell) in generated.cells.iter().enumerate() {
                        let hidden = mask == "empty"
                            || (mask == "masked" && ((index % 64 < 23) || (index / 64 > 47)));
                        if hidden {
                            indices.push(None);
                            continue;
                        }
                        let next = palette.len() as u16;
                        let value = *lookup.entry(&cell.biome).or_insert_with(|| {
                            palette.push(cell.biome.clone());
                            next
                        });
                        indices.push(Some(value));
                    }
                    result.push(Sample {
                        name: format!("{label}/level{level}/{tile_x},{tile_z}/{mask}"),
                        tile: MinecraftSeedTile {
                            world: world.into(),
                            tile_x,
                            tile_z,
                            level,
                            min_x,
                            min_z,
                            y: Some(64),
                            step,
                            width: 64,
                            height: 64,
                            preset: match dimension {
                                Dimension::Overworld if large => MinecraftSeedPreset::LargeBiomes,
                                Dimension::Overworld => MinecraftSeedPreset::Default,
                                Dimension::Nether => MinecraftSeedPreset::Nether,
                                Dimension::End => MinecraftSeedPreset::End,
                            },
                            generator_revision: minecraft_seed::GENERATOR_REVISION.into(),
                            profile_epoch: "e486a66a-78d6-41b3-930e-66a8ee34ed29".into(),
                            sampled_at_ms: 1_800_000_000_000,
                            expires_at_ms: 1_800_000_015_000,
                            palette,
                            indices,
                        },
                    });
                }
            }
        }
    }
    Ok(result)
}
