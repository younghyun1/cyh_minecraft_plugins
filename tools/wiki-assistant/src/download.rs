//! Serial MediaWiki downloads, respecting maxlag and resumable continuation tokens.
use crate::{
    corpus::{self, API, MAX_BATCH, Manifest, Page},
    error::{Error, Result},
};
use reqwest::blocking::Client;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::Path,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Read a bounded response and retry only throttling, server overload, or maxlag.
fn request(client: &Client, query: &[(String, String)]) -> Result<Value> {
    for attempt in 0..5 {
        let response = client.get(API).query(query).send()?;
        let status = response.status();
        if status.as_u16() == 429 || status.is_server_error() {
            let delay = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5 << attempt)
                .clamp(1, 120);
            thread::sleep(Duration::from_secs(delay));
            continue;
        }
        let mut bytes = Vec::new();
        response
            .error_for_status()?
            .take(MAX_BATCH + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BATCH {
            return Err(Error::Invalid("wiki response exceeds 32 MiB".into()));
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        if value.pointer("/error/code").and_then(Value::as_str) == Some("maxlag") {
            thread::sleep(Duration::from_secs(5 << attempt));
            continue;
        }
        if value.get("error").is_some() || value.get("warnings").is_some() {
            return Err(Error::Invalid(
                "wiki returned an error or warning; snapshot paused".into(),
            ));
        }
        return Ok(value);
    }
    Err(Error::Invalid(
        "wiki throttled five requests; rerun to resume".into(),
    ))
}

/// Require actual revision content; a partial API page must never become a complete snapshot.
fn page(value: &Value) -> Result<Page> {
    let revision = value
        .pointer("/revisions/0")
        .ok_or_else(|| Error::Invalid("missing revision".into()))?;
    let string = |v: &Value, key: &str| -> Result<String> {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| Error::Invalid(format!("missing {key}")))
    };
    Ok(Page {
        page_id: value["pageid"]
            .as_u64()
            .ok_or_else(|| Error::Invalid("missing page ID".into()))?,
        namespace: value["ns"]
            .as_i64()
            .ok_or_else(|| Error::Invalid("missing namespace".into()))?,
        title: string(value, "title")?,
        revision_id: revision["revid"]
            .as_u64()
            .ok_or_else(|| Error::Invalid("missing revision ID".into()))?,
        timestamp: string(revision, "timestamp")?,
        text: string(&revision["slots"]["main"], "content")?,
    })
}

/// Download every current documentation page, including redirects and transclusion sources.
pub fn download(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".download.lock"))?;
    lock.try_lock()
        .map_err(|_| Error::Invalid("another download owns this snapshot".into()))?;
    let client = Client::builder()
        .user_agent(
            "MinecraftWikiAssistant/0.1 (https://github.com/younghyun1/cyh_minecraft_plugins)",
        )
        .timeout(Duration::from_secs(60))
        .build()?;
    let base = || {
        vec![
            ("format".into(), "json".into()),
            ("formatversion".into(), "2".into()),
            ("action".into(), "query".into()),
            ("maxlag".into(), "5".into()),
        ]
    };
    let mut manifest = if root.join("manifest.json").exists() {
        corpus::load_manifest(root, false)?
    } else {
        let mut query = base();
        query.extend([
            ("meta".into(), "siteinfo".into()),
            ("siprop".into(), "rightsinfo|namespaces".into()),
        ]);
        let info = request(&client, &query)?;
        let namespaces = info
            .pointer("/query/namespaces")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::Invalid("missing wiki namespaces".into()))?
            .values()
            .filter(|v| {
                v["content"].as_bool() == Some(true)
                    || [4, 10, 12, 14, 828].contains(&v["id"].as_i64().unwrap_or(-1))
            })
            .filter_map(|v| v["id"].as_i64())
            .collect();
        let license = info["query"]["rightsinfo"].clone();
        if license["url"].as_str().is_none() {
            return Err(Error::Invalid("missing wiki license".into()));
        }
        Manifest {
            format: 1,
            source: API.into(),
            license,
            namespaces,
            namespace_cursor: 0,
            continuation: None,
            batches: 0,
            pages: 0,
            compressed_bytes: 0,
            started_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Invalid("clock before epoch".into()))?
                .as_secs(),
            complete: false,
        }
    };
    while manifest.namespace_cursor < manifest.namespaces.len() {
        let mut query = base();
        query.extend([
            ("generator".into(), "allpages".into()),
            (
                "gapnamespace".into(),
                manifest.namespaces[manifest.namespace_cursor].to_string(),
            ),
            ("gaplimit".into(), "50".into()),
            ("prop".into(), "revisions".into()),
            ("rvprop".into(), "ids|timestamp|content".into()),
            ("rvslots".into(), "main".into()),
        ]);
        if let Some(continuation) = &manifest.continuation {
            let fields = continuation
                .as_object()
                .ok_or_else(|| Error::Invalid("bad continuation".into()))?;
            for (key, value) in fields {
                let value = value
                    .as_str()
                    .ok_or_else(|| Error::Invalid("bad continuation value".into()))?;
                query.push((key.clone(), value.into()));
            }
        }
        let response = request(&client, &query)?;
        let pages = match response.pointer("/query/pages").and_then(Value::as_array) {
            Some(values) => values.iter().map(page).collect::<Result<Vec<_>>>()?,
            None if response.get("batchcomplete").is_some() => Vec::new(),
            None => return Err(Error::Invalid("missing page batch".into())),
        };
        let encoded = serde_json::to_vec(&pages)?;
        if encoded.len() as u64 > MAX_BATCH {
            return Err(Error::Invalid("serialized batch exceeds bound".into()));
        }
        let compressed = zstd::encode_all(encoded.as_slice(), 6)?;
        manifest.compressed_bytes += compressed.len() as u64;
        if manifest.compressed_bytes > 8 * 1024 * 1024 * 1024 {
            return Err(Error::Invalid("snapshot exceeds 8 GiB".into()));
        }
        let path = root.join(format!("{:06}.json.zst", manifest.batches));
        let temp = path.with_extension("tmp");
        fs::write(&temp, compressed)?;
        fs::rename(temp, path)?;
        manifest.pages += pages.len() as u64;
        manifest.batches += 1;
        let next = response.get("continue").cloned();
        if next.is_some() && next == manifest.continuation {
            return Err(Error::Invalid("wiki repeated continuation".into()));
        }
        manifest.continuation = next;
        if manifest.continuation.is_none() {
            manifest.namespace_cursor += 1;
        }
        manifest.complete = manifest.namespace_cursor == manifest.namespaces.len();
        corpus::save_manifest(root, &manifest)?;
        if manifest.batches % 20 == 0 || manifest.complete {
            tracing::info!(
                pages = manifest.pages,
                batches = manifest.batches,
                complete = manifest.complete,
                "Wiki checkpoint saved"
            );
        }
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}
