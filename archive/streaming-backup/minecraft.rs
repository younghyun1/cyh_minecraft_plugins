use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use anyhow::bail;
use tracing::{error, info};

use crate::config::settings::Config;

pub async fn backup_minecraft(config: &Config) -> anyhow::Result<PathBuf> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("minecraft_{}.tar.zst", timestamp);
    let output_path = config.backup_temp_dir.join(&filename);

    let mc_path = config.minecraft_server_path.clone();

    if !mc_path.exists() {
        error!(path = %mc_path.display(), "Minecraft server path does not exist");
        bail!(
            "Minecraft server path does not exist: {}",
            mc_path.display()
        );
    }

    info!(
        source = %mc_path.display(),
        output = %output_path.display(),
        "Starting Minecraft server backup (streaming tar+zstd)"
    );

    let out = output_path.clone();
    let mc = mc_path.clone();

    // tar and zstd crates are synchronous - run in a blocking thread
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
        let file = match File::create(&out) {
            Ok(f) => f,
            Err(e) => {
                error!(error = %e, path = %out.display(), "Failed to create output file");
                bail!("Failed to create output file {}: {}", out.display(), e);
            }
        };
        let writer = BufWriter::with_capacity(512 * 1024, file);

        let mut encoder = match zstd::Encoder::new(writer, 3) {
            Ok(enc) => enc,
            Err(e) => {
                error!(error = %e, "Failed to create zstd encoder");
                bail!("Failed to create zstd encoder: {}", e);
            }
        };

        if let Err(e) = encoder.multithread(0) {
            error!(error = %e, "Failed to enable zstd multithreading");
            bail!("Failed to enable zstd multithreading: {}", e);
        }

        let mut tar_builder = tar::Builder::new(encoder);
        // Don't follow symlinks - prevents chasing links outside the server directory
        // and avoids archiving unexpected/duplicate data
        tar_builder.follow_symlinks(false);

        if let Err(e) = tar_builder.append_dir_all("minecraft", &mc) {
            error!(
                error = %e,
                source_path = %mc.display(),
                "Failed to build tar archive"
            );
            bail!("Failed to build tar archive from {}: {}", mc.display(), e);
        }

        let encoder = match tar_builder.into_inner() {
            Ok(enc) => enc,
            Err(e) => {
                error!(error = %e, "Failed to finalize tar archive");
                bail!("Failed to finalize tar archive: {}", e);
            }
        };

        let writer = match encoder.finish() {
            Ok(w) => w,
            Err(e) => {
                error!(error = %e, "Failed to finalize zstd compression");
                bail!("Failed to finalize zstd compression: {}", e);
            }
        };

        let file = match writer.into_inner() {
            Ok(f) => f,
            Err(e) => {
                error!(error = %e, "Failed to flush output buffer");
                bail!("Failed to flush output buffer: {}", e);
            }
        };

        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "Failed to get output file metadata");
                bail!("Failed to get output file metadata: {}", e);
            }
        };

        Ok(metadata.len())
    })
    .await;

    let size_bytes = match result {
        Ok(Ok(size)) => size,
        Ok(Err(e)) => {
            error!(error = %e, output = %output_path.display(), "Minecraft backup failed");
            cleanup_temp_file(&output_path).await;
            return Err(e);
        }
        Err(e) => {
            cleanup_temp_file(&output_path).await;
            error!(error = %e, "Minecraft backup task panicked");
            bail!("Minecraft backup blocking task panicked: {}", e);
        }
    };

    info!(
        path = %output_path.display(),
        size_bytes = size_bytes,
        "Minecraft server backup completed"
    );

    Ok(output_path)
}

async fn cleanup_temp_file(path: &std::path::Path) {
    if let Err(e) = tokio::fs::remove_file(path).await {
        // File may not exist if creation itself failed - that's fine
        if e.kind() != std::io::ErrorKind::NotFound {
            error!(
                error = %e,
                path = %path.display(),
                "Failed to clean up partial temp file after backup failure"
            );
        }
    }
}
