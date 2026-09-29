//! PNG pixels and public identity metadata must describe the same visible cells.

use super::super::seed_tile_dto::MinecraftSeedPreset;
use super::*;
use std::io::Cursor;

fn tile(palette: Vec<String>) -> MinecraftSeedTile {
    MinecraftSeedTile {
        world: "minecraft:overworld".into(),
        tile_x: -1,
        tile_z: 2,
        level: 0,
        min_x: -256,
        min_z: 512,
        y: Some(64),
        step: 4,
        width: 64,
        height: 64,
        preset: MinecraftSeedPreset::Default,
        generator_revision: "fixture".into(),
        profile_epoch: "0123456789abcdef".into(),
        sampled_at_ms: 1_000_000,
        expires_at_ms: 1_015_000,
        indices: (0..4096)
            .map(|index| {
                if index % 5 == 0 || palette.is_empty() {
                    None
                } else {
                    Some((index % palette.len()) as u16)
                }
            })
            .collect(),
        palette,
    }
}

fn metadata(bytes: &[u8]) -> Result<&[u8], Box<dyn std::error::Error>> {
    let mut offset = 8;
    let mut found = None;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into()?) as usize;
        let chunk = &bytes[offset + 4..offset + 8];
        let body = bytes
            .get(offset + 8..offset + 8 + length)
            .ok_or("short PNG chunk")?;
        if chunk == METADATA_CHUNK {
            if found.is_some() {
                return Err("duplicate metadata".into());
            }
            found = Some(body);
        }
        offset += length + 12;
    }
    found.ok_or_else(|| "missing PNG metadata".into())
}

#[test]
fn transparent_pixels_and_hover_indices_match_for_every_palette_depth()
-> Result<(), Box<dyn std::error::Error>> {
    for count in [0, 1, 2, 3, 4, 15, 16, 255, 256] {
        let palette = (0..count)
            .map(|index| format!("minecraft:fixture_{index}"))
            .collect();
        let tile = tile(palette);
        let bytes = encode(&tile)?;
        assert!(bytes.len() <= MAX_BYTES);
        assert_eq!(metadata(&bytes)?, seed_tile_binary::encode(&tile)?);
        let mut decoder = png::Decoder::new(Cursor::new(&bytes));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info()?;
        let mut pixels = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or("PNG dimensions missing")?
        ];
        let info = reader.next_frame(&mut pixels)?;
        assert_eq!(
            (info.width, info.height, info.color_type),
            (64, 64, png::ColorType::Rgba)
        );
        for (index, symbol) in tile.indices.iter().enumerate() {
            let pixel = &pixels[index * 4..index * 4 + 4];
            match symbol {
                None => assert_eq!(pixel, &[0, 0, 0, 0]),
                Some(symbol) => {
                    assert_eq!(
                        &pixel[..3],
                        &seed_tile_colors::color(&tile.palette[usize::from(*symbol)])
                    );
                    assert_eq!(pixel[3], 255);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn identical_display_colors_retain_distinct_hover_names() -> Result<(), Box<dyn std::error::Error>>
{
    let mut tile = tile(vec!["minecraft:plains".into(), "minecraft:meadow".into()]);
    tile.y = None;
    assert_eq!(
        seed_tile_colors::color(&tile.palette[0]),
        seed_tile_colors::color(&tile.palette[1])
    );
    assert_eq!(metadata(&encode(&tile)?)?, seed_tile_binary::encode(&tile)?);
    assert_eq!(metadata(&encode(&tile)?)?[4], 2);
    Ok(())
}

#[test]
fn frozen_river_stays_distinct_in_png_pixels_and_hover_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        ("frozen_river", [0x8d, 0xd8, 0xe8, 255]),
        ("river", [0x53, 0x7f, 0xba, 255]),
        ("snowy_plains", [0xd6, 0xe4, 0xe1, 255]),
        ("ice_spikes", [0xd6, 0xe4, 0xe1, 255]),
        ("grove", [0xd6, 0xe4, 0xe1, 255]),
        ("frozen_ocean", [0x53, 0x7f, 0xba, 255]),
    ];
    let mut tile = tile(
        fixtures
            .iter()
            .map(|(name, _)| format!("minecraft:{name}"))
            .collect(),
    );
    tile.indices.fill(None);
    for (index, value) in tile.indices.iter_mut().take(fixtures.len()).enumerate() {
        *value = Some(index as u16);
    }
    let bytes = encode(&tile)?;
    assert_eq!(metadata(&bytes)?, seed_tile_binary::encode(&tile)?);
    let mut decoder = png::Decoder::new(Cursor::new(&bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG dimensions missing")?
    ];
    let info = reader.next_frame(&mut pixels)?;
    assert_eq!(info.color_type, png::ColorType::Rgba);
    for (index, (name, color)) in fixtures.iter().enumerate() {
        assert_eq!(&pixels[index * 4..index * 4 + 4], color, "{name}");
    }
    assert_eq!(
        &pixels[fixtures.len() * 4..fixtures.len() * 4 + 4],
        &[0, 0, 0, 0]
    );
    Ok(())
}

#[test]
fn dimension_colors_match_the_browser_palette() {
    for (name, color) in [
        ("nether_wastes", [0x91, 0x46, 0x46]),
        ("warped_forest", [0x33, 0x8b, 0x80]),
        ("the_end", [0xc8, 0xc2, 0x8c]),
        ("end_barrens", [0x71, 0x68, 0x85]),
        ("frozen_ocean", [0x53, 0x7f, 0xba]),
        ("snowy_plains", [0xd6, 0xe4, 0xe1]),
    ] {
        assert_eq!(seed_tile_colors::color(&format!("minecraft:{name}")), color);
    }
}
