//! Aggregate only complete sessions; retain uncertainty separately.
use super::event::Event;
use crate::error::Error;
use chrono::NaiveDateTime;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Default)]
pub struct Player {
    pub uuid: Option<String>,
    pub completed_sessions: u64,
    pub playtime_seconds: u64,
    pub incomplete_sessions: u64,
    pub unmatched_leaves: u64,
    pub duplicate_joins: u64,
    pub first_join: Option<NaiveDateTime>,
    pub last_seen: Option<NaiveDateTime>,
    pub open_session_since: Option<NaiveDateTime>,
    pub open_session_observed_seconds: u64,
}

#[derive(Default, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub timestamp_basis: &'static str,
    pub files: u64,
    pub lines: u64,
    pub ignored_lines: u64,
    pub out_of_order_lines: u64,
    pub observed_until: Option<NaiveDateTime>,
    pub players: BTreeMap<String, Player>,
}

impl Report {
    /// Guard memory before inserting names from untrusted logs.
    fn player(&mut self, name: &str, limit: usize) -> Result<&mut Player, Error> {
        if !self.players.contains_key(name) && self.players.len() >= limit {
            return Err(Error::PlayerLimit(limit));
        }
        Ok(self.players.entry(name.to_owned()).or_default())
    }

    /// End sessions at a known stop, or discard duration at an unclean restart/gap.
    pub fn boundary(&mut self, at: NaiveDateTime, clean: bool) {
        for player in self.players.values_mut() {
            if let Some(start) = player.open_session_since.take() {
                if clean {
                    player.playtime_seconds += (at - start).num_seconds().max(0) as u64;
                    player.completed_sessions += 1;
                    player.last_seen = Some(at);
                } else {
                    player.incomplete_sessions += 1;
                }
            }
        }
    }

    /// Process each activity once; the login-detail and lost-connection lines are ignored.
    pub fn apply(
        &mut self,
        at: NaiveDateTime,
        event: Event<'_>,
        limit: usize,
    ) -> Result<(), Error> {
        self.observed_until = Some(at);
        match event {
            Event::Join(name) => {
                let player = self.player(name, limit)?;
                match player.open_session_since {
                    Some(start) if start == at => player.duplicate_joins += 1,
                    Some(_) => {
                        // A later join can mean a lost disconnect, so never bridge it.
                        player.incomplete_sessions += 1;
                        player.open_session_since = Some(at);
                    }
                    None => player.open_session_since = Some(at),
                }
                if player.first_join.is_none() {
                    player.first_join = Some(at);
                }
                player.last_seen = Some(at);
            }
            Event::Leave(name) => {
                let player = self.player(name, limit)?;
                match player.open_session_since.take() {
                    Some(start) => {
                        player.playtime_seconds += (at - start).num_seconds().max(0) as u64;
                        player.completed_sessions += 1;
                    }
                    None => player.unmatched_leaves += 1,
                }
                player.last_seen = Some(at);
            }
            Event::Identity(name, uuid) => {
                self.player(name, limit)?.uuid = Some(uuid.to_ascii_lowercase());
            }
            Event::Stop => self.boundary(at, true),
            Event::Start => self.boundary(at, false),
            Event::Other => self.ignored_lines += 1,
        }
        Ok(())
    }

    /// Open durations stop at the last log timestamp, never at the current wall clock.
    pub fn finish(&mut self) {
        self.schema_version = 1;
        self.timestamp_basis = "server_local_wall_clock";
        if let Some(end) = self.observed_until {
            for player in self.players.values_mut() {
                if let Some(start) = player.open_session_since {
                    player.open_session_observed_seconds =
                        (end - start).num_seconds().max(0) as u64;
                }
            }
        }
    }
}
