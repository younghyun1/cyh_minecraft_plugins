//! Dispatch only wiki tools during inference; hidden events never become chat output.
use crate::{
    error::{Error, Result},
    protocol,
    rpc::Codex,
    wiki_tools::Lookup,
};
use serde_json::{Value, json};

pub struct Answer {
    pub text: String,
    pub sources: Vec<String>,
    pub tool_calls: usize,
}

pub async fn run(
    codex: &mut Codex,
    thread: &str,
    prompt: String,
    mut lookup: Option<&mut Lookup<'_>>,
) -> Result<Answer> {
    let started = codex
        .request(
            "turn/start",
            json!({"threadId":thread,"model":"gpt-6-luna","effort":"low","serviceTier":"fast",
        "input":[{"type":"text","text":prompt,"text_elements":[]}]}),
        )
        .await?;
    let turn_id = started
        .pointer("/turn/id")
        .and_then(Value::as_str)
        .ok_or(Error::Codex)?;
    let mut answer = String::new();
    let mut complete = false;
    let mut requests = 0;
    for _ in 0..1024 {
        let event = codex.read().await?;
        if event["method"] == "item/tool/call" {
            requests += 1;
            let params = &event["params"];
            if requests > 6
                || params["threadId"].as_str() != Some(thread)
                || params["turnId"].as_str() != Some(turn_id)
            {
                return Err(Error::Codex);
            }
            let result = match lookup.as_deref_mut() {
                Some(local) => local.call(
                    params["tool"].as_str().ok_or(Error::Codex)?,
                    params["arguments"].clone(),
                ),
                None => Err(Error::Invalid("local lookup unavailable".into())),
            };
            let (success, content) = match result {
                Ok(value) => (true, serde_json::to_string(&value)?),
                Err(error) => (
                    false,
                    serde_json::to_string(&json!({"error":error.to_string()}))?,
                ),
            };
            codex.write(json!({"id":event["id"],"result":{"success":success,"contentItems":[{"type":"inputText","text":content}]}})).await?;
            continue;
        }
        if event["params"]["threadId"].as_str() != Some(thread) {
            continue;
        }
        match event["method"].as_str() {
            Some("item/completed") => {
                let item = &event["params"]["item"];
                if item["type"] == "agentMessage" && item["phase"].as_str() == Some("final_answer")
                {
                    answer = item["text"].as_str().ok_or(Error::Codex)?.to_owned();
                    if answer.len() > 16 * 1024 {
                        return Err(Error::Codex);
                    }
                }
            }
            Some("turn/completed") => {
                if event["params"]["turn"]["status"] != "completed" {
                    return Err(Error::Codex);
                }
                complete = true;
                break;
            }
            _ => {}
        }
    }
    if !complete || answer.trim().is_empty() {
        return Err(Error::Codex);
    }
    let (sources, tool_calls) = match lookup {
        Some(local) => (std::mem::take(&mut local.sources), local.calls),
        None => (Vec::new(), 0),
    };
    Ok(Answer {
        text: protocol::answer(&answer),
        sources,
        tool_calls,
    })
}
