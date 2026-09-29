//! Bounded per-player conversation state, keyed by authenticated Bukkit UUID.
use crate::{
    error::{Error, Result},
    rpc::Codex,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub struct Session {
    pub thread: String,
    pub previous_question: String,
    pub last_used: Instant,
}

#[derive(Default)]
pub struct Sessions {
    entries: HashMap<String, Session>,
}

/// Names may change; UUID remains the session authority supplied by the plugin.
pub fn valid_sender(uuid: &str, username: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
        && !username.is_empty()
        && username.len() <= 16
        && username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

impl Sessions {
    /// Expire idle sessions; Codex compacts long conversations instead of discarding prior context.
    pub async fn acquire(&mut self, uuid: &str, codex: &mut Codex) -> Result<&mut Session> {
        let expired: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, session)| session.last_used.elapsed() >= Duration::from_secs(1800))
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.remove(&id, codex).await?;
        }
        if !self.entries.contains_key(uuid) {
            if self.entries.len() >= 32 {
                let oldest = self
                    .entries
                    .iter()
                    .min_by_key(|(_, session)| session.last_used)
                    .map(|(id, _)| id.clone());
                if let Some(id) = oldest {
                    self.remove(&id, codex).await?;
                }
            }
            self.entries.insert(
                uuid.into(),
                Session {
                    thread: codex.start_thread().await?,
                    previous_question: String::new(),
                    last_used: Instant::now(),
                },
            );
        }
        self.entries.get_mut(uuid).ok_or(Error::Codex)
    }

    pub async fn remove(&mut self, uuid: &str, codex: &mut Codex) -> Result<()> {
        if let Some(session) = self.entries.remove(uuid) {
            codex.unload(&session.thread).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn validates_identity_without_accepting_prompt_markup() {
        assert!(super::valid_sender(
            "12345678-1234-1234-1234-123456789abc",
            "Player_01"
        ));
        assert!(!super::valid_sender("not-a-uuid", "Player_01"));
        assert!(!super::valid_sender(
            "12345678-1234-1234-1234-123456789abc",
            "<admin>"
        ));
    }
}
