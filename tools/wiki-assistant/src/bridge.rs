//! One in-flight request with bounded shared conversation state per Minecraft world.
use crate::{
    error::{Error, Result},
    protocol,
    retrieval::Search,
    rpc::Codex,
    sessions::{self, Sessions},
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
}

/// Serve line-delimited requests from the Paper child-process pipe only.
pub async fn serve(search: Search, binary: &Path, home: &Path, cwd: &Path) -> Result<()> {
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    let mut codex = tokio::time::timeout(Duration::from_secs(20), Codex::start(binary, home, cwd))
        .await
        .map_err(|_| Error::Timeout)??;
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
            || request.question.trim().is_empty()
            || request.question.chars().count() > 240
            || request.question.chars().any(char::is_control)
        {
            return Err(Error::Invalid(
                "question must contain 1-240 printable characters".into(),
            ));
        }
        let started = Instant::now();
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
        let answer = match tokio::time::timeout(
            Duration::from_secs(15),
            codex.answer(&session.thread, prompt),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout),
        };
        let failed = answer.is_err();
        session.previous_question = request.question;
        let response = match answer {
            Ok(text) => {
                json!({"id":request.id,"answer":text,"sources":results.hits.iter().map(|hit| &hit.url).take(2).collect::<Vec<_>>(),
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
