//! Opt-in end-to-end verification with a complete external index and synthetic speakers.
use crate::{
    error::{Error, Result},
    protocol,
};
use serde_json::{Value, json};
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::Command,
};

/// Run only with an explicit authenticated account; no Minecraft service is involved.
#[tokio::test]
#[ignore = "Requires explicit WIKI_TEST_BRIDGE, WIKI_TEST_CORPUS, WIKI_TEST_INDEX, WIKI_TEST_CODEX and WIKI_TEST_AUTH_FILE"]
async fn live_bridge_shares_world_context_and_isolates_other_worlds() -> Result<()> {
    let env = |name| std::env::var(name).map_err(|_| Error::Invalid(format!("{name} is required")));
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let work = root.path().join("work");
    std::fs::create_dir(&home)?;
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
    std::fs::create_dir(&work)?;
    symlink(env("WIKI_TEST_AUTH_FILE")?, home.join("auth.json"))?;
    let started = std::time::Instant::now();
    let mut child = Command::new(env("WIKI_TEST_BRIDGE")?)
        .args([
            "serve",
            "--index",
            &env("WIKI_TEST_INDEX")?,
            "--corpus",
            &env("WIKI_TEST_CORPUS")?,
            "--codex",
            &env("WIKI_TEST_CODEX")?,
            "--codex-home",
        ])
        .arg(&home)
        .arg("--work-dir")
        .arg(&work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().ok_or(Error::Codex)?;
    let mut output = BufReader::new(child.stdout.take().ok_or(Error::Codex)?);
    let ready: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(25), protocol::line(&mut output, 4096))
            .await
            .map_err(|_| Error::Timeout)??,
    )?;
    assert_eq!(ready["ready"], true);
    println!("{}", json!({"startup_ms":started.elapsed().as_millis()}));
    #[cfg(target_os = "linux")]
    if let Some(pid) = child.id() {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
        for line in status
            .lines()
            .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        {
            println!("Bridge process {line}");
        }
    }
    for (id, name, world, question) in [
        (
            1,
            "Alice",
            "00000000-0000-0000-0000-000000000001",
            "How do I craft a bucket? Our shared mine is called CopperKite.",
        ),
        (
            2,
            "Bob",
            "00000000-0000-0000-0000-000000000001",
            "What did Alice call our mine?",
        ),
        (
            3,
            "Bob",
            "00000000-0000-0000-0000-000000000002",
            "What did Alice call our mine?",
        ),
    ] {
        let request = json!({"id":id,"player_uuid":if name == "Alice" {"12345678-1234-1234-1234-123456789abc"} else {"12345678-1234-1234-1234-123456789def"},"username":name,
            "world_uuid":world,"world_name":format!("SyntheticWorld{world}"),"question":question});
        input.write_all(format!("{request}\n").as_bytes()).await?;
        input.flush().await?;
        let response: Value = tokio::time::timeout(Duration::from_secs(55), async {
            for _ in 0..33 {
                let frame: Value = serde_json::from_str(&protocol::line(&mut output, 4096).await?)?;
                assert_eq!(frame["id"], id);
                if frame.get("status").is_none() {
                    return Ok::<Value, Error>(frame);
                }
            }
            Err(Error::Codex)
        })
        .await
        .map_err(|_| Error::Timeout)??;
        assert_eq!(response["id"], id);
        let answer = response["answer"].as_str().ok_or(Error::Codex)?;
        assert!(answer.chars().count() <= 360 && answer.split_whitespace().count() <= 60);
        assert!(!answer.contains('\n'));
        match id {
            1 => assert!(answer.to_ascii_lowercase().contains("iron")),
            2 => assert!(answer.contains("CopperKite")),
            3 => assert!(!answer.contains("CopperKite")),
            _ => return Err(Error::Invalid("unknown test request".into())),
        }
        println!(
            "{}",
            json!({"request":id,"answer":answer,"tool_calls":response["tool_calls"],"retrieval_micros":response["retrieval_micros"],"elapsed_ms":response["elapsed_ms"]})
        );
    }
    child.kill().await?;
    Ok(())
}
