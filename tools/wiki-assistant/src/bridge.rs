//! One in-flight request with bounded shared conversation state per Minecraft world.
use crate::{
    error::{Error, Result},
    memory::Pages,
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
        .write_all(b"{\"ready\":true,\"protocol\":1}\n")
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
        let started = Instant::now();
        if request.action != Action::Ask {
            let result = tokio::time::timeout(Duration::from_secs(35), async {
                match request.action {
                    Action::Clear => match sessions.clear(&request.world_uuid, &mut codex).await {
                        Ok(()) => Ok("This world's shared conversation has been cleared."),
                        Err(error) => Err(error),
                    },
                    Action::Compact => {
                        match sessions.compact(&request.world_uuid, &mut codex).await {
                            Ok(true) => Ok("This world's shared conversation has been compacted."),
                            Ok(false) => Ok("This world has no conversation to compact."),
                            Err(error) => Err(error),
                        }
                    }
                    Action::Ask => Err(Error::Codex),
                }
            })
            .await;
            let answer = match result {
                Ok(Ok(answer)) => answer,
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(Error::Timeout),
            };
            let response = json!({"id":request.id,"answer":answer,"sources":[]});
            output.write_all(format!("{response}\n").as_bytes()).await?;
            output.flush().await?;
            continue;
        }
        if !sessions.can_admit(&request.world_uuid) {
            let response = json!({"id":request.id,"error":"Minecraft help has reached its world conversation limit."});
            output.write_all(format!("{response}\n").as_bytes()).await?;
            output.flush().await?;
            continue;
        }
        let session = tokio::time::timeout(
            Duration::from_secs(5),
            sessions.acquire(&request.world_uuid, &mut codex),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        // Include the previous question only for short follow-ups; retrieval remains exactly one lookup.
        let query = if request.question.split_whitespace().count() <= 5 {
            format!("{} {}", request.question, session.previous_question)
        } else {
            request.question.clone()
        };
        let results = search.search(&query)?;
        let prompt = serde_json::to_string(
            &json!({"world":{"uuid":request.world_uuid,"name":request.world_name},"sender":{"uuid":request.player_uuid,"username":request.username},"question":request.question,"wiki_passages":results.hits}),
        )?;
        let mut lookup = Lookup {
            search: &search,
            pages: &pages,
            calls: 0,
            sources: Vec::new(),
        };
        let answer = match tokio::time::timeout(
            Duration::from_secs(15),
            crate::turn::run(&mut codex, &session.thread, prompt, Some(&mut lookup)),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout),
        };
        let failed = answer.is_err();
        session.previous_question = request.question;
        let response = match answer {
            Ok(mut answer) => {
                for hit in &results.hits {
                    if !answer.sources.contains(&hit.url) {
                        answer.sources.push(hit.url.clone());
                    }
                }
                answer.sources.truncate(8);
                json!({"id":request.id,"answer":answer.text,"sources":answer.sources,"tool_calls":answer.tool_calls,
                "retrieval_micros":results.retrieval_micros,"elapsed_ms":started.elapsed().as_millis()})
            }
            Err(error) => {
                tracing::warn!(error = %error, "Chat request failed");
                json!({"id":request.id,"error":"Minecraft help is temporarily unavailable. Try again shortly."})
            }
        };
        output
            .write_all(format!("{}\n", serde_json::to_string(&response)?).as_bytes())
            .await?;
        output.flush().await?;
        if failed {
            codex.stop().await?;
            return Err(Error::Codex);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
