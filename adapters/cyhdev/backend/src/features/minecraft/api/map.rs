//! Public observations and markers; marker mutations inherit the existing administrator guards.

use super::{
    dto::MinecraftActionResult,
    map_dto::{
        MinecraftMapQuery, MinecraftWaypoint, MinecraftWaypointInput, MinecraftWaypointQuery,
    },
    map_error::map_error,
    map_response::MinecraftMapData,
    prediction_dto::{MinecraftPrediction, MinecraftPredictionQuery},
    seed_tile_dto::{MinecraftSeedTile, MinecraftSeedTileQuery},
};
use crate::{
    dto::responses::response_data::http_resp,
    errors::code_error::{CodeErrorResp, HandlerResponse},
    features::minecraft::service::seed_tile::SeedTileService,
    features::minecraft::service::world_query::WorldQueryService,
    init::state::ServerState,
    util::time::now::tokio_now,
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query},
    response::IntoResponse,
    routing::{get, patch, post},
};
use std::sync::Arc;
use uuid::Uuid;

pub fn public_router(state: &Arc<ServerState>) -> anyhow::Result<Router<Arc<ServerState>>> {
    let world = Arc::new(WorldQueryService::from_environment()?);
    let seed = Arc::new(SeedTileService::new(Arc::clone(&world))?);
    Ok(Router::new()
        .route("/api/minecraft/map/query", post(minecraft_map_query))
        .route(
            "/api/minecraft/map/seed-tile",
            post(minecraft_map_seed_tile),
        )
        .route(
            "/api/minecraft/map/seed-tile.bin",
            post(minecraft_map_seed_tile_binary).layer(super::seed_tile_compression::layer()),
        )
        .route(
            "/api/minecraft/map/seed-tile.png",
            post(minecraft_map_seed_tile_png),
        )
        .route(
            "/api/minecraft/map/prediction",
            post(minecraft_map_prediction),
        )
        .route("/api/minecraft/map/waypoints", get(minecraft_map_waypoints))
        .layer(DefaultBodyLimit::max(4096))
        .layer(Extension(world))
        .layer(Extension(seed))
        .layer(Extension(state.minecraft_waypoint_service()))
        .layer(axum::middleware::from_fn(
            crate::routers::middleware::sensitive_response::sensitive_response_headers,
        )))
}

pub fn admin_router(state: &Arc<ServerState>) -> Router<Arc<ServerState>> {
    admin_routes().layer(Extension(state.minecraft_waypoint_service()))
}

fn admin_routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route(
            "/api/admin/minecraft/map/waypoints",
            post(create_minecraft_map_waypoint),
        )
        .route(
            "/api/admin/minecraft/map/waypoints/{waypoint_id}",
            patch(update_minecraft_map_waypoint).delete(delete_minecraft_map_waypoint),
        )
        .layer(DefaultBodyLimit::max(4096))
}

type Waypoints = Arc<crate::features::minecraft::service::waypoints::WaypointService>;

#[utoipa::path(post, path = "/api/minecraft/map/seed-tile.png", tag = "minecraft", request_body = MinecraftSeedTileQuery, responses((status = 200, description = "64 by 64 PNG with transparent visibility mask and private cyBM metadata chunk", content_type = "image/png"), (status = 400, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_map_seed_tile_png(
    Extension(service): Extension<Arc<SeedTileService>>,
    Json(request): Json<MinecraftSeedTileQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let tile = service.tile(request.into()).await.map_err(map_error)?;
    let bytes = super::seed_tile_render::png(MinecraftSeedTile::from(tile))
        .await
        .map_err(map_error)?;
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            super::seed_tile_png::CONTENT_TYPE,
        )],
        bytes,
    ))
}

#[utoipa::path(post, path = "/api/minecraft/map/seed-tile.bin", tag = "minecraft", request_body = MinecraftSeedTileQuery, responses((status = 200, description = "CYBM v1 palette tile with bounded bit-packed or run-length indices", content_type = "application/vnd.cyhdev.biome-tile"), (status = 400, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_map_seed_tile_binary(
    Extension(service): Extension<Arc<SeedTileService>>,
    Json(request): Json<MinecraftSeedTileQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let tile = service.tile(request.into()).await.map_err(map_error)?;
    let bytes =
        super::seed_tile_binary::encode(&MinecraftSeedTile::from(tile)).map_err(map_error)?;
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            super::seed_tile_binary::CONTENT_TYPE,
        )],
        bytes,
    ))
}

