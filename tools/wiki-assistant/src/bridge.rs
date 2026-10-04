//! One in-flight request with bounded shared conversation state per Minecraft world.
use crate::{
    error::{Error, Result},
    memory::Pages,
    progress::Phase,
    protocol,
    retrieval::Search,
    rpc::Codex,
    sessions::{self, Sessions},
    wiki_tools::Lookup,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::io::{AsyncWriteExt, BufReader};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    player_uuid: String,
    username: String,
    world_uuid: String,
    world_name: String,
    question: String,
    #[serde(default)]
    action: Action,
}

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Action {
    #[default]
    Ask,
    Clear,
    Compact,
}

/// Serve line-delimited requests from the Paper child-process pipe only.
pub async fn serve(
    search: Search,
    pages: Pages,
    binary: &Path,
    home: &Path,
    cwd: &Path,
) -> Result<()> {
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    let started = tokio::time::timeout(Duration::from_secs(20), Codex::start(binary, home, cwd))
        .await
        .map_err(|_| Error::Timeout)
        .and_then(|result| result);
    let mut codex = match started {
        Ok(codex) => codex,
        Err(error) => {
            protocol::startup_failure(&error)?;
            return Err(error);
        }
    };
    output
        .write_all(b"{\"ready\":true,\"protocol\":2}\n")
        .await?;
    output.flush().await?;
    let mut sessions = Sessions::default();
    loop {
        let line = protocol::line(&mut input, 4096).await?;
        let request: Request = serde_json::from_str(&line)?;
        if !sessions::valid_sender(&request.player_uuid, &request.username)
            || !sessions::valid_world(&request.world_uuid, &request.world_name)
            || (request.action == Action::Ask && request.question.trim().is_empty())
            || request.question.chars().count() > 240
            || request.question.chars().any(char::is_control)
        {
            return Err(Error::Invalid(
                "question must contain 1-240 printable characters".into(),
            ));
        }
        codex.progress.begin(request.id);
        let response = handle(&request, &search, &pages, &mut codex, &mut sessions).await;
        let (response, failure) = match response {
            Ok(response) => (response, None),
            Err(error) => {
                codex.progress.emit(Phase::Resetting).await?;
                // Reap before the terminal frame so another command cannot race failed-session cleanup.
                if let Err(stop_error) = codex.stop().await {
                    tracing::warn!(code = stop_error.chat_code(), "Codex cleanup failed");
                }
                (
                    json!({"id":request.id,"error_code":error.chat_code(),"fatal":true}),
                    Some(error),
                )
            }
        };
        output.write_all(format!("{response}\n").as_bytes()).await?;
        output.flush().await?;
        if let Some(error) = failure {
            return Err(error);
        }
    }
}

