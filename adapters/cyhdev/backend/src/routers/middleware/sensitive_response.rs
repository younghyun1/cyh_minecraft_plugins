//! Response policy for authenticated or otherwise sensitive HTTP surfaces.

use axum::{
    body::Body,
    http::{HeaderValue, Request, Response, header},
    middleware::Next,
};

/// Prevents storage and referrer disclosure for successes and all downstream errors.
pub async fn sensitive_response_headers(request: Request<Body>, next: Next) -> Response<Body> {
    apply_sensitive_headers(next.run(request).await)
}

/// Applies the private policy to every admin result, including outer middleware failures.
pub async fn sensitive_admin_response_headers(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let is_admin = is_admin_path(request.uri().path());
    let response = next.run(request).await;
    if is_admin {
        apply_sensitive_headers(response)
    } else {
        response
    }
}

fn is_admin_path(path: &str) -> bool {
    path.starts_with("/api/admin/")
}

fn apply_sensitive_headers(mut response: Response<Body>) -> Response<Body> {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::{apply_sensitive_headers, is_admin_path};
    use axum::{
        body::Body,
        http::{HeaderValue, Response, header},
    };

    #[test]
    fn private_policy_sets_cache_and_referrer_headers() {
        let response = apply_sensitive_headers(Response::new(Body::empty()));
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store, max-age=0")),
        );
        assert_eq!(
            response.headers().get(header::REFERRER_POLICY),
            Some(&HeaderValue::from_static("no-referrer")),
        );
    }

    #[test]
    fn admin_path_match_does_not_cover_public_prefix_collisions() {
        assert!(is_admin_path("/api/admin/sync-i18n-cache"));
        assert!(!is_admin_path("/api/administer"));
        assert!(!is_admin_path("/api/i18n/ui-text"));
    }
}
