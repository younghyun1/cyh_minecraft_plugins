//! Invalid public tile inputs are rejected before a response is encoded.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Input bounds or metadata cannot be represented by the current wire version.
    #[error("invalid or unrepresentable biome tile")]
    Unavailable,
}
