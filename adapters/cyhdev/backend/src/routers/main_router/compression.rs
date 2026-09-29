//! Shared response compression follows browser HTTP content-encoding negotiation.

use crate::features::minecraft::api::seed_tile_binary;
use tower_http::compression::{
    CompressionLayer,
    predicate::{And, DefaultPredicate, NotForContentType, Predicate},
};

/// Binary map tiles negotiate gzip at their route; an identity fallback stays identity.
pub(super) fn layer() -> CompressionLayer<And<DefaultPredicate, NotForContentType>> {
    CompressionLayer::new().zstd(true).gzip(true).compress_when(
        DefaultPredicate::new().and(NotForContentType::const_new(seed_tile_binary::CONTENT_TYPE)),
    )
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
