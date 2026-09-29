//! Stable client-visible errors for Minecraft management.
use super::CodeError;
use axum::http::StatusCode;
use tracing::Level;

impl CodeError {
    pub const MINECRAFT_MAP_UNAVAILABLE: Self = Self {
        success: false,
        error_code: 89,
        http_status_code: StatusCode::SERVICE_UNAVAILABLE,
        message: "Minecraft map observations are unavailable.",
        log_level: Level::WARN,
    };
    pub const MINECRAFT_WAYPOINT_NOT_FOUND: Self = Self {
        success: false,
        error_code: 90,
        http_status_code: StatusCode::NOT_FOUND,
        message: "Minecraft waypoint was not found.",
        log_level: Level::INFO,
    };
    pub const MINECRAFT_WAYPOINT_CAPACITY: Self = Self {
        success: false,
        error_code: 91,
        http_status_code: StatusCode::CONFLICT,
        message: "This world already has 256 waypoints.",
        log_level: Level::INFO,
    };
    pub const MINECRAFT_DISABLED: Self = Self {
        success: false,
        error_code: 86,
        http_status_code: StatusCode::SERVICE_UNAVAILABLE,
        message: "Minecraft controls are not configured.",
        log_level: Level::INFO,
    };
    pub const MINECRAFT_BUSY: Self = Self {
        success: false,
        error_code: 87,
        http_status_code: StatusCode::TOO_MANY_REQUESTS,
        message: "A Minecraft operation is in progress or cooling down. Wait before retrying.",
        log_level: Level::INFO,
    };
    pub const MINECRAFT_UNCERTAIN: Self = Self {
        success: false,
        error_code: 88,
        http_status_code: StatusCode::SERVICE_UNAVAILABLE,
        message: "Minecraft did not acknowledge the request. Check its state before retrying.",
        log_level: Level::WARN,
    };
}
