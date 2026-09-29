//! Filesystem-backed squaremap assets, isolated from session and API middleware.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    middleware::{Next, from_fn, from_fn_with_state},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use tower_http::services::ServeDir;

#[path = "squaremap_cache.rs"]
mod tile_cache;
#[path = "squaremap_cache_http.rs"]
mod tile_cache_http;

/// Read the public asset root once at startup; absent assets return an explicit failure.
pub(super) fn from_environment() -> anyhow::Result<Router> {
    let root = match std::env::var_os("SQUAREMAP_WEB_DIR") {
        Some(value) if !value.is_empty() => {
            let path = PathBuf::from(value);
            anyhow::ensure!(path.is_absolute(), "SQUAREMAP_WEB_DIR must be absolute");
            Some(path)
        }
        Some(_) => anyhow::bail!("SQUAREMAP_WEB_DIR must not be empty"),
        None => None,
    };
    Ok(router(root))
}

/// Mount only the plugin's public web directory, never the Minecraft server directory.
fn router(root: Option<PathBuf>) -> Router {
    let files = match root {
        Some(root) => {
            let cache = Arc::new(tile_cache::TileCache::new(root.clone()));
            Router::new()
                .fallback_service(ServeDir::new(root))
                .layer(from_fn_with_state(cache, tile_cache_http::serve))
        }
        None => Router::new().fallback(unavailable),
    }
    .route("/", get(retired_map))
    .route("/index.html", get(retired_map))
    .layer(from_fn(native_assets_only));
    Router::new()
        .route("/minecraft/map", get(retired_map))
        .nest("/minecraft/map/", files)
        .layer(from_fn(cache_headers))
}

/// Old bookmarks lead to the native explorer; the retired JavaScript interface is not served.
async fn retired_map() -> Redirect {
    Redirect::permanent("/minecraft")
}

/// Preserve only the native explorer's public data and raster images.
async fn native_assets_only(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let relative = path.trim_start_matches('/');
    let canonical = relative.len() <= 512
        && relative.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        });
    let asset = canonical
        && ((relative.starts_with("tiles/")
            && (relative.ends_with(".png") || relative.ends_with(".json")))
            || (relative.starts_with("images/") && relative.ends_with(".png")));
    if matches!(path, "/" | "/index.html") || asset {
        next.run(request).await
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Keep missing assets explicit without serving the SPA recursively.
async fn unavailable() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "The Minecraft map is not available yet.",
    )
}

/// Squaremap rewrites stable filenames; immutable caching would freeze map updates.
async fn cache_headers(request: Request, next: Next) -> Response {
    let live_data = request.uri().path().ends_with(".json");
    let mut response = next.run(request).await;
    let policy = if live_data
        || response.status().is_client_error()
        || response.status().is_server_error()
    {
        "no-store"
    } else {
        "public, no-cache"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

#[cfg(test)]
#[path = "squaremap_tests.rs"]
mod tests;
