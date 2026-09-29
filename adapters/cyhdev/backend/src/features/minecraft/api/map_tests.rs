use super::*;
use crate::{
    features::accounts::{
        domain::{account::LoginAccount, role::RoleType, session::SESSION_COOKIE_NAME},
        service::session_service::SessionService,
    },
    init::state::{DeploymentEnvironment, PublicAppOrigin},
    routers::middleware::{
        auth::auth_middleware,
        role::require_superuser_middleware,
        sensitive_response::sensitive_admin_response_headers,
        trusted_origin::{TrustedOrigins, require_trusted_origin},
    },
};
use axum::{
    http::{StatusCode, header},
    middleware::{from_fn, from_fn_with_state},
};

/// Exercise the handlers behind the authentication/origin layers used by the root router.
#[tokio::test]
async fn waypoints_reject_missing_sessions_roles_and_origins() -> anyhow::Result<()> {
    let sessions = Arc::new(SessionService::new());
    let account = LoginAccount {
        user_id: Uuid::new_v4(),
        user_name: "control-test".into(),
        password_hash: "unused".into(),
        is_email_verified: true,
        country: 840,
        language: 1,
    };
    let user = sessions
        .create(&account, RoleType::User, None, None)
        .await?;
    let admin = sessions
        .create(&account, RoleType::Younghyun, None, None)
        .await?;
    let origin = PublicAppOrigin::parse(Some("https://cyhdev.com"), DeploymentEnvironment::Prod)?;
    let origins = Arc::new(TrustedOrigins::from_environment(
        DeploymentEnvironment::Prod,
        &origin,
    )?);
    let router = admin_routes::<()>()
        .layer(from_fn(require_superuser_middleware))
        .layer(from_fn_with_state(sessions, auth_middleware))
        .layer(from_fn_with_state(origins, require_trusted_origin))
        .layer(from_fn(sensitive_admin_response_headers));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let client = reqwest::Client::new();
    for (token, origin, expected) in [
        (None, Some("https://cyhdev.com"), StatusCode::UNAUTHORIZED),
        (
            Some(user.expose()),
            Some("https://cyhdev.com"),
            StatusCode::FORBIDDEN,
        ),
        (
            Some(admin.expose()),
            Some("https://untrusted.example"),
            StatusCode::FORBIDDEN,
        ),
        (Some(admin.expose()), None, StatusCode::FORBIDDEN),
    ] {
        for (method, path) in [
            (
                reqwest::Method::POST,
                "/api/admin/minecraft/map/waypoints".to_owned(),
            ),
            (
                reqwest::Method::PATCH,
                format!("/api/admin/minecraft/map/waypoints/{}", Uuid::new_v4()),
            ),
            (
                reqwest::Method::DELETE,
                format!("/api/admin/minecraft/map/waypoints/{}", Uuid::new_v4()),
            ),
        ] {
            let mut request = client
            .request(method, format!("http://{address}{path}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(r#"{"world":"minecraft:overworld","name":"Home","description":"","x":0,"y":64,"z":0}"#);
            if let Some(token) = token {
                request = request.header(header::COOKIE, format!("{SESSION_COOKIE_NAME}={token}"));
            }
            if let Some(origin) = origin {
                request = request.header(header::ORIGIN, origin);
            }
            let response = request.send().await?;
            assert_eq!(response.status(), expected);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "no-store, max-age=0"
            );
        }
    }
    server.abort();
    Ok(())
}

#[test]
fn map_queries_reject_arbitrary_operations_and_fields() {
    for value in [
        r#"{"kind":"console"}"#,
        r#"{"kind":"catalog","extra":true}"#,
        r#"{"kind":"area","world":"minecraft:overworld","chunk_x":0,"chunk_z":0,"width":256,"height":1}"#,
    ] {
        assert!(serde_json::from_str::<MinecraftMapQuery>(value).is_err());
    }
}
