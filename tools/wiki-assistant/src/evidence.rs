//! Clarify simple crafting-grid markup while preserving the original source as evidence.

/// A single explicit recipe can be annotated without executing templates or guessing ingredients.
pub fn crafting(text: &str) -> String {
    if text.to_ascii_lowercase().matches("{{crafting").count() != 1 {
        return text.into();
    }
    let mut grid = ["empty"; 9];
    let mut found = false;
    for part in text.split('|').skip(1) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().as_bytes();
        if key.len() != 2 || !(b'A'..=b'C').contains(&key[0]) || !(b'1'..=b'3').contains(&key[1]) {
            continue;
        }
        let value = value
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('}')
            .trim();
        if value.is_empty() || value.len() > 80 || value.contains(['{', '[']) {
            return text.into();
        }
        grid[((key[1] - b'1') * 3 + key[0] - b'A') as usize] = value;
        found = true;
    }
    if !found {
        return text.into();
    }
    format!(
        "Crafting grid, rows from top to bottom; cells from left to right:\n{}\n{}\n{}\nOriginal source:\n{text}",
        grid[..3].join(" | "),
        grid[3..6].join(" | "),
        grid[6..].join(" | ")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn preserves_bucket_grid_orientation() {
        let source = "{{Crafting\n |A2= Iron Ingot\n |C2= Iron Ingot\n |B3= Iron Ingot\n |Output= Bucket\n}}";
        let text = super::crafting(source);
        assert!(text.contains(
            "empty | empty | empty\nIron Ingot | empty | Iron Ingot\nempty | Iron Ingot | empty"
        ));
        assert!(text.ends_with(source));
    }
}
