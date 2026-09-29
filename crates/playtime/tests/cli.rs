//! End-to-end coverage through the actual CLI and compressed input pipeline.
use flate2::{Compression, write::GzEncoder};
use serde_json::Value;
use std::{fs, io::Write, path::PathBuf, process::Command};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Fixture(PathBuf);
impl Fixture {
    /// Isolate test files by test name and process, without shared mutable state.
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let path = std::env::temp_dir().join(format!("parse-logs-{name}-{}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    /// Write authentic archives to exercise the decoder, not a mocked reader.
    fn gzip(&self, name: &str, text: &str) -> TestResult {
        let mut encoder = GzEncoder::new(fs::File::create(self.0.join(name))?, Compression::fast());
        encoder.write_all(text.as_bytes())?;
        encoder.finish()?;
        Ok(())
    }
}
impl Drop for Fixture {
    /// Best-effort cleanup must not hide a test failure.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Files sort numerically, repeated paths are deduplicated, and midnight sessions survive.
#[test]
fn rotations_and_gzip() -> TestResult {
    let fixture = Fixture::new("rotations")?;
    fixture.gzip(
        "2026-09-27-2.log.gz",
        "[23:58:00 INFO]: Alex joined the game\n",
    )?;
    fixture.gzip("2026-09-27-10.log.gz", "[23:59:00 INFO]: <Alex> hello\n")?;
    fs::write(
        fixture.0.join("latest.log"),
        "[00:02:00] [Server thread/INFO]: System chat: Alex left the game\n",
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_parse_logs"))
        .arg(&fixture.0)
        .arg(fixture.0.join("2026-09-27-2.log.gz"))
        .args(["--latest-date", "2026-09-28"])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["files"], 3);
    assert_eq!(report["players"]["Alex"]["playtime_seconds"], 240);
    assert_eq!(report["out_of_order_lines"], 0);
    Ok(())
}

/// Clock reordering is visible, and a later join cannot bridge a lost disconnect.
#[test]
fn missing_disconnect_and_reordering() -> TestResult {
    let fixture = Fixture::new("missing")?;
    fs::write(
        fixture.0.join("2026-09-27-1.log"),
        concat!(
            "[12:00:00 INFO]: Alex joined the game\n",
            "[11:59:59 INFO]: Alex left the game\n",
            "[13:00:00 INFO]: Alex joined the game\n",
            "[13:05:00 INFO]: Alex left the game\n",
        ),
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_parse_logs"))
        .arg(&fixture.0)
        .output()?;
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["out_of_order_lines"], 1);
    assert_eq!(report["players"]["Alex"]["playtime_seconds"], 300);
    assert_eq!(report["players"]["Alex"]["incomplete_sessions"], 1);
    Ok(())
}

/// Corrupt compressed data fails rather than publishing a partial success report.
#[test]
fn corrupt_archive() -> TestResult {
    let fixture = Fixture::new("corrupt")?;
    fs::write(fixture.0.join("2026-09-27-1.log.gz"), b"not gzip")?;
    let output = Command::new(env!("CARGO_BIN_EXE_parse_logs"))
        .arg(&fixture.0)
        .output()?;
    assert!(!output.status.success());
    let diagnostic: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(diagnostic["level"], "ERROR");
    assert!(diagnostic.get("players").is_none());
    Ok(())
}