/// Keep gameplay and control failures on the same typed response path.
async fn handle(
    request: &Request,
    search: &Search,
    pages: &Pages,
    codex: &mut Codex,
    sessions: &mut Sessions,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    if request.action != Action::Ask {
        codex
            .progress
            .emit(if request.action == Action::Clear {
                Phase::Clearing
            } else {
                Phase::Compacting
            })
            .await?;
        let result = tokio::time::timeout(Duration::from_secs(35), async {
            match request.action {
                Action::Clear => match sessions.clear(&request.world_uuid, codex).await {
                    Ok(()) => Ok("This world's shared conversation has been cleared."),
                    Err(error) => Err(error),
                },
                Action::Compact => match sessions.compact(&request.world_uuid, codex).await {
                    Ok(true) => Ok("This world's shared conversation has been compacted."),
                    Ok(false) => Ok("This world has no conversation to compact."),
                    Err(error) => Err(error),
                },
                Action::Ask => Err(Error::Codex),
            }
        })
        .await;
        let answer = match result {
            Ok(Ok(answer)) => answer,
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err(Error::Timeout),
        };
        return Ok(json!({"id":request.id,"answer":answer,"sources":[]}));
    }
    if !sessions.can_admit(&request.world_uuid) {
        return Ok(json!({"id":request.id,"error_code":"capacity","fatal":false}));
    }
    codex.progress.emit(Phase::Connecting).await?;
    let session = tokio::time::timeout(
        Duration::from_secs(5),
        sessions.acquire(&request.world_uuid, codex),
    )
    .await
    .map_err(|_| Error::Timeout)??;
    // Include the previous question only for short follow-ups; retrieval remains exactly one lookup.
    let query = if request.question.split_whitespace().count() <= 5 {
        format!("{} {}", request.question, session.previous_question)
    } else {
        request.question.clone()
    };
    codex.progress.emit(Phase::Searching).await?;
    let results = search.search(&query)?;
    let prompt = serde_json::to_string(
        &json!({"world":{"uuid":request.world_uuid,"name":request.world_name},"sender":{"uuid":request.player_uuid,"username":request.username},"question":request.question,"wiki_passages":results.hits}),
    )?;
    let mut lookup = Lookup {
        search,
        pages,
        calls: 0,
        sources: Vec::new(),
    };
    let answer = match tokio::time::timeout(
        Duration::from_secs(45),
        crate::turn::run(codex, &session.thread, prompt, Some(&mut lookup)),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(Error::Timeout),
    };
    match answer {
        Ok(mut answer) => {
            if answer.relevant {
                session.previous_question.clone_from(&request.question);
                for hit in &results.hits {
                    if !answer.sources.contains(&hit.url) {
                        answer.sources.push(hit.url.clone());
                    }
                }
            }
            answer.sources.truncate(8);
            Ok(
                json!({"id":request.id,"answer":answer.text,"sources":answer.sources,"tool_calls":answer.tool_calls,
                "retrieval_micros":results.retrieval_micros,"elapsed_ms":started.elapsed().as_millis()}),
            )
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejected_turn_has_no_sources_and_preserves_relevant_retrieval_context() -> Result<()> {
        let (search, pages) = crate::fixtures::resident()?;
        let (mut codex, remote) = crate::rpc::test_pair();
        let server = tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(remote);
            let mut input = BufReader::new(read);
            let start: serde_json::Value =
                serde_json::from_str(&protocol::line(&mut input, 16384).await?)?;
            write.write_all(format!("{}\n", json!({"id":start["id"],"result":{"model":"gpt-6.1-sol","thread":{"id":"one"}}})).as_bytes()).await?;
            let turn: serde_json::Value =
                serde_json::from_str(&protocol::line(&mut input, 16384).await?)?;
            for event in [
                json!({"id":turn["id"],"result":{"turn":{"id":"turn-1"}}}),
                json!({"method":"item/completed","params":{"threadId":"one","item":{"type":"agentMessage","phase":"final_answer","text":json!({"relevance":"irrelevant","answer":"DO NOT FORWARD"}).to_string()}}}),
                json!({"method":"turn/completed","params":{"threadId":"one","turn":{"status":"completed"}}}),
            ] {
                write.write_all(format!("{event}\n").as_bytes()).await?;
            }
            Ok::<(), Error>(())
        });
        let mut sessions = Sessions::default();
        sessions
            .acquire("world", &mut codex)
            .await?
            .previous_question = "Bucket recipe".into();
        let request = Request {
            id: 1,
            player_uuid: "player".into(),
            username: "Alice".into(),
            world_uuid: "world".into(),
            world_name: "Survival".into(),
            question: "Bucket investment advice".into(),
            action: Action::Ask,
        };
        let response = handle(&request, &search, &pages, &mut codex, &mut sessions).await?;
        assert_eq!(response["answer"], crate::response::IRRELEVANT);
        assert_eq!(response["sources"], json!([]));
        assert_eq!(
            sessions
                .acquire("world", &mut codex)
                .await?
                .previous_question,
            "Bucket recipe"
        );
        match server.await {
            Ok(result) => result,
            Err(_) => Err(Error::Codex),
        }
    }

    #[test]
    fn legacy_questions_and_explicit_controls_decode_without_prompt_dispatch() -> Result<()> {
        let mut frame = json!({"id":1,"player_uuid":"player","username":"Alice","world_uuid":"world","world_name":"Survival","question":"clear"});
        let request: Request = serde_json::from_value(frame.clone())?;
        assert!(request.action == Action::Ask);
        for action in ["clear", "compact"] {
            frame["action"] = json!(action);
            frame["question"] = json!("");
            let request: Request = serde_json::from_value(frame.clone())?;
            assert!(request.action != Action::Ask);
        }
        frame["action"] = json!("shell");
        assert!(serde_json::from_value::<Request>(frame).is_err());
        Ok(())
    }
}
