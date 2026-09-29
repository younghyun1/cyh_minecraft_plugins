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
async fn private_controls_reject_missing_sessions_roles_and_origins() -> anyhow::Result<()> {
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
    let router = routes::<()>()
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
        let mut request = client
            .post(format!("http://{address}/api/admin/minecraft/actions"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(r#"{"action":"restart"}"#);
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
    server.abort();
    Ok(())
}

#[test]
fn arbitrary_commands_and_extra_fields_are_rejected() {
    for body in [
        r#"{"action":"console","command":"op somebody"}"#,
        r#"{"action":"restart","command":"stop"}"#,
        r#"{"action":"message","message":"hello","extra":true}"#,
        r#"{"action":"map_visibility","id":"@a","hidden":true}"#,
        r#"{"action":"map_visibility","id":"00000000-0000-4000-8000-000000000001","hidden":true,"command":"stop"}"#,
    ] {
        assert!(serde_json::from_str::<MinecraftAction>(body).is_err());
    }
}
