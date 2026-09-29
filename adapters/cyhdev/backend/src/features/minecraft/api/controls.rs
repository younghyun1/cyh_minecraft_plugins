//! Session-protected HTTP handlers; management credentials never cross this boundary.

use super::super::{
    domain::command::Command,
    service::management::{ManagementError, ManagementService, Player},
};
use super::dto::{MinecraftAction, MinecraftActionResult, MinecraftPlayer, MinecraftStatus};
use crate::{
    dto::responses::response_data::http_resp,
    errors::code_error::{CodeError, CodeErrorResp, HandlerResponse, code_err},
    features::accounts::api::map_authorization_error,
    init::state::ServerState,
    util::time::now::tokio_now,
};
use axum::{
    Extension, Json, Router,
    extract::DefaultBodyLimit,
    response::IntoResponse,
    routing::{get, post},
};
use std::sync::Arc;
use uuid::Uuid;

/// The composition root merges these routes inside the existing superuser/origin guards.
pub fn router(state: &Arc<ServerState>) -> anyhow::Result<Router<Arc<ServerState>>> {
    let service = Arc::new(ManagementService::from_environment(
        state.account_service(),
    )?);
    Ok(routes().layer(Extension(service)))
}

fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/admin/minecraft", get(minecraft_status))
        .route("/api/admin/minecraft/actions", post(minecraft_action))
        .layer(DefaultBodyLimit::max(4096))
}

#[cfg(test)]
#[path = "controls_tests.rs"]
mod tests;

#[utoipa::path(get, path = "/api/admin/minecraft", tag = "admin", responses((status = 200, body = MinecraftStatus), (status = 401, body = CodeErrorResp), (status = 403, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_status(
    Extension(actor): Extension<Uuid>,
    Extension(service): Extension<Arc<ManagementService>>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let status = service.status(actor).await.map_err(map_error)?;
    Ok(http_resp(
        MinecraftStatus {
            players: status.players.into_iter().map(player).collect(),
            whitelist: status.whitelist.into_iter().map(player).collect(),
            whitelist_enabled: status.whitelist_enabled,
        },
        (),
        start,
    ))
}

#[utoipa::path(post, path = "/api/admin/minecraft/actions", tag = "admin", request_body = MinecraftAction, responses((status = 200, body = MinecraftActionResult), (status = 400, body = CodeErrorResp), (status = 401, body = CodeErrorResp), (status = 403, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_action(
    Extension(actor): Extension<Uuid>,
    Extension(service): Extension<Arc<ManagementService>>,
    Json(request): Json<MinecraftAction>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let command = match request {
        MinecraftAction::Message { message } => Command::Message(message),
        MinecraftAction::WhitelistAdd { name } => Command::WhitelistAdd(name),
        MinecraftAction::WhitelistRemove { name } => Command::WhitelistRemove(name),
        MinecraftAction::WhitelistEnable { enabled } => Command::WhitelistEnable(enabled),
        MinecraftAction::Kick { name } => Command::Kick(name),
        MinecraftAction::MapVisibility { id, hidden } => Command::MapVisibility { id, hidden },
        MinecraftAction::Save {} => Command::Save,
        MinecraftAction::Restart {} => Command::Restart,
    };
    service.execute(actor, command).await.map_err(map_error)?;
    Ok(http_resp(
        MinecraftActionResult { acknowledged: true },
        (),
        start,
    ))
}

fn player(value: Player) -> MinecraftPlayer {
    MinecraftPlayer {
        id: value.id,
        name: value.name,
        map_hidden: value.map_hidden,
    }
}

fn map_error(error: ManagementError) -> CodeErrorResp {
    let code = match error {
        ManagementError::Authority(error) => return map_authorization_error(error),
        ManagementError::Invalid => return code_err(CodeError::INVALID_REQUEST, error),
        ManagementError::Disabled => CodeError::MINECRAFT_DISABLED,
        ManagementError::Busy => CodeError::MINECRAFT_BUSY,
        ManagementError::Uncertain => CodeError::MINECRAFT_UNCERTAIN,
    };
    code_err(code, error)
}
