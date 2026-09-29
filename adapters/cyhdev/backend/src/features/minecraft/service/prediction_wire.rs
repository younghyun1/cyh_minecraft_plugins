//! Private seed/profile protocol; these values are never browser response types or log fields.

use crate::features::minecraft::domain::prediction::{
    CoverageState, PredictionCoverage, PredictionPreset, PredictionQuery,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ContextKind {
    PredictionContext,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Preset {
    Default,
    LargeBiomes,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum State {
    Generated,
    Ungenerated,
    Unknown,
    Excluded,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Coverage {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub state: State,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    kind: ContextKind,
    pub world: String,
    pub world_id: Uuid,
    pub sampled_at_ms: i64,
    pub seed: i64,
    pub preset: Preset,
    pub profile_revision: String,
    pub coverage: Vec<Coverage>,
}

impl Context {
    pub fn same_profile(&self, other: &Self) -> bool {
        self.world == other.world
            && self.world_id == other.world_id
            && self.seed == other.seed
            && self.preset == other.preset
            && self.profile_revision == other.profile_revision
    }

    pub fn valid_for(&self, query: &PredictionQuery, now_ms: i64) -> bool {
        let region = query.region();
        self.kind == ContextKind::PredictionContext
            && self.world == query.world
            && !self.world_id.is_nil()
            && (0..=5_000).contains(&(now_ms.saturating_sub(self.sampled_at_ms)))
            && (16..=128).contains(&self.profile_revision.len())
            && self
                .profile_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && self.coverage.len() == 64
            && self.coverage.iter().all(|cell| {
                (region.chunk_x..region.chunk_x + 8).contains(&cell.chunk_x)
                    && (region.chunk_z..region.chunk_z + 8).contains(&cell.chunk_z)
            })
            && self
                .coverage
                .iter()
                .map(|cell| (cell.chunk_x, cell.chunk_z))
                .collect::<BTreeSet<_>>()
                .len()
                == 64
    }

    pub fn public_preset(&self) -> PredictionPreset {
        match self.preset {
            Preset::Default => PredictionPreset::Default,
            Preset::LargeBiomes => PredictionPreset::LargeBiomes,
        }
    }

    pub fn public_coverage(self) -> Vec<PredictionCoverage> {
        self.coverage
            .into_iter()
            .map(|cell| PredictionCoverage {
                chunk_x: cell.chunk_x,
                chunk_z: cell.chunk_z,
                state: match cell.state {
                    State::Generated => CoverageState::Generated,
                    State::Ungenerated => CoverageState::Ungenerated,
                    State::Unknown => CoverageState::Unknown,
                    State::Excluded => CoverageState::Excluded,
                },
            })
            .collect()
    }
}

pub(super) fn request(query: &PredictionQuery) -> anyhow::Result<Vec<u8>> {
    let region = query.region();
    let mut bytes = serde_json::to_vec(&serde_json::json!({
        "kind":"prediction_context", "world":query.world,
        "chunk_x":region.chunk_x,"chunk_z":region.chunk_z,"width":8,"height":8,"y":query.y,
    }))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn parse(bytes: &[u8], query: &PredictionQuery, now_ms: i64) -> anyhow::Result<Context> {
    anyhow::ensure!(
        bytes.last() == Some(&b'\n'),
        "Incomplete prediction context"
    );
    let context: Context = serde_json::from_slice(bytes)?;
    anyhow::ensure!(
        context.valid_for(query, now_ms),
        "Invalid prediction context"
    );
    Ok(context)
}
