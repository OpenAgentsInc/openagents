//! NIP-64: Chess (Portable Game Notation)
//!
//! Defines kind 64 events representing chess games in PGN (Portable Game Notation)
//! format, which can be read by humans and processed by chess software.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/64.md>

use crate::Event;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

/// Event kind for chess games
pub const CHESS_GAME_KIND: u16 = 64;

/// Errors that can occur during NIP-64 operations
#[derive(Debug, Error)]
pub enum Nip64Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid PGN format: {0}")]
    InvalidPgn(String),

    #[error("missing tag: {0}")]
    MissingTag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Game result from PGN
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameResult {
    /// White wins
    WhiteWins,
    /// Black wins
    BlackWins,
    /// Draw
    Draw,
    /// Unknown or in progress
    Unknown,
}

impl GameResult {
    pub fn as_str(&self) -> &str {
        match self {
            GameResult::WhiteWins => "1-0",
            GameResult::BlackWins => "0-1",
            GameResult::Draw => "1/2-1/2",
            GameResult::Unknown => "*",
        }
    }
}

impl std::str::FromStr for GameResult {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim() {
            "1-0" => GameResult::WhiteWins,
            "0-1" => GameResult::BlackWins,
            "1/2-1/2" => GameResult::Draw,
            "*" => GameResult::Unknown,
            _ => GameResult::Unknown,
        })
    }
}

/// A chess game in PGN format
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChessGame {
    pub event: Event,
    /// PGN content
    pub pgn: String,
    /// Parsed PGN tags
    pub tags: HashMap<String, String>,
}

impl ChessGame {
    /// Create a chess game from an event
    pub fn from_event(event: Event) -> Result<Self, Nip64Error> {
        if event.kind != CHESS_GAME_KIND {
            return Err(Nip64Error::InvalidKind {
                expected: CHESS_GAME_KIND,
                actual: event.kind,
            });
        }

        let pgn = event.content.clone();
        let tags = parse_pgn_tags(&pgn);

        Ok(Self { event, pgn, tags })
    }

    /// Get a PGN tag value
    pub fn get_tag(&self, tag: &str) -> Option<&str> {
        self.tags.get(tag).map(|s| s.as_str())
    }

    /// Get the event name
    pub fn get_event_name(&self) -> Option<&str> {
        self.get_tag("Event")
    }

    /// Get the site/location
    pub fn get_site(&self) -> Option<&str> {
        self.get_tag("Site")
    }

    /// Get the date
    pub fn get_date(&self) -> Option<&str> {
        self.get_tag("Date")
    }

    /// Get the round
    pub fn get_round(&self) -> Option<&str> {
        self.get_tag("Round")
    }

    /// Get the white player's name
    pub fn get_white(&self) -> Option<&str> {
        self.get_tag("White")
    }

    /// Get the black player's name
    pub fn get_black(&self) -> Option<&str> {
        self.get_tag("Black")
    }

    /// Get the game result
    pub fn get_result(&self) -> GameResult {
        self.get_tag("Result")
            .and_then(|value| GameResult::from_str(value).ok())
            .unwrap_or(GameResult::Unknown)
    }

    /// Get the moves section (everything after tags)
    pub fn get_moves(&self) -> &str {
        // Find the end of tags section (double newline or start of moves)
        let content = self.pgn.as_str();

        // Skip all tag pairs
        let in_tags = true;
        let mut start_index = 0;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if in_tags {
                if trimmed.starts_with('[') {
                    continue; // Still in tags
                } else if !trimmed.is_empty() {
                    // Found first non-tag, non-empty line
                    start_index = content.lines().take(i).map(|l| l.len() + 1).sum();
                    break;
                }
            }
        }

        content[start_index..].trim()
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Validate basic PGN structure
    pub fn validate(&self) -> Result<(), Nip64Error> {
        if self.event.kind != CHESS_GAME_KIND {
            return Err(Nip64Error::InvalidKind {
                expected: CHESS_GAME_KIND,
                actual: self.event.kind,
            });
        }

        if self.pgn.is_empty() {
            return Err(Nip64Error::InvalidPgn("PGN content is empty".to_string()));
        }

        Ok(())
    }
}

/// Parse PGN tags from content
/// Tags are in format: [TagName "TagValue"]
fn parse_pgn_tags(pgn: &str) -> HashMap<String, String> {
    let mut tags = HashMap::new();

    for line in pgn.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Extract tag name and value
            let inner = &trimmed[1..trimmed.len() - 1];
            if let Some(quote_start) = inner.find('"') {
                let tag_name = inner[..quote_start].trim().to_string();
                if let Some(quote_end) = inner.rfind('"')
                    && quote_end > quote_start
                {
                    let tag_value = inner[quote_start + 1..quote_end].to_string();
                    tags.insert(tag_name, tag_value);
                }
            }
        }
    }

    tags
}

