//! Bounded observations of generated terrain; no query may generate chunks.

pub const MIN_Y: i32 = -2032;
pub const MAX_Y: i32 = 2031;
pub const WORLD_EDGE: i32 = 30_000_000;

#[derive(Clone, Debug)]
pub enum MapQuery {
    Catalog,
    Area {
        world: String,
        region: Region,
        y: Option<i32>,
    },
    Blocks {
        world: String,
        region: Region,
        block: String,
        min_y: i32,
        max_y: i32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub width: u8,
    pub height: u8,
}

impl Region {
    /// Check the complete rectangle in widened arithmetic before calculating block positions.
    pub fn valid(self, maximum: u8) -> bool {
        (1..=maximum).contains(&self.width)
            && (1..=maximum).contains(&self.height)
            && [(self.chunk_x, self.width), (self.chunk_z, self.height)]
                .into_iter()
                .all(|(origin, size)| {
                    let start = i64::from(origin) * 16;
                    let end = start + i64::from(size) * 16 - 1;
                    start >= -i64::from(WORLD_EDGE) && end <= i64::from(WORLD_EDGE)
                })
    }

    pub fn contains(self, x: i32, z: i32) -> bool {
        let x = i64::from(x) - i64::from(self.chunk_x) * 16;
        let z = i64::from(z) - i64::from(self.chunk_z) * 16;
        x >= 0 && z >= 0 && x < i64::from(self.width) * 16 && z < i64::from(self.height) * 16
    }
}

impl MapQuery {
    pub fn valid(&self) -> bool {
        match self {
            Self::Catalog => true,
            Self::Area { world, region, y } => {
                identifier(world) && region.valid(8) && y.is_none_or(vertical)
            }
            Self::Blocks {
                world,
                region,
                block,
                min_y,
                max_y,
            } => {
                identifier(world)
                    && identifier(block)
                    && region.valid(4)
                    && vertical(*min_y)
                    && vertical(*max_y)
                    && min_y <= max_y
                    && i64::from(*max_y) - i64::from(*min_y) < 512
            }
        }
    }
}

pub fn vertical(y: i32) -> bool {
    (MIN_Y..=MAX_Y).contains(&y)
}
pub fn horizontal(value: i32) -> bool {
    (-WORLD_EDGE..=WORLD_EDGE).contains(&value)
}

/// Minecraft resource keys use separate namespace and path alphabets.
pub fn identifier(value: &str) -> bool {
    if value.len() > 128 {
        return false;
    }
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    let base =
        |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte);
    !namespace.is_empty()
        && !path.is_empty()
        && namespace.bytes().all(base)
        && path.bytes().all(|byte| base(byte) || byte == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_keys_and_complete_rectangles_are_bounded() {
        assert!(identifier("minecraft:overworld"));
        for value in [
            "",
            "world",
            "minecraft:",
            "Minecraft:world",
            "a:b:c",
            "a:b\n",
        ] {
            assert!(!identifier(value));
        }
        assert!(
            Region {
                chunk_x: -1_875_000,
                chunk_z: 0,
                width: 8,
                height: 8
            }
            .valid(8)
        );
        assert!(
            !Region {
                chunk_x: 1_875_000,
                chunk_z: 0,
                width: 1,
                height: 1
            }
            .valid(8)
        );
        assert!(
            !Region {
                chunk_x: i32::MAX,
                chunk_z: 0,
                width: 1,
                height: 1
            }
            .valid(8)
        );
        let query = |max_y| MapQuery::Blocks {
            world: "minecraft:overworld".into(),
            region: Region {
                chunk_x: 0,
                chunk_z: 0,
                width: 4,
                height: 4,
            },
            block: "minecraft:stone".into(),
            min_y: -64,
            max_y,
        };
        assert!(query(447).valid());
        assert!(!query(448).valid());
        assert!(!query(-65).valid());
    }
}
