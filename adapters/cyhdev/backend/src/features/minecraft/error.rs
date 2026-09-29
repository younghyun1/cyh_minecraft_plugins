//! Map observation and marker persistence failures.

use crate::features::accounts::authorization_error::AuthorizationError;
use diesel::result::Error as DieselError;
use diesel_async::pooled_connection::bb8::RunError;

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("Invalid Minecraft map input")]
    Invalid,
    #[error("Minecraft world observations are not configured")]
    Disabled,
    #[error("Minecraft map query is running or cooling down")]
    Busy,
    #[error("Minecraft world observations are unavailable")]
    Unavailable,
    #[error("Waypoint was not found")]
    NotFound,
    #[error("This world already has 256 waypoints")]
    Full,
    #[error("Minecraft waypoint database pool unavailable")]
    Pool(#[source] RunError),
    #[error("Minecraft waypoint database query failed")]
    Query(#[from] DieselError),
    #[error(transparent)]
    Authority(#[from] AuthorizationError),
}

impl MapError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Busy | Self::Unavailable | Self::Pool(_))
            || matches!(
                self,
                Self::Query(DieselError::DatabaseError(
                    diesel::result::DatabaseErrorKind::SerializationFailure
                        | diesel::result::DatabaseErrorKind::ClosedConnection,
                    _
                ))
            )
            || matches!(self, Self::Authority(error) if error.is_retryable())
    }
}
