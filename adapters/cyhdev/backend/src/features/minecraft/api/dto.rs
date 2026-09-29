//! Browser contracts for the finite set of Minecraft controls.
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MinecraftAction {
    Message { message: String },
    WhitelistAdd { name: String },
    WhitelistRemove { name: String },
    WhitelistEnable { enabled: bool },
    Kick { name: String },
    MapVisibility { id: uuid::Uuid, hidden: bool },
    Save {},
    Restart {},
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct MinecraftPlayer {
    pub id: uuid::Uuid,
    pub name: String,
    /// Null when the plugin is unavailable or the player is no longer online.
    pub map_hidden: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftStatus {
    pub players: Vec<MinecraftPlayer>,
    pub whitelist: Vec<MinecraftPlayer>,
    pub whitelist_enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub struct MinecraftActionResult {
    pub acknowledged: bool,
}