#[utoipa::path(post, path = "/api/minecraft/map/seed-tile", tag = "minecraft", request_body = MinecraftSeedTileQuery, responses((status = 200, body = MinecraftSeedTile), (status = 400, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_map_seed_tile(
    Extension(service): Extension<Arc<SeedTileService>>,
    Json(request): Json<MinecraftSeedTileQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let tile = service.tile(request.into()).await.map_err(map_error)?;
    Ok(http_resp(MinecraftSeedTile::from(tile), (), start))
}

#[utoipa::path(post, path = "/api/minecraft/map/prediction", tag = "minecraft", request_body = MinecraftPredictionQuery, responses((status = 200, body = MinecraftPrediction), (status = 400, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_map_prediction(
    Extension(service): Extension<Arc<WorldQueryService>>,
    Json(request): Json<MinecraftPredictionQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let data = service.predict(request.into()).await.map_err(map_error)?;
    Ok(http_resp(MinecraftPrediction::from(data), (), start))
}

#[utoipa::path(post, path = "/api/minecraft/map/query", tag = "minecraft", request_body = MinecraftMapQuery, responses((status = 200, body = MinecraftMapData), (status = 400, body = CodeErrorResp), (status = 429, body = CodeErrorResp), (status = 503, body = CodeErrorResp)))]
pub async fn minecraft_map_query(
    Extension(service): Extension<Arc<WorldQueryService>>,
    Json(request): Json<MinecraftMapQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let data = service.query(request.into()).await.map_err(map_error)?;
    Ok(http_resp(MinecraftMapData::from(data), (), start))
}

#[utoipa::path(get, path = "/api/minecraft/map/waypoints", tag = "minecraft", params(MinecraftWaypointQuery), responses((status = 200, body = Vec<MinecraftWaypoint>), (status = 400, body = CodeErrorResp)))]
pub async fn minecraft_map_waypoints(
    Extension(service): Extension<Waypoints>,
    Query(query): Query<MinecraftWaypointQuery>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let data = service.list(&query.world).await.map_err(map_error)?;
    Ok(http_resp(
        data.into_iter()
            .map(MinecraftWaypoint::from)
            .collect::<Vec<_>>(),
        (),
        start,
    ))
}

#[utoipa::path(post, path = "/api/admin/minecraft/map/waypoints", tag = "admin", request_body = MinecraftWaypointInput, responses((status = 200, body = MinecraftWaypoint), (status = 400, body = CodeErrorResp), (status = 401, body = CodeErrorResp), (status = 403, body = CodeErrorResp), (status = 409, body = CodeErrorResp)))]
pub async fn create_minecraft_map_waypoint(
    Extension(actor): Extension<Uuid>,
    Extension(service): Extension<Waypoints>,
    Json(request): Json<MinecraftWaypointInput>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let data = service
        .save(actor, None, request.into())
        .await
        .map_err(map_error)?;
    Ok(http_resp(MinecraftWaypoint::from(data), (), start))
}

#[utoipa::path(patch, path = "/api/admin/minecraft/map/waypoints/{waypoint_id}", tag = "admin", params(("waypoint_id" = Uuid, Path)), request_body = MinecraftWaypointInput, responses((status = 200, body = MinecraftWaypoint), (status = 400, body = CodeErrorResp), (status = 401, body = CodeErrorResp), (status = 403, body = CodeErrorResp), (status = 404, body = CodeErrorResp), (status = 409, body = CodeErrorResp)))]
pub async fn update_minecraft_map_waypoint(
    Extension(actor): Extension<Uuid>,
    Extension(service): Extension<Waypoints>,
    Path(id): Path<Uuid>,
    Json(request): Json<MinecraftWaypointInput>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    let data = service
        .save(actor, Some(id), request.into())
        .await
        .map_err(map_error)?;
    Ok(http_resp(MinecraftWaypoint::from(data), (), start))
}

#[utoipa::path(delete, path = "/api/admin/minecraft/map/waypoints/{waypoint_id}", tag = "admin", params(("waypoint_id" = Uuid, Path)), responses((status = 200, body = MinecraftActionResult), (status = 400, body = CodeErrorResp), (status = 401, body = CodeErrorResp), (status = 403, body = CodeErrorResp), (status = 404, body = CodeErrorResp)))]
pub async fn delete_minecraft_map_waypoint(
    Extension(actor): Extension<Uuid>,
    Extension(service): Extension<Waypoints>,
    Path(id): Path<Uuid>,
) -> HandlerResponse<impl IntoResponse> {
    let start = tokio_now();
    service.delete(actor, id).await.map_err(map_error)?;
    Ok(http_resp(
        MinecraftActionResult { acknowledged: true },
        (),
        start,
    ))
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
