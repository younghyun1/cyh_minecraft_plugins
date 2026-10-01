//! Synthetic lifecycle checks for world-local controls, without an authenticated account.
use super::*;
use crate::sessions::Sessions;

#[tokio::test]
async fn clear_replaces_only_one_world_and_compact_preserves_followup() -> Result<()> {
    let (mut client, server) = super::tests::pair();
    let remote = tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server);
        let mut read = BufReader::new(read);
        for (method, thread) in [
            ("thread/start", "one"),
            ("thread/start", "two"),
            ("thread/compact/start", "one"),
            ("thread/unsubscribe", "one"),
            ("thread/start", "three"),
        ] {
            let request: Value = serde_json::from_str(&protocol::line(&mut read, 8192).await?)?;
            assert_eq!(request["method"], method);
            if method != "thread/start" {
                assert_eq!(request["params"]["threadId"], thread);
            }
            let result = if method == "thread/start" {
                json!({"model":"gpt-6-luna","thread":{"id":thread}})
            } else {
                json!({})
            };
            if method == "thread/compact/start" {
                // Notifications can arrive before the acknowledgement; other worlds do not count.
                for event in [
                    json!({"method":"turn/completed","params":{"threadId":"two","turn":{"status":"failed"}}}),
                    json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"agentMessage","phase":"commentary","text":"PRIVATE"}}}),
                    json!({"method":"item/completed","params":{"threadId":thread,"item":{"type":"contextCompaction"}}}),
                    json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"status":"completed"}}}),
                ] {
                    write.write_all(format!("{event}\n").as_bytes()).await?;
                }
            }
            let response = json!({"id":request["id"],"result":result});
            write.write_all(format!("{response}\n").as_bytes()).await?;
        }
        Ok::<(), Error>(())
    });
    let mut sessions = Sessions::default();
    sessions.acquire("a", &mut client).await?.previous_question = "prior question".into();
    assert_eq!(sessions.acquire("b", &mut client).await?.thread, "two");
    assert!(!sessions.compact("unused", &mut client).await?);
    sessions.clear("unused", &mut client).await?;
    assert!(sessions.compact("a", &mut client).await?);
    let session = sessions.acquire("a", &mut client).await?;
    assert_eq!(session.thread, "one");
    assert_eq!(session.previous_question, "prior question");
    sessions.clear("a", &mut client).await?;
    sessions.clear("a", &mut client).await?;
    assert_eq!(sessions.acquire("b", &mut client).await?.thread, "two");
    let fresh = sessions.acquire("a", &mut client).await?;
    assert_eq!(fresh.thread, "three");
    assert!(fresh.previous_question.is_empty());
    match remote.await {
        Ok(result) => result,
        Err(_) => Err(Error::Codex),
    }
}

#[tokio::test]
async fn compaction_rejects_failed_or_incomplete_completion() -> Result<()> {
    for status in ["failed", "interrupted", "completed"] {
        let (mut client, server) = super::tests::pair();
        let remote = tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(server);
            let mut read = BufReader::new(read);
            let request: Value = serde_json::from_str(&protocol::line(&mut read, 4096).await?)?;
            for event in [
                json!({"id":request["id"],"result":{}}),
                json!({"method":"turn/completed","params":{"threadId":"one","turn":{"status":status}}}),
            ] {
                write.write_all(format!("{event}\n").as_bytes()).await?;
            }
            Ok::<(), Error>(())
        });
        assert!(client.compact("one").await.is_err());
        match remote.await {
            Ok(result) => result?,
            Err(_) => return Err(Error::Codex),
        }
    }
    Ok(())
}
