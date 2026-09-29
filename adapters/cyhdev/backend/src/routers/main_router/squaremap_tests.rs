use std::error::Error;

use axum::{Router, http::StatusCode};
use reqwest::{Client, header};

/// Exercise the mounted router over HTTP, including the enclosing SPA fallback.
#[tokio::test]
async fn redirects_retired_documents_and_serves_only_native_assets() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let web = directory.path().join("web");
    std::fs::create_dir(&web)?;
    std::fs::write(web.join("index.html"), "<title>Map</title>")?;
    std::fs::write(web.join("style.css"), "body{}")?;
    std::fs::write(web.join("app.js"), "alert('retired')")?;
    std::fs::create_dir(web.join("tiles"))?;
    std::fs::write(web.join("tiles/players.json"), "[]")?;
    for directory in ["icon", "health", "armor"] {
        std::fs::create_dir_all(web.join("images").join(directory))?;
        let name = if directory == "icon" {
            "player.png"
        } else {
            "0.png"
        };
        std::fs::write(web.join("images").join(directory).join(name), b"icon")?;
    }
    std::fs::write(directory.path().join("secret.txt"), "private")?;
    let app = Router::new()
        .merge(super::router(Some(web)))
        .fallback(|| async { "website shell" });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    for path in [
        "/minecraft/map",
        "/minecraft/map/",
        "/minecraft/map/index.html",
    ] {
        let redirect = client.get(format!("{origin}{path}")).send().await?;
        assert_eq!(redirect.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(redirect.headers()[header::LOCATION], "/minecraft");
        assert_eq!(
            redirect.headers()[header::CACHE_CONTROL],
            "public, no-cache"
        );
        assert_eq!(
            redirect.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
    }
    let icon = client
        .get(format!("{origin}/minecraft/map/images/icon/player.png"))
        .send()
        .await?;
    let modified = icon.headers()[header::LAST_MODIFIED].clone();
    assert_eq!(icon.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(icon.bytes().await?.as_ref(), b"icon");
    let conditional = client
        .get(format!("{origin}/minecraft/map/images/icon/player.png"))
        .header(header::IF_MODIFIED_SINCE, modified)
        .send()
        .await?;
    assert_eq!(conditional.status(), StatusCode::NOT_MODIFIED);
    for path in ["health/0.png", "armor/0.png"] {
        let image = client
            .get(format!("{origin}/minecraft/map/images/{path}"))
            .send()
            .await?;
        assert_eq!(image.status(), StatusCode::OK);
        assert_eq!(image.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(image.bytes().await?.as_ref(), b"icon");
    }
    let players = client
        .get(format!("{origin}/minecraft/map/tiles/players.json"))
        .send()
        .await?;
    assert_eq!(players.status(), StatusCode::OK);
    assert_eq!(players.headers()[header::CACHE_CONTROL], "no-store");
    for path in [
        "style.css",
        "app.js",
        "tiles/other.html",
        "tiles/other.js",
        "tiles/missing.png",
        "..%2fsecret.txt",
        "images/../secret.txt",
        "tiles/%2e%2e/secret.txt",
    ] {
        let response = client
            .get(format!("{origin}/minecraft/map/{path}"))
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.text().await?.contains("website shell"));
    }
    let head = client
        .head(format!("{origin}/minecraft/map/index.html"))
        .send()
        .await?;
    assert_eq!(head.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(head.headers()[header::LOCATION], "/minecraft");
    assert!(head.bytes().await?.is_empty());
    server.abort();
    Ok(())
}

#[tokio::test]
async fn unconfigured_map_is_explicitly_unavailable() -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let url = format!(
        "http://{}/minecraft/map/tiles/settings.json",
        listener.local_addr()?
    );
    let server = tokio::spawn(async move { axum::serve(listener, super::router(None)).await });
    let response = Client::new().get(url).send().await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    server.abort();
    Ok(())
}

/// Verify that cache hits preserve HTTP behavior and never cache squaremap's live JSON.
#[tokio::test]
async fn tile_cache_validators_ranges_and_updates() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    tokio::fs::create_dir(directory.path().join("tiles")).await?;
    let tile_path = directory.path().join("tiles/a.png");
    let players_path = directory.path().join("tiles/players.json");
    tokio::fs::write(&tile_path, b"tile").await?;
    tokio::fs::write(&players_path, b"[]").await?;
    let app = super::router(Some(directory.path().to_owned()));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let origin = format!("http://{}/minecraft/map", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let client = Client::new();
    let url = format!("{origin}/tiles/a.png");
    let initial = client.get(&url).send().await?;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(initial.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(initial.headers()[header::CACHE_CONTROL], "public, no-cache");
    let etag = initial.headers()[header::ETAG].clone();
    assert_eq!(initial.bytes().await?.as_ref(), b"tile");
    let hit = client.get(format!("{url}?cachebust=1")).send().await?;
    assert_eq!(hit.headers()[header::ETAG], etag);
    assert_eq!(hit.bytes().await?.as_ref(), b"tile");
    let conditional = client
        .get(format!("{url}?refresh=2"))
        .header(header::IF_NONE_MATCH, etag.clone())
        .send()
        .await?;
    assert_eq!(conditional.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(conditional.headers()[header::ETAG], etag);
    assert_eq!(
        conditional.headers()[header::CACHE_CONTROL],
        "public, no-cache"
    );
    assert!(conditional.bytes().await?.is_empty());
    let head = client.head(&url).send().await?;
    assert_eq!(head.headers()[header::CONTENT_LENGTH], "4");
    assert_eq!(head.headers()[header::ETAG], etag);
    assert!(head.bytes().await?.is_empty());
    let range = client
        .get(&url)
        .header(header::RANGE, "bytes=1-2")
        .send()
        .await?;
    assert_eq!(range.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(range.bytes().await?.as_ref(), b"il");
    let post = client.post(&url).send().await?;
    assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
    let replacement = directory.path().join("replacement");
    tokio::fs::write(&replacement, b"edit").await?;
    tokio::fs::rename(&replacement, &tile_path).await?;
    let updated = client
        .get(format!("{url}?refresh=3"))
        .header(header::IF_NONE_MATCH, etag.clone())
        .header(header::IF_MODIFIED_SINCE, "Wed, 01 Jan 2031 00:00:00 GMT")
        .send()
        .await?;
    assert_eq!(updated.status(), StatusCode::OK);
    assert_ne!(updated.headers()[header::ETAG], etag);
    assert_eq!(updated.bytes().await?.as_ref(), b"edit");
    let players_url = format!("{origin}/tiles/players.json");
    let players = client.get(&players_url).send().await?;
    assert_eq!(players.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(players.text().await?, "[]");
    tokio::fs::write(&players_path, b"[1]").await?;
    assert_eq!(client.get(&players_url).send().await?.text().await?, "[1]");
    tokio::fs::remove_file(tile_path).await?;
    let deleted = client
        .get(&url)
        .header(header::IF_NONE_MATCH, "*")
        .send()
        .await?;
    assert_eq!(deleted.status(), StatusCode::NOT_FOUND);
    assert_eq!(deleted.headers()[header::CACHE_CONTROL], "no-store");
    server.abort();
    Ok(())
}
