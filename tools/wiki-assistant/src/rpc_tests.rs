//! Synthetic transport tests never use a credential, model service, or Minecraft server.
use super::*;
use crate::sessions::Sessions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Simulate an app-server on a bounded duplex pipe.
fn pair() -> (Codex, tokio::io::DuplexStream) {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let (read, write) = tokio::io::split(client);
    (
        Codex {
            child: None,
            input: Box::new(write),
            output: Box::new(BufReader::new(read)),
            next_id: 1,
        },
        server,
    )
}

#[tokio::test]
async fn worlds_share_speakers_but_isolate_context_and_only_return_final_answer() -> Result<()> {
    let (mut client, server) = pair();
    let remote = tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server);
        let mut read = BufReader::new(read);
        let mut threads = 0;
        for _ in 0..5 {
            let mut line = String::new();
            read.read_line(&mut line).await?;
            let request: Value = serde_json::from_str(&line)?;
            let id = request["id"].clone();
            let response = if request["method"] == "thread/start" {
                assert_eq!(request["params"]["model"], "gpt-6-luna");
                assert_eq!(request["params"]["serviceTier"], "fast");
                assert_eq!(request["params"]["config"]["model_reasoning_effort"], "low");
                threads += 1;
                json!({"id":id,"result":{"model":"gpt-6-luna","thread":{"id":format!("thread-{threads}")}}})
            } else {
                assert_eq!(request["method"], "turn/start");
                assert_eq!(request["params"]["effort"], "low");
                assert_eq!(request["params"]["serviceTier"], "fast");
                let thread = request["params"]["threadId"].clone();
                for value in [
                    json!({"id":id,"result":{"turn":{"id":"turn-1"}}}),
                    json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"reasoning","text":"SECRET REASONING"}}}),
                    json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"agentMessage","phase":"commentary","text":"SECRET COMMENTARY"}}}),
                    json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"agentMessage","phase":"final_answer","text":"Use three iron ingots."}}}),
                ] {
                    write.write_all(format!("{value}\n").as_bytes()).await?;
                }
                json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"status":"completed"}}})
            };
            write.write_all(format!("{response}\n").as_bytes()).await?;
        }
        Ok::<(), Error>(())
    });
    let mut sessions = Sessions::default();
    let first = sessions
        .acquire("world-a", &mut client)
        .await?
        .thread
        .clone();
    assert_eq!(
        client
            .answer(
                &first,
                json!({"sender":{"username":"Alice"},"question":"question 1"}).to_string()
            )
            .await?,
        "Use three iron ingots."
    );
    let again = sessions
        .acquire("world-a", &mut client)
        .await?
        .thread
        .clone();
    assert_eq!(first, again);
    assert_eq!(
        client
            .answer(
                &again,
                json!({"sender":{"username":"Bob"},"question":"follow up"}).to_string()
            )
            .await?,
        "Use three iron ingots."
    );
    let other = sessions
        .acquire("world-b", &mut client)
        .await?
        .thread
        .clone();
    assert_ne!(first, other);
    assert_eq!(
        client.answer(&other, "other question".into()).await?,
        "Use three iron ingots."
    );
    remote.await.map_err(|_| Error::Codex)??;
    Ok(())
}

#[tokio::test]
async fn rejects_approval_requests_and_failed_turns() -> Result<()> {
    let (mut client, mut remote) = pair();
    remote
        .write_all(
            b"{\"id\":8,\"method\":\"item/commandExecution/requestApproval\",\"params\":{}}\n",
        )
        .await?;
    assert!(client.read().await.is_err());
    let line = protocol::line(&mut BufReader::new(remote), 4096).await?;
    let response: Value = serde_json::from_str(&line)?;
    assert_eq!(response["error"]["code"], -32601);
    Ok(())
}

#[tokio::test]
#[ignore = "Explicit Codex integration check; uses the local authenticated account"]
async fn live_cli_keeps_prior_messages() -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let binary = std::env::var("WIKI_TEST_CODEX")
        .map_err(|_| Error::Invalid("WIKI_TEST_CODEX is required".into()))?;
    let auth = std::env::var("WIKI_TEST_AUTH_FILE")
        .map_err(|_| Error::Invalid("WIKI_TEST_AUTH_FILE is required".into()))?;
    let directory = tempfile::tempdir()?;
    let home = directory.path().join("home");
    let work = directory.path().join("work");
    std::fs::create_dir(&home)?;
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
    std::fs::create_dir(&work)?;
    symlink(auth, home.join("auth.json"))?;
    tokio::time::timeout(std::time::Duration::from_secs(50), async {
        let mut client = Codex::start(Path::new(&binary), &home, &work).await?;
        let thread = client.start_thread().await?;
        let first = client.answer(&thread, json!({"world":{"uuid":"00000000-0000-0000-0000-000000000001","name":"SyntheticWorld"},"sender":{"uuid":"12345678-1234-1234-1234-123456789abc","username":"Alice"},"question":"My fictional base is called CopperKite. Remember that name for our next question.","wiki_passages":[]}).to_string()).await?;
        assert!(first.chars().count() <= 360);
        let second = client.answer(&thread, json!({"world":{"uuid":"00000000-0000-0000-0000-000000000001","name":"SyntheticWorld"},"sender":{"uuid":"12345678-1234-1234-1234-123456789def","username":"Bob"},"question":"What did Alice call her fictional base?","wiki_passages":[]}).to_string()).await?;
        assert!(second.contains("CopperKite"));
        client.unload(&thread).await?;
        client.stop().await?;
        Ok::<(), Error>(())
    }).await.map_err(|_| Error::Timeout)?
}
