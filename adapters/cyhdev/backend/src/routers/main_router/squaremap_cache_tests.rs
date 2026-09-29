use std::{error::Error, sync::Arc};

use super::{ENTRY_OVERHEAD, TileCache};

/// Tiny budgets exercise the same reservation path without allocating gigabytes in tests.
async fn fixture(budget: usize) -> Result<(tempfile::TempDir, TileCache), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    tokio::fs::create_dir(directory.path().join("tiles")).await?;
    let cache = TileCache::with_budget(directory.path().to_owned(), budget);
    Ok((directory, cache))
}

async fn write_tile(directory: &tempfile::TempDir, key: &str) -> std::io::Result<()> {
    tokio::fs::write(directory.path().join(key), b"tile").await
}

#[test]
fn only_canonical_tile_paths_are_cached() {
    assert!(TileCache::eligible("tiles/minecraft_overworld/3/-1_0.png"));
    for path in [
        "tiles/players.json",
        "assets/image.png",
        "tiles/../secret.png",
        "tiles//a.png",
        "tiles/%2e%2e/secret.png",
        "/tiles/a.png",
        "tiles/a\\b.png",
    ] {
        assert!(!TileCache::eligible(path), "{path}");
    }
    assert!(!TileCache::eligible(&format!(
        "tiles/{}.png",
        "a".repeat(513)
    )));
}

#[tokio::test]
async fn unchanged_reads_share_allocation_and_rewrites_refresh() -> Result<(), Box<dyn Error>> {
    let (directory, cache) = fixture(16 * 1024).await?;
    write_tile(&directory, "tiles/a.png").await?;
    let original = cache.get("tiles/a.png").await?.ok_or("missing tile")?;
    let hit = cache.get("tiles/a.png").await?.ok_or("missing hit")?;
    assert!(Arc::ptr_eq(&original, &hit));
    let path = directory.path().join("tiles/a.png");
    let modified = tokio::fs::metadata(&path).await?.modified()?;
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    tokio::fs::write(&path, b"edit").await?;
    // Preserve size and mtime to prove that change time also participates in invalidation.
    let file = std::fs::OpenOptions::new().write(true).open(&path)?;
    file.set_times(std::fs::FileTimes::new().set_modified(modified))?;
    drop(file);
    let refreshed = cache.get("tiles/a.png").await?.ok_or("missing refresh")?;
    assert_ne!(original.version, refreshed.version);
    assert_eq!(refreshed.body().as_ref(), b"edit");
    tokio::fs::write(directory.path().join("replacement"), b"next").await?;
    tokio::fs::rename(directory.path().join("replacement"), &path).await?;
    assert_eq!(
        cache
            .get("tiles/a.png")
            .await?
            .ok_or("missing replacement")?
            .body()
            .as_ref(),
        b"next"
    );
    tokio::fs::remove_file(path).await?;
    assert_eq!(
        cache
            .get("tiles/a.png")
            .await
            .err()
            .ok_or("expected missing file")?
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(cache.entries.is_empty());
    Ok(())
}

#[tokio::test]
async fn evicted_response_buffers_keep_their_budget_reservations() -> Result<(), Box<dyn Error>> {
    let cost = 4 + ENTRY_OVERHEAD + "tiles/a.png".len() * 2;
    let (directory, cache) = fixture(cost * 2).await?;
    for key in ["tiles/a.png", "tiles/b.png", "tiles/c.png"] {
        write_tile(&directory, key).await?;
    }
    let first = cache.get("tiles/a.png").await?.ok_or("first")?.body();
    let second = cache.get("tiles/b.png").await?.ok_or("second")?.body();
    assert_eq!(cache.budget.available_permits(), 0);
    assert!(cache.get("tiles/c.png").await?.is_none());
    assert!(cache.entries.is_empty());
    assert_eq!(cache.budget.available_permits(), 0);
    let another_reader = first.clone();
    drop(first);
    assert_eq!(cache.budget.available_permits(), 0);
    drop(another_reader);
    assert_eq!(cache.budget.available_permits(), cost);
    let third = cache
        .get("tiles/c.png")
        .await?
        .ok_or("third after reclaim")?;
    drop(third);
    drop(second);
    cache.entries.clear_async().await;
    assert_eq!(cache.budget.available_permits(), cost * 2);
    Ok(())
}

#[tokio::test]
async fn pressure_evicts_entries_and_non_tiles_never_consume_budget() -> Result<(), Box<dyn Error>>
{
    let cost = 4 + ENTRY_OVERHEAD + "tiles/a.png".len() * 2;
    let (directory, cache) = fixture(cost).await?;
    write_tile(&directory, "tiles/a.png").await?;
    write_tile(&directory, "tiles/b.png").await?;
    drop(cache.get("tiles/a.png").await?.ok_or("first")?);
    drop(cache.get("tiles/b.png").await?.ok_or("second")?);
    assert_eq!(cache.entries.len(), 1);
    assert!(
        cache
            .entries
            .read_async("tiles/a.png", |_, _| ())
            .await
            .is_none()
    );
    assert!(cache.get("tiles/players.json").await?.is_none());
    assert_eq!(cache.budget.available_permits(), 0);
    Ok(())
}

#[tokio::test]
async fn concurrent_fills_remain_within_byte_budget() -> Result<(), Box<dyn Error>> {
    let budget = 8 * 1024;
    let (directory, cache) = fixture(budget).await?;
    let cache = Arc::new(cache);
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..64 {
        let key = format!("tiles/{index}.png");
        write_tile(&directory, &key).await?;
        let cache = Arc::clone(&cache);
        tasks.spawn(async move { cache.get(&key).await });
    }
    let mut tiles = Vec::new();
    while let Some(result) = tasks.join_next().await {
        if let Some(tile) = result?? {
            tiles.push(tile);
        }
    }
    assert!(tiles.len() <= budget / (4 + ENTRY_OVERHEAD));
    cache.entries.clear_async().await;
    drop(tiles);
    assert_eq!(cache.budget.available_permits(), budget);
    Ok(())
}

#[tokio::test]
async fn oversized_files_and_busy_fills_bypass_cache() -> Result<(), Box<dyn Error>> {
    let (directory, cache) = fixture(16 * 1024).await?;
    let path = directory.path().join("tiles/large.png");
    let file = tokio::fs::File::create(&path).await?;
    file.set_len(super::MAX_TILE_BYTES + 1).await?;
    assert!(cache.get("tiles/large.png").await?.is_none());
    write_tile(&directory, "tiles/a.png").await?;
    let _busy = cache.fills.acquire_many(16).await?;
    assert!(cache.get("tiles/a.png").await?.is_none());
    assert_eq!(cache.budget.available_permits(), 16 * 1024);
    Ok(())
}
