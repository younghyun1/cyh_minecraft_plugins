//! Shared map colors mirror the browser's biomeColor rules.

/// Convert the public biome identifier into its map RGB color.
pub fn color(biome: &str) -> [u8; 3] {
    let explicit = match biome {
        "minecraft:nether_wastes" => Some(0x914646),
        "minecraft:soul_sand_valley" => Some(0x655044),
        "minecraft:crimson_forest" => Some(0xb1324c),
        "minecraft:warped_forest" => Some(0x338b80),
        "minecraft:basalt_deltas" => Some(0x666676),
        "minecraft:the_end" => Some(0xc8c28c),
        "minecraft:end_highlands" => Some(0xb6ae72),
        "minecraft:end_midlands" => Some(0xd1c895),
        "minecraft:small_end_islands" => Some(0xa293bc),
        "minecraft:end_barrens" => Some(0x716885),
        _ => None,
    };
    let rgb: u32 = if let Some(rgb) = explicit {
        rgb
    } else if biome.contains("ocean") || biome.ends_with(":river") {
        0x537fba
    } else if contains(biome, &["snow", "frozen", "ice", "grove"]) {
        0xd6e4e1
    } else if contains(biome, &["desert", "beach"]) {
        0xd4c28a
    } else if biome.contains("badlands") {
        0xb77852
    } else if biome.contains("swamp") {
        0x647c59
    } else if biome.contains("jungle") {
        0x3e7848
    } else if contains(biome, &["forest", "taiga"]) {
        0x62945b
    } else if contains(biome, &["plains", "meadow"]) {
        0x9aaf70
    } else if biome.contains("savanna") {
        0xb6ad67
    } else if contains(biome, &["peak", "stony", "mountain"]) {
        0x989f95
    } else {
        return fallback(biome);
    };
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

fn contains(biome: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| biome.contains(needle))
}

/// Match CSS hsl(hash % 360 62% 55%); identifiers are restricted to ASCII.
fn fallback(biome: &str) -> [u8; 3] {
    let hash = biome.bytes().fold(0_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    let hue = f64::from(hash % 360) / 60.0;
    let chroma = 0.558;
    let secondary = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let color = match hue as u8 {
        0 => [chroma, secondary, 0.0],
        1 => [secondary, chroma, 0.0],
        2 => [0.0, chroma, secondary],
        3 => [0.0, secondary, chroma],
        4 => [secondary, 0.0, chroma],
        _ => [chroma, 0.0, secondary],
    };
    color.map(|value| ((value + 0.271) * 255.0).round() as u8)
}
