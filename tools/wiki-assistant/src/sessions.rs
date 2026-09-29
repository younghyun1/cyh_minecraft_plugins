//! Bounded shared conversations, keyed by the Bukkit world UUID rather than the speaker.
use crate::{
    error::{Error, Result},
    rpc::Codex,
};
use std::collections::HashMap;

pub struct Session {
    pub thread: String,
    pub previous_question: String,
}

#[derive(Default)]
pub struct Sessions {
    entries: HashMap<String, Session>,
}

/// Names may change; UUID remains the session authority supplied by the plugin.
fn valid_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

pub fn valid_sender(uuid: &str, username: &str) -> bool {
    valid_uuid(uuid)
        && !username.is_empty()
        && username.len() <= 16
        && username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

pub fn valid_world(uuid: &str, name: &str) -> bool {
    valid_uuid(uuid)
        && !name.trim().is_empty()
        && name.chars().count() <= 128
        && !name.chars().any(char::is_control)
}

impl Sessions {
    /// Reject new worlds at capacity without forgetting existing conversations.
    pub fn can_admit(&self, uuid: &str) -> bool {
        self.entries.contains_key(uuid) || self.entries.len() < 32
    }

    /// Keep every admitted world's context for the process lifetime; Codex handles compaction.
    pub async fn acquire(&mut self, uuid: &str, codex: &mut Codex) -> Result<&mut Session> {
        if !self.entries.contains_key(uuid) {
            if self.entries.len() >= 32 {
                return Err(Error::Invalid(
                    "32 world conversations are already active".into(),
                ));
            }
            self.entries.insert(
                uuid.into(),
                Session {
                    thread: codex.start_thread().await?,
                    previous_question: String::new(),
                },
            );
        }
        self.entries.get_mut(uuid).ok_or(Error::Codex)
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

    #[test]
    fn world_names_are_bounded_and_sessions_never_evict_at_capacity() {
        assert!(super::valid_world(
            "12345678-1234-1234-1234-123456789abc",
            "Survival World"
        ));
        assert!(!super::valid_world(
            "12345678-1234-1234-1234-123456789abc",
            "world\nspoof"
        ));
        assert!(!super::valid_world(
            "12345678-1234-1234-1234-123456789abc",
            &"x".repeat(129)
        ));
        let mut sessions = super::Sessions::default();
        for i in 0..32 {
            sessions.entries.insert(
                i.to_string(),
                super::Session {
                    thread: i.to_string(),
                    previous_question: String::new(),
                },
            );
        }
        assert!(sessions.can_admit("0"));
        assert!(!sessions.can_admit("new-world"));
        assert_eq!(sessions.entries.len(), 32);
    }
}
