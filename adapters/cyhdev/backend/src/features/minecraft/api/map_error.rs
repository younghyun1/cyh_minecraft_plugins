//! Stable HTTP errors for observations and waypoint administration.

use crate::{
    errors::code_error::{CodeError, CodeErrorResp, code_err},
    features::{accounts::api::map_authorization_error, minecraft::error::MapError},
};

pub(super) fn map_error(error: MapError) -> CodeErrorResp {
    let code = match error {
        MapError::Invalid => CodeError::INVALID_REQUEST,
        MapError::Disabled => CodeError::MINECRAFT_DISABLED,
        MapError::Busy => {
            return code_err(CodeError::MINECRAFT_BUSY, error)
                .with_retry_after(std::time::Duration::from_secs(1));
        }
        MapError::Unavailable => CodeError::MINECRAFT_MAP_UNAVAILABLE,
        MapError::NotFound => CodeError::MINECRAFT_WAYPOINT_NOT_FOUND,
        MapError::Full => CodeError::MINECRAFT_WAYPOINT_CAPACITY,
        MapError::Pool(_) => CodeError::POOL_ERROR,
        MapError::Query(_) => CodeError::DB_QUERY_ERROR,
        MapError::Authority(error) => return map_authorization_error(error),
    };
    code_err(code, error)
}
