//! Verify the actual HTTP layer with private binary biome tiles, without a world.

use crate::{
    features::minecraft::api::{
        seed_tile_binary, seed_tile_compression,
        seed_tile_dto::{MinecraftSeedPreset, MinecraftSeedTile},
    },
    routers::middleware::sensitive_response::sensitive_response_headers,
};
use axum::{Router, http::header, middleware::from_fn, routing::post};
use std::{
    error::Error,
    io::{Read, Write},
    sync::Arc,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Server {
    origin: String,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Server {
    async fn start(app: Router) -> TestResult<Self> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let task = tokio::spawn(async move { axum::serve(listener, app).await });
        Ok(Self { origin, task })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn binary() -> TestResult<Vec<u8>> {
    Ok(seed_tile_binary::encode(&MinecraftSeedTile {
        world: "minecraft:overworld".into(),
        tile_x: -1,
        tile_z: 2,
        level: 0,
        min_x: -256,
        min_z: 512,
        y: Some(64),
        step: 4,
        width: 64,
        height: 64,
        preset: MinecraftSeedPreset::Default,
        generator_revision: "http-fixture".into(),
        profile_epoch: "0123456789abcdef".into(),
        sampled_at_ms: 1_000_000,
        expires_at_ms: 1_015_000,
        palette: vec!["minecraft:plains".into(), "minecraft:forest".into()],
        indices: (0..4096).map(|index| Some((index % 2) as u16)).collect(),
    })?)
}

fn decoded(bytes: &[u8], encoding: Option<&str>) -> TestResult<Vec<u8>> {
    Ok(match encoding {
        Some("gzip") => {
            let mut output = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .take(seed_tile_binary::MAX_BYTES as u64 + 1)
                .read_to_end(&mut output)?;
            output
        }
        Some("zstd") => zstd::bulk::decompress(bytes, seed_tile_binary::MAX_BYTES)?,
        None => bytes.to_vec(),
        _ => return Err("unexpected response encoding".into()),
    })
}

#[tokio::test]
async fn binary_tiles_negotiate_gzip_only_without_losing_private_headers() -> TestResult {
    let expected = binary()?;
    let bytes = Arc::new(expected.clone());
    let app = Router::new()
        .route(
            "/seed-tile.bin",
            post(move || {
                let bytes = Arc::clone(&bytes);
                async move {
                    (
                        [
                            (header::CONTENT_TYPE, seed_tile_binary::CONTENT_TYPE),
                            (header::VARY, "Origin"),
                        ],
                        bytes.as_ref().clone(),
                    )
                }
            })
            .layer(seed_tile_compression::layer()),
        )
        .layer(from_fn(sensitive_response_headers))
        .layer(super::layer());
    let server = Server::start(app).await?;
    // Inspect bytes on the wire; the browser has its own native decoding path.
    let client = reqwest::Client::builder().no_gzip().no_zstd().build()?;
    for (accept, encoding) in [
        ("gzip", Some("gzip")),
        ("zstd", None),
        ("gzip, zstd", Some("gzip")),
        ("gzip;q=1, zstd;q=0.5", Some("gzip")),
        ("gzip;q=0.5, zstd;q=1", Some("gzip")),
        ("gzip, zstd;q=0", Some("gzip")),
        ("gzip;q=0, zstd", None),
        ("gzip;q=0.5, identity;q=1, zstd", None),
        ("identity", None),
    ] {
        let response = client
            .post(format!("{}/seed-tile.bin", server.origin))
            .header(header::ACCEPT_ENCODING, accept)
            .send()
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            seed_tile_binary::CONTENT_TYPE
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store, max-age=0"
        );
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|value| value.to_str())
                .transpose()?,
            encoding,
        );
        let varies = response
            .headers()
            .get_all(header::VARY)
            .iter()
            .map(|value| value.to_str())
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        assert!(
            varies
                .split(',')
                .any(|value| value.trim().eq_ignore_ascii_case("accept-encoding"))
        );
        assert!(
            varies
                .split(',')
                .any(|value| value.trim().eq_ignore_ascii_case("origin"))
        );
        assert_eq!(decoded(&response.bytes().await?, encoding)?, expected);
    }
    Ok(())
}

#[tokio::test]
async fn unrelated_responses_keep_global_zstd_negotiation() -> TestResult {
    let expected = binary()?;
    let bytes = expected.clone();
    let app = Router::new()
        .route(
            "/other",
            post(move || {
                let bytes = bytes.clone();
                async move { ([(header::CONTENT_TYPE, "application/octet-stream")], bytes) }
            }),
        )
        .layer(super::layer());
    let server = Server::start(app).await?;
    let response = reqwest::Client::builder()
        .no_gzip()
        .no_zstd()
        .build()?
        .post(format!("{}/other", server.origin))
        .header(header::ACCEPT_ENCODING, "gzip, zstd")
        .send()
        .await?;
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "zstd");
    assert_eq!(decoded(&response.bytes().await?, Some("zstd"))?, expected);
    Ok(())
}

#[tokio::test]
async fn already_encoded_binary_is_not_compressed_again() -> TestResult {
    let expected = binary()?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
    encoder.write_all(&expected)?;
    let encoded = encoder.finish()?;
    let bytes = Arc::new(encoded.clone());
    let app = Router::new()
        .route(
            "/seed-tile.bin",
            post(move || {
                let bytes = Arc::clone(&bytes);
                async move {
                    (
                        [
                            (header::CONTENT_TYPE, seed_tile_binary::CONTENT_TYPE),
                            (header::CONTENT_ENCODING, "gzip"),
                            (header::VARY, "Accept-Encoding"),
                        ],
                        bytes.as_ref().clone(),
                    )
                }
            })
            .layer(seed_tile_compression::layer()),
        )
        .layer(from_fn(sensitive_response_headers))
        .layer(super::layer());
    let server = Server::start(app).await?;
    let response = reqwest::Client::builder()
        .no_gzip()
        .no_zstd()
        .build()?
        .post(format!("{}/seed-tile.bin", server.origin))
        .header(header::ACCEPT_ENCODING, "gzip, zstd")
        .send()
        .await?;
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
    assert_eq!(response.headers()[header::VARY], "Accept-Encoding");
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "no-store, max-age=0"
    );
    let wire = response.bytes().await?;
    assert_eq!(wire.as_ref(), encoded);
    assert_eq!(decoded(&wire, Some("gzip"))?, expected);
    Ok(())
}
