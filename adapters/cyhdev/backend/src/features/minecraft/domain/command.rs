//! The supported operations and their input bounds.

pub enum Command {
    Message(String),
    WhitelistAdd(String),
    WhitelistRemove(String),
    WhitelistEnable(bool),
    Kick(String),
    MapVisibility { id: uuid::Uuid, hidden: bool },
    Save,
    Restart,
}

impl Command {
    /// Reject control characters and excessive text before any external side effect.
    pub fn valid(&self) -> bool {
        match self {
            Self::Message(text) => {
                !text.trim().is_empty()
                    && text.chars().count() <= 512
                    && !text.chars().any(char::is_control)
            }
            Self::WhitelistAdd(name) | Self::WhitelistRemove(name) | Self::Kick(name) => {
                (1..=16).contains(&name.len())
                    && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }
            Self::MapVisibility { id, .. } => !id.is_nil(),
            Self::WhitelistEnable(_) | Self::Save | Self::Restart => true,
        }
    }
}
