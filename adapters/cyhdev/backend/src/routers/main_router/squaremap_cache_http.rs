//! Cache ordinary tile reads while delegating range and date-precondition semantics to ServeDir.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};

use super::tile_cache::TileCache;

/// The nested router has stripped the map prefix; queries do not create duplicate cache keys.
pub(super) async fn serve(
    State(cache): State<Arc<TileCache>>,
    request: Request,
    next: Next,
) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD)
        || [
            header::RANGE,
            header::IF_RANGE,
            header::IF_MATCH,
            header::IF_UNMODIFIED_SINCE,
        ]
        .iter()
        .any(|name| request.headers().contains_key(name))
        || (request.headers().contains_key(header::IF_MODIFIED_SINCE)
            && !request.headers().contains_key(header::IF_NONE_MATCH))
    {
        return next.run(request).await;
    }
    let key = request.uri().path().trim_start_matches('/');
    let tile = match cache.get(key).await {
        Ok(Some(tile)) => tile,
        Ok(None) => return next.run(request).await,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!(error = %error, "Tile cache read failed; falling back to file server");
            }
            return next.run(request).await;
        }
    };
    let etag = tile.version.etag();
    let validator = match HeaderValue::from_str(&etag) {
        Ok(value) => value,
        Err(_) => return next.run(request).await,
    };
    let not_modified = request
        .headers()
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .any(|value| match value.to_str() {
            Ok(value) => value.split(',').any(|candidate| {
                let candidate = candidate.trim();
                candidate == "*"
                    || candidate.trim_start_matches("W/") == etag.trim_start_matches("W/")
            }),
            Err(_) => false,
        });
    let length = tile.len();
    let mut response = if not_modified || request.method() == Method::HEAD {
        Response::new(Body::empty())
    } else {
        Response::new(Body::from(tile.body()))
    };
    response.headers_mut().insert(header::ETAG, validator);
    if not_modified {
        *response.status_mut() = StatusCode::NOT_MODIFIED;
    } else {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from(length));
        response
            .headers_mut()
            .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    }
    response
}
