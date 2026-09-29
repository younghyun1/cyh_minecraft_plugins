//! Private generator identity and bounded visibility geometry from the live plugin.

use crate::features::minecraft::domain::seed_tile::{SeedDimension, SeedPreset};
use serde::Deserialize;
use uuid::Uuid;

pub(super) const REFRESH_MS: i64 = 5_000;
pub(super) const PERMISSION_MS: i64 = 15_000;

#[derive(Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    SeedProfile,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(super) enum Preset {
    Default,
    LargeBiomes,
    Nether,
    End,
}

impl Preset {
    pub fn dimension(self) -> SeedDimension {
        match self {
            Self::Default | Self::LargeBiomes => SeedDimension::Overworld,
            Self::Nether => SeedDimension::Nether,
            Self::End => SeedDimension::End,
        }
    }
    pub fn public(self) -> SeedPreset {
        match self {
            Self::Default => SeedPreset::Default,
            Self::LargeBiomes => SeedPreset::LargeBiomes,
            Self::Nether => SeedPreset::Nether,
            Self::End => SeedPreset::End,
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Rectangle {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl Rectangle {
    fn valid(&self) -> bool {
        [self.min_x, self.min_z, self.max_x, self.max_z]
            .into_iter()
            .all(|n| (-30_000_000..=30_000_000).contains(&n))
            && self.min_x <= self.max_x
            && self.min_z <= self.max_z
    }
    fn contains(&self, x: i64, z: i64, end_x: i64, end_z: i64) -> bool {
        x >= i64::from(self.min_x)
            && z >= i64::from(self.min_z)
            && end_x <= i64::from(self.max_x)
            && end_z <= i64::from(self.max_z)
    }
}

#[derive(Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Shape {
    Rectangle {
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    },
    Circle {
        center_x: i32,
        center_z: i32,
        radius: u32,
    },
}

impl Shape {
    fn valid(&self) -> bool {
        match *self {
            Self::Rectangle {
                min_x,
                min_z,
                max_x,
                max_z,
            } => Rectangle {
                min_x,
                min_z,
                max_x,
                max_z,
            }
            .valid(),
            Self::Circle {
                center_x,
                center_z,
                radius,
            } => {
                (1..=46_340).contains(&radius)
                    && [center_x, center_z]
                        .into_iter()
                        .all(|n| (-30_000_000..=30_000_000).contains(&n))
            }
        }
    }
    fn contains(&self, x: i64, z: i64, end_x: i64, end_z: i64) -> bool {
        match *self {
            Self::Rectangle {
                min_x,
                min_z,
                max_x,
                max_z,
            } => Rectangle {
                min_x,
                min_z,
                max_x,
                max_z,
            }
            .contains(x, z, end_x, end_z),
            Self::Circle {
                center_x,
                center_z,
                radius,
            } => {
                let dx = (x - i64::from(center_x))
                    .abs()
                    .max((end_x - i64::from(center_x)).abs());
                let dz = (z - i64::from(center_z))
                    .abs()
                    .max((end_z - i64::from(center_z)).abs());
                dx * dx + dz * dz <= i64::from(radius) * i64::from(radius)
            }
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    kind: Kind,
    pub world: String,
    pub world_id: Uuid,
    pub sampled_at_ms: i64,
    pub seed: i64,
    pub preset: Preset,
    pub profile_revision: String,
    pub world_border: Rectangle,
    pub visibility: Vec<Shape>,
}

impl Profile {
    pub fn same(&self, other: &Self) -> bool {
        self.world == other.world
            && self.world_id == other.world_id
            && self.seed == other.seed
            && self.preset == other.preset
            && self.profile_revision == other.profile_revision
            && self.world_border == other.world_border
            && self.visibility == other.visibility
    }
    pub fn fresh(&self, now: i64) -> bool {
        (0..REFRESH_MS).contains(&now.saturating_sub(self.sampled_at_ms))
    }
    pub fn permits(&self, x: i64, z: i64, step: u32) -> bool {
        let end_x = x + i64::from(step) - 1;
        let end_z = z + i64::from(step) - 1;
        self.world_border.contains(x, z, end_x, end_z)
            && (self.visibility.is_empty()
                || self
                    .visibility
                    .iter()
                    .any(|shape| shape.contains(x, z, end_x, end_z)))
    }
    pub fn parse(bytes: &[u8], world: &str, now: i64) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() <= 32_768 && bytes.last() == Some(&b'\n'),
            "Invalid seed profile frame"
        );
        let profile: Self = serde_json::from_slice(bytes)?;
        anyhow::ensure!(
            profile.kind == Kind::SeedProfile
                && profile.world == world
                && profile.preset.dimension().world() == world
                && !profile.world_id.is_nil()
                && profile.fresh(now)
                && (16..=128).contains(&profile.profile_revision.len())
                && profile
                    .profile_revision
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
                && profile.world_border.valid()
                && profile.visibility.len() <= 64
                && profile.visibility.iter().all(Shape::valid),
            "Unsupported seed profile"
        );
        Ok(profile)
    }
}