/// Create a minimal chess game event content
pub fn create_chess_game(moves: &str) -> String {
    moves.to_string()
}

/// Create a chess game with tags
pub fn create_chess_game_with_tags(tags: &HashMap<String, String>, moves: &str) -> String {
    let mut pgn = String::new();

    // Add tags in Seven Tag Roster order if present
    let roster = ["Event", "Site", "Date", "Round", "White", "Black", "Result"];
    for tag in &roster {
        if let Some(value) = tags.get(*tag) {
            pgn.push_str(&format!("[{} \"{}\"]\n", tag, value));
        }
    }

    // Add other tags
    for (key, value) in tags {
        if !roster.contains(&key.as_str()) {
            pgn.push_str(&format!("[{} \"{}\"]\n", key, value));
        }
    }

    // Add blank line between tags and moves
    if !tags.is_empty() {
        pgn.push('\n');
    }

    // Add moves
    pgn.push_str(moves);

    pgn
}

/// Check if an event kind is a chess game
pub fn is_chess_game_kind(kind: u16) -> bool {
    kind == CHESS_GAME_KIND
}

/// Extract the alt tag description for a chess game
pub fn create_alt_description(
    white: &str,
    black: &str,
    event: Option<&str>,
    date: Option<&str>,
) -> String {
    let mut parts = vec![white, "vs.", black];

    if let Some(event_name) = event {
        parts.push("in");
        parts.push(event_name);
    }

    if let Some(game_date) = date {
        parts.push("on");
        parts.push(game_date);
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(pgn: &str) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: CHESS_GAME_KIND,
            tags: vec![],
            content: pgn.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_chess_game_minimal() {
        let pgn = "1. e4 *";
        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert_eq!(game.pgn, pgn);
        assert_eq!(game.get_result(), GameResult::Unknown);
    }

    #[test]
    fn test_chess_game_with_tags() {
        let pgn = r#"[Event "Test Game"]
[Site "Online"]
[Date "2024.01.01"]
[Round "1"]
[White "Player1"]
[Black "Player2"]
[Result "1-0"]

1. e4 e5 2. Nf3 1-0"#;

        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert_eq!(game.get_event_name(), Some("Test Game"));
        assert_eq!(game.get_site(), Some("Online"));
        assert_eq!(game.get_date(), Some("2024.01.01"));
        assert_eq!(game.get_round(), Some("1"));
        assert_eq!(game.get_white(), Some("Player1"));
        assert_eq!(game.get_black(), Some("Player2"));
        assert_eq!(game.get_result(), GameResult::WhiteWins);
    }

    #[test]
    fn test_chess_game_fischer_spassky() {
        let pgn = r#"[Event "F/S Return Match"]
[Site "Belgrade, Serbia JUG"]
[Date "1992.11.04"]
[Round "29"]
[White "Fischer, Robert J."]
[Black "Spassky, Boris V."]
[Result "1/2-1/2"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 1/2-1/2"#;

        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert_eq!(game.get_event_name(), Some("F/S Return Match"));
        assert_eq!(game.get_site(), Some("Belgrade, Serbia JUG"));
        assert_eq!(game.get_white(), Some("Fischer, Robert J."));
        assert_eq!(game.get_black(), Some("Spassky, Boris V."));
        assert_eq!(game.get_result(), GameResult::Draw);
    }

    #[test]
    fn test_game_result_from_str() {
        assert!(matches!(
            GameResult::from_str("1-0"),
            Ok(GameResult::WhiteWins)
        ));
        assert!(matches!(
            GameResult::from_str("0-1"),
            Ok(GameResult::BlackWins)
        ));
        assert!(matches!(
            GameResult::from_str("1/2-1/2"),
            Ok(GameResult::Draw)
        ));
        assert!(matches!(
            GameResult::from_str("*"),
            Ok(GameResult::Unknown)
        ));
        assert!(matches!(
            GameResult::from_str("invalid"),
            Ok(GameResult::Unknown)
        ));
    }

    #[test]
    fn test_game_result_as_str() {
        assert_eq!(GameResult::WhiteWins.as_str(), "1-0");
        assert_eq!(GameResult::BlackWins.as_str(), "0-1");
        assert_eq!(GameResult::Draw.as_str(), "1/2-1/2");
        assert_eq!(GameResult::Unknown.as_str(), "*");
    }

    #[test]
    fn test_get_moves() {
        let pgn = r#"[White "Player1"]
[Black "Player2"]

1. e4 e5 2. Nf3 *"#;

        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert_eq!(game.get_moves(), "1. e4 e5 2. Nf3 *");
    }

    #[test]
    fn test_get_moves_no_tags() {
        let pgn = "1. e4 *";
        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert_eq!(game.get_moves(), "1. e4 *");
    }

    #[test]
    fn test_chess_game_invalid_kind() {
        let mut event = create_test_event("1. e4 *");
        event.kind = 1;

        let result = ChessGame::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip64Error::InvalidKind {
                expected: CHESS_GAME_KIND,
                actual: 1
            }
        ));
    }

    #[test]
    fn test_chess_game_validate() {
        let pgn = "1. e4 *";
        let event = create_test_event(pgn);
        let game = ChessGame::from_event(event).unwrap();

        assert!(game.validate().is_ok());
    }

    #[test]
    fn test_chess_game_validate_empty() {
        let event = create_test_event("");
        let game = ChessGame::from_event(event).unwrap();

        let result = game.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_create_chess_game() {
        let moves = "1. e4 e5 2. Nf3 *";
        let pgn = create_chess_game(moves);
        assert_eq!(pgn, moves);
    }

    #[test]
    fn test_create_chess_game_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("White".to_string(), "Player1".to_string());
        tags.insert("Black".to_string(), "Player2".to_string());
        tags.insert("Result".to_string(), "1-0".to_string());

        let pgn = create_chess_game_with_tags(&tags, "1. e4 1-0");

        assert!(pgn.contains("[White \"Player1\"]"));
        assert!(pgn.contains("[Black \"Player2\"]"));
        assert!(pgn.contains("[Result \"1-0\"]"));
        assert!(pgn.contains("1. e4 1-0"));
    }

    #[test]
    fn test_create_chess_game_with_seven_tag_roster() {
        let mut tags = HashMap::new();
        tags.insert("Event".to_string(), "Test".to_string());
        tags.insert("Site".to_string(), "Online".to_string());
        tags.insert("Date".to_string(), "2024.01.01".to_string());
        tags.insert("Round".to_string(), "1".to_string());
        tags.insert("White".to_string(), "Player1".to_string());
        tags.insert("Black".to_string(), "Player2".to_string());
        tags.insert("Result".to_string(), "*".to_string());

        let pgn = create_chess_game_with_tags(&tags, "1. e4 *");

        // Check tags appear in Seven Tag Roster order
        let event_pos = pgn.find("[Event").unwrap();
        let site_pos = pgn.find("[Site").unwrap();
        let white_pos = pgn.find("[White").unwrap();
        let result_pos = pgn.find("[Result").unwrap();

        assert!(event_pos < site_pos);
        assert!(site_pos < white_pos);
        assert!(white_pos < result_pos);
    }

    #[test]
    fn test_is_chess_game_kind() {
        assert!(is_chess_game_kind(CHESS_GAME_KIND));
        assert!(!is_chess_game_kind(1));
        assert!(!is_chess_game_kind(0));
    }

    #[test]
    fn test_create_alt_description() {
        let alt = create_alt_description("Fischer", "Spassky", None, None);
        assert_eq!(alt, "Fischer vs. Spassky");

        let alt = create_alt_description("Fischer", "Spassky", Some("World Championship"), None);
        assert_eq!(alt, "Fischer vs. Spassky in World Championship");

        let alt = create_alt_description(
            "Fischer",
            "Spassky",
            Some("World Championship"),
            Some("1972.07.11"),
        );
        assert_eq!(
            alt,
            "Fischer vs. Spassky in World Championship on 1972.07.11"
        );
    }

    #[test]
    fn test_parse_pgn_tags() {
        let pgn = r#"[Event "Test"]
[White "Player1"]
[Black "Player2"]

1. e4 *"#;

        let tags = parse_pgn_tags(pgn);
        assert_eq!(tags.get("Event"), Some(&"Test".to_string()));
        assert_eq!(tags.get("White"), Some(&"Player1".to_string()));
        assert_eq!(tags.get("Black"), Some(&"Player2".to_string()));
    }

    #[test]
    fn test_chess_game_get_author() {
        let event = create_test_event("1. e4 *");
        let game = ChessGame::from_event(event).unwrap();
        assert_eq!(game.get_author(), "test_pubkey");
    }

    #[test]
    fn test_chess_game_get_created_at() {
        let event = create_test_event("1. e4 *");
        let game = ChessGame::from_event(event).unwrap();
        assert_eq!(game.get_created_at(), 1234567890);
    }
}
