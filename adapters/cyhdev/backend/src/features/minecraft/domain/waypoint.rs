//! Public markers with bounded, independently validated administrator input.

use super::map_query::{horizontal, identifier, vertical};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WaypointInput {
    pub world: String,
    pub name: String,
    pub description: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Debug)]
pub struct Waypoint {
    pub id: Uuid,
    pub input: WaypointInput,
}

impl WaypointInput {
    pub fn valid(&self) -> bool {
        identifier(&self.world)
            && text(&self.name, 80)
            && !self.name.trim().is_empty()
            && text(&self.description, 500)
            && horizontal(self.x)
            && horizontal(self.z)
            && vertical(self.y)
    }
}

fn text(value: &str, maximum: usize) -> bool {
    value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_bounds_count_unicode_characters() {
        let mut value = WaypointInput {
            world: "minecraft:overworld".into(),
            name: "한".repeat(80),
            description: "家".repeat(500),
            x: 30_000_000,
            y: -2032,
            z: -30_000_000,
        };
        assert!(value.valid());
        value.name.push('한');
        assert!(!value.valid());
        value.name = "Home".into();
        value.description.push('\n');
        assert!(!value.valid());
        value.description.clear();
        value.y = 2032;
        assert!(!value.valid());
    }
}
