//! Allocation-free recognition of trusted server messages.
use chrono::NaiveTime;

#[derive(Debug, PartialEq)]
pub enum Event<'a> {
    Join(&'a str),
    Leave(&'a str),
    Identity(&'a str, &'a str),
    Stop,
    Start,
    Other,
}

/// Minecraft Java names cannot contain whitespace or chat punctuation.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Validate canonical UUID spelling without allocating a UUID object.
fn valid_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// Accept file and console layouts; chat/plugin text never becomes player activity.
pub fn parse(line: &str) -> Option<(NaiveTime, Event<'_>)> {
    let after = line.strip_prefix('[')?;
    let time = NaiveTime::parse_from_str(after.get(..8)?, "%H:%M:%S").ok()?;
    let header = after.get(8..)?;
    let (message, auth, server) = match header.strip_prefix("] [") {
        Some(header) => {
            let (source, message) = header.split_once("]: ")?;
            (
                message,
                source.starts_with("User Authenticator #") && source.ends_with("/INFO"),
                source == "Server thread/INFO" || source == "ServerMain/INFO",
            )
        }
        None => (header.strip_prefix(" INFO]: ")?, true, true),
    };
    if auth
        && let Some(identity) = message.strip_prefix("UUID of player ")
        && let Some((name, uuid)) = identity.split_once(" is ")
        && valid_name(name)
        && valid_uuid(uuid)
    {
        return Some((time, Event::Identity(name, uuid)));
    }
    if !server {
        return Some((time, Event::Other));
    }
    let message = match message.strip_prefix("System chat: ") {
        Some(message) => message,
        None => message,
    };
    let event = if message == "Stopping server" {
        Event::Stop
    } else if message.starts_with("Starting minecraft server version ") {
        Event::Start
    } else if let Some(name) = message
        .strip_suffix(" joined the game")
        .filter(|name| valid_name(name))
    {
        Event::Join(name)
    } else if let Some(name) = message
        .strip_suffix(" left the game")
        .filter(|name| valid_name(name))
    {
        Event::Leave(name)
    } else {
        Event::Other
    };
    Some((time, event))
}
