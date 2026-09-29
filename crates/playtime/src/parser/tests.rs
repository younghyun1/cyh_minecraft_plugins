//! Boundary cases for event recognition and conservative duration accounting.
use super::{
    event::{self, Event},
    state::Report,
};
use chrono::NaiveDateTime;

/// Parse fixed fixture times while propagating malformed fixture errors.
fn at(time: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(time, "%Y-%m-%d %H:%M:%S")
}

/// Both Paper layouts and the installed system-chat prefix are recognized.
#[test]
fn layouts_and_spoofing() {
    for line in [
        "[12:00:00] [Server thread/INFO]: Alex joined the game",
        "[12:00:00] [Server thread/INFO]: System chat: Alex joined the game",
        "[12:00:00 INFO]: Alex joined the game",
    ] {
        assert!(matches!(event::parse(line), Some((_, Event::Join("Alex")))));
    }
    for line in [
        "[12:00:00] [Async Chat Thread - #1/INFO]: Alex joined the game",
        "[12:00:00 INFO]: <Alex> Steve joined the game",
        "[12:00:00 INFO]: [Plugin] Steve joined the game",
        "[12:00:00 INFO]: é joined the game",
        "[12:00:00 INFO]: Alex[/127.0.0.1:1] logged in with entity id 5",
        "[12:00:00 INFO]: Alex lost connection: Disconnected",
    ] {
        assert!(matches!(event::parse(line), Some((_, Event::Other))));
    }
    for line in ["", "[é", "[99:00:00 INFO]: Alex joined the game"] {
        assert!(event::parse(line).is_none());
    }
}

/// Duplicate joins preserve the earliest timestamp; reconnects add separate sessions.
#[test]
fn reconnects_and_duplicates() -> Result<(), Box<dyn std::error::Error>> {
    let mut report = Report::default();
    for (time, event) in [
        ("2026-09-27 23:59:00", Event::Join("Alex")),
        ("2026-09-27 23:59:00", Event::Join("Alex")),
        ("2026-09-28 00:01:00", Event::Leave("Alex")),
        ("2026-09-28 00:02:00", Event::Join("Alex")),
        ("2026-09-28 00:03:00", Event::Stop),
    ] {
        report.apply(at(time)?, event, 10)?;
    }
    let player = &report.players["Alex"];
    assert_eq!(player.playtime_seconds, 180);
    assert_eq!(player.completed_sessions, 2);
    assert_eq!(player.duplicate_joins, 1);
    Ok(())
}

/// Missing endpoints never enter confirmed totals; restarts clear active state.
#[test]
fn uncertain_sessions() -> Result<(), Box<dyn std::error::Error>> {
    let mut report = Report::default();
    let start = at("2026-09-28 01:00:00")?;
    let end = at("2026-09-28 02:00:00")?;
    report.apply(start, Event::Leave("Alex"), 10)?;
    report.apply(start, Event::Join("Alex"), 10)?;
    report.apply(end, Event::Start, 10)?;
    report.apply(end, Event::Join("Alex"), 10)?;
    report.apply(at("2026-09-28 02:10:00")?, Event::Other, 10)?;
    report.finish();
    let player = &report.players["Alex"];
    assert_eq!(player.playtime_seconds, 0);
    assert_eq!(player.unmatched_leaves, 1);
    assert_eq!(player.incomplete_sessions, 1);
    assert_eq!(player.open_session_observed_seconds, 600);
    assert!(report.apply(end, Event::Join("Steve"), 1).is_err());
    Ok(())
}

/// UUID metadata is accepted only when its shape is valid.
#[test]
fn identities() {
    assert!(matches!(
        event::parse(
            "[01:00:00] [User Authenticator #1/INFO]: UUID of player Alex is 00000000-0000-4000-8000-000000000001"
        ),
        Some((_, Event::Identity("Alex", _)))
    ));
    assert!(matches!(
        event::parse("[01:00:00 INFO]: UUID of player Alex is invalid"),
        Some((_, Event::Other))
    ));
}
