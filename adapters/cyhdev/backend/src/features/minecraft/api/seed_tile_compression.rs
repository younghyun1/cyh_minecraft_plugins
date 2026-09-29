//! Binary map transport uses browser-native gzip consistently across clients.

use tower_http::compression::CompressionLayer;

/// Let the HTTP library honor gzip quality values and identity fallback.
pub(crate) fn layer() -> CompressionLayer {
    CompressionLayer::new()
        .gzip(true)
        .no_zstd()
        .no_br()
        .no_deflate()
}
