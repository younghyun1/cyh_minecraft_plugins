//! Bounded status codes expose lifecycle phases without exposing model content.
use crate::error::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Connecting,
    Answering,
    Searching,
    Compacting,
    Clearing,
    Resetting,
}

#[derive(Default)]
pub struct Progress {
    request: Option<u64>,
    thread: Option<String>,
    resume_answer: bool,
    phase: Option<Phase>,
    sent: u8,
}

impl Progress {
    /// Only production bridge requests enable the stdout status channel.
    pub fn begin(&mut self, id: u64) {
        *self = Self {
            request: Some(id),
            ..Self::default()
        };
    }

    pub fn thread(&mut self, thread: &str, resume_answer: bool) {
        self.thread = Some(thread.into());
        self.resume_answer = resume_answer;
    }

    pub async fn emit(&mut self, phase: Phase) -> Result<()> {
        let frame = match self.frame(phase) {
            Some(frame) => frame,
            None => return Ok(()),
        };
        let mut output = tokio::io::stdout();
        match output.write_all(format!("{frame}\n").as_bytes()).await {
            Ok(()) => {}
            Err(error) => return Err(Error::Io(error)),
        }
        match output.flush().await {
            Ok(()) => Ok(()),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn frame(&mut self, phase: Phase) -> Option<Value> {
        let id = self.request?;
        if self.phase == Some(phase) || self.sent >= 32 {
            return None;
        }
        self.phase = Some(phase);
        self.sent += 1;
        Some(json!({"id":id,"status":phase}))
    }

    /// Called by the RPC reader, including notifications preceding a turn/start acknowledgement.
    pub async fn observe(&mut self, event: &Value) -> Result<()> {
        match self.event_phase(event) {
            Some(phase) => self.emit(phase).await,
            None => Ok(()),
        }
    }

    fn event_phase(&self, event: &Value) -> Option<Phase> {
        if self.thread.is_none()
            || event["params"]["threadId"].as_str() != self.thread.as_deref()
            || event["params"]["item"]["type"] != "contextCompaction"
        {
            return None;
        }
        match event["method"].as_str() {
            Some("item/started") => Some(Phase::Compacting),
            Some("item/completed") if self.resume_answer => Some(Phase::Answering),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_is_bounded_deduplicated_and_contains_no_model_text() {
        let mut progress = Progress::default();
        assert!(progress.frame(Phase::Answering).is_none());
        progress.begin(7);
        assert_eq!(
            progress.frame(Phase::Answering),
            Some(json!({"id":7,"status":"answering"}))
        );
        assert!(progress.frame(Phase::Answering).is_none());
        for _ in 0..100 {
            progress.frame(Phase::Compacting);
            progress.frame(Phase::Answering);
        }
        assert_eq!(progress.sent, 32);
        progress.thread("world-a", true);
        let mut event = json!({"method":"item/started","params":{"threadId":"world-b","item":{"type":"contextCompaction","text":"private"}}});
        assert!(progress.event_phase(&event).is_none());
        event["params"]["threadId"] = json!("world-a");
        assert!(progress.event_phase(&event) == Some(Phase::Compacting));
        event["method"] = json!("item/completed");
        assert!(progress.event_phase(&event) == Some(Phase::Answering));
        progress.thread("world-a", false);
        assert!(progress.event_phase(&event).is_none());
    }
}
