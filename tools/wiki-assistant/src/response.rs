//! Structured relevance decisions keep rejected content out of public chat.
use crate::{
    error::{Error, Result},
    protocol,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const IRRELEVANT: &str = "Question irrelevant to purpose";

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Relevance {
    Minecraft,
    Irrelevant,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    relevance: Relevance,
    answer: String,
}

/// The scope decision and answer share one inference; no additional classifier round trip.
pub fn schema() -> Value {
    json!({"type":"object","properties":{
        "relevance":{"type":"string","enum":["minecraft","irrelevant"]},
        "answer":{"type":"string"}},"required":["relevance","answer"],"additionalProperties":false})
}

/// Never forward raw JSON, an explanation of rejection, or an unclassified reply.
pub fn decode(raw: &str) -> Result<(String, bool)> {
    let reply: Reply = match serde_json::from_str(raw) {
        Ok(reply) => reply,
        Err(_) => return Err(Error::Codex),
    };
    match reply.relevance {
        Relevance::Irrelevant => Ok((IRRELEVANT.into(), false)),
        Relevance::Minecraft => {
            let text = protocol::answer(&reply.answer);
            if text.is_empty() {
                return Err(Error::Codex);
            }
            Ok((text, true))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejection_discards_content_and_invalid_classification_fails_closed() -> Result<()> {
        assert_eq!(
            decode(
                r#"{"relevance":"irrelevant","answer":"Unrelated answer that must never be shown"}"#
            )?,
            (IRRELEVANT.into(), false)
        );
        for raw in [
            "plain answer",
            r#"{"answer":"missing decision"}"#,
            r#"{"relevance":"other","answer":"x"}"#,
            r#"{"relevance":"minecraft","answer":"x","reasoning":"private"}"#,
        ] {
            assert!(decode(raw).is_err());
        }
        assert_eq!(
            decode(r#"{"relevance":"minecraft","answer":"Build\na roof."}"#)?,
            ("Build a roof.".into(), true)
        );
        Ok(())
    }
}
