use super::*;

fn tile() -> MinecraftSeedTile {
    MinecraftSeedTile {
        world: "minecraft:overworld".into(),
        tile_x: -1,
        tile_z: 2,
        level: 0,
        min_x: -256,
        min_z: 512,
        y: 64,
        step: 4,
        width: 64,
        height: 64,
        preset: MinecraftSeedPreset::LargeBiomes,
        generator_revision: "fixture".into(),
        profile_epoch: "a".repeat(32),
        sampled_at_ms: 1_000_000,
        expires_at_ms: 1_015_000,
        palette: vec!["minecraft:plains".into()],
        indices: vec![Some(0); 4096],
    }
}

#[test]
fn compact_runs_keep_exact_metadata_and_nulls() -> Result<(), MapError> {
    let mut tile = tile();
    tile.indices[..2048].fill(None);
    let frame = encode(&tile)?;
    assert_eq!(&frame[..9], b"CYBM\x01\x01\x00\x01\x00");
    assert_eq!(&frame[11..15], &(-1_i32).to_le_bytes());
    assert_eq!(&frame[15..19], &2_i32.to_le_bytes());
    assert_eq!(&frame[19..27], &1_000_000_u64.to_le_bytes());
    assert_eq!(&frame[27..29], &15_000_u16.to_le_bytes());
    // Two 2,048-cell runs: null then palette symbol1, each encoded in two bytes.
    assert_eq!(&frame[frame.len() - 4..], &[254, 31, 255, 31]);
    assert!(frame.len() < 100);
    Ok(())
}

#[test]
fn packing_supports_every_palette_entry_without_stealing_a_null_symbol() -> Result<(), MapError> {
    let mut tile = tile();
    tile.palette = (0..256)
        .map(|index| format!("minecraft:fixture_{index}"))
        .collect();
    tile.indices = (0..4096)
        .map(|index| {
            if index % 257 == 0 {
                None
            } else {
                Some((index % 257 - 1) as u16)
            }
        })
        .collect();
    let frame = encode(&tile)?;
    assert_eq!(frame[5], 0);
    assert_eq!(
        frame.len()
            - (33
                + tile.profile_epoch.len()
                + tile.generator_revision.len()
                + tile
                    .palette
                    .iter()
                    .map(|name| name.len() + 1)
                    .sum::<usize>()),
        4608
    );
    assert!(frame.len() <= MAX_BYTES);
    Ok(())
}

#[test]
fn invalid_lengths_indices_and_permissions_are_rejected() {
    let mut bad = tile();
    bad.indices.push(None);
    assert!(encode(&bad).is_err());
    let mut bad = tile();
    bad.indices[0] = Some(1);
    assert!(encode(&bad).is_err());
    let mut bad = tile();
    bad.expires_at_ms += 1;
    assert!(encode(&bad).is_err());
    let mut bad = tile();
    bad.palette[0] = "x".repeat(129);
    assert!(encode(&bad).is_err());
}

#[test]
fn an_entirely_hidden_tile_has_no_pixel_payload() -> Result<(), MapError> {
    let mut tile = tile();
    tile.indices.fill(None);
    tile.palette.clear();
    let frame = encode(&tile)?;
    assert_eq!(frame[5], 0);
    assert_eq!(
        frame.len(),
        33 + tile.profile_epoch.len() + tile.generator_revision.len()
    );
    Ok(())
}

#[test]
fn public_metadata_must_match_the_canonical_tile() {
    for mutate in [
        |tile: &mut MinecraftSeedTile| tile.width = 32,
        |tile: &mut MinecraftSeedTile| tile.height = 128,
        |tile: &mut MinecraftSeedTile| tile.min_x += 1,
        |tile: &mut MinecraftSeedTile| tile.min_z += 1,
        |tile: &mut MinecraftSeedTile| tile.step *= 2,
        |tile: &mut MinecraftSeedTile| tile.y = 320,
        |tile: &mut MinecraftSeedTile| tile.world = "minecraft:the_nether".into(),
        |tile: &mut MinecraftSeedTile| tile.profile_epoch = "short".into(),
        |tile: &mut MinecraftSeedTile| tile.palette[0] = "minecraft:bad biome".into(),
        |tile: &mut MinecraftSeedTile| tile.expires_at_ms = i64::MAX,
    ] {
        let mut tile = tile();
        mutate(&mut tile);
        assert!(encode(&tile).is_err());
    }
}
