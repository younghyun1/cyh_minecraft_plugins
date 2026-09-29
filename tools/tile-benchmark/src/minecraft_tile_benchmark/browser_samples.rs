//! Matching native PNG, binary and JSON grids for browser pan, revisit and zoom tests.

use super::{formats::Result, samples::Sample};
use minecraft_seed::{Dimension, PredictionRequest, Predictor};
use minecraft_tile_codec::{
    seed_tile_binary, seed_tile_png,
    types::{MinecraftSeedPreset, MinecraftSeedTile},
};
use std::{collections::BTreeMap, path::Path};

/// Export a fixed location grid rather than selecting coordinates for either codec.
pub async fn export(directory: &Path, samples: &[Sample]) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let mut manifest = Vec::with_capacity(samples.len() + 360);
    for sample in samples {
        write(directory, &mut manifest, sample)?;
    }
    let predictor = Predictor::new()?;
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
        for level in [0, 1] {
            for tile_z in -2..=2 {
                for tile_x in -3..=5 {
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
                    let mut lookup = BTreeMap::new();
                    let mut palette = Vec::new();
                    let mut indices = Vec::with_capacity(4096);
                    for cell in generated.cells {
                        let next = palette.len() as u16;
                        indices.push(Some(*lookup.entry(cell.biome.clone()).or_insert_with(
                            || {
                                palette.push(cell.biome);
                                next
                            },
                        )));
                    }
                    let sample = Sample {
                        name: format!("grid/{label}/level{level}/{tile_x},{tile_z}/full"),
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
                    };
                    write(directory, &mut manifest, &sample)?;
                }
            }
        }
    }
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    eprintln!(
        "Exported {} matched browser tiles to {}",
        manifest.len(),
        directory.display()
    );
    Ok(())
}

fn write(directory: &Path, manifest: &mut Vec<serde_json::Value>, sample: &Sample) -> Result<()> {
    let base = format!("tile-{:03}", manifest.len());
    std::fs::write(
        directory.join(format!("{base}.json")),
        serde_json::to_vec(&sample.tile)?,
    )?;
    std::fs::write(
        directory.join(format!("{base}.bin")),
        seed_tile_binary::encode(&sample.tile)?,
    )?;
    std::fs::write(
        directory.join(format!("{base}.png")),
        seed_tile_png::encode(&sample.tile)?,
    )?;
    manifest.push(serde_json::json!({"name":sample.name,"query":{"world":sample.tile.world,"y":sample.tile.y,"level":sample.tile.level,"tile_x":sample.tile.tile_x,"tile_z":sample.tile.tile_z},
        "json":format!("{base}.json"),"binary":format!("{base}.bin"),"png":format!("{base}.png")}));
    Ok(())
}
