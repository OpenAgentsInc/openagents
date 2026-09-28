//! NIP-EVAL Gym results publication (`nips/openagents/NIP-EVAL.md`,
//! "Gym results publication").
//!
//! A `3195` event is one signed statement that a publisher released one
//! Gym leaderboard: its content digest, the commit its evidence was read
//! at, and its board IDs. This module builds the unsigned parts and checks
//! a signed event's signature, tags, and body. Whether the signer is a
//! publisher the reader trusts, and whether the digest, commit, and boards
//! match the leaderboard and index entry the reader verified, belong to
//! the reader (`gym_leaderboard::signed`).

use serde_json::{Map, Value, json};

use crate::contracts::{ContractError, RefusalCode, parse_strict};
use crate::domain::Event;
use crate::kb::{
    Unsigned, is_hex, malformed, mismatch, one_tag, reject, require, requires_empty, t_values, tag,
    text,
};

/// One Gym results publication.
pub const PUBLICATION_KIND: u16 = 3_195;

/// The body version.
pub const PUBLICATION_VERSION: &str = "openagents.gym-results-publication.v1";

/// The leaderboard schema a publication names.
pub const LEADERBOARD_SCHEMA: &str = "openagents.gym.leaderboard.v1";

/// The `t` marker every publication carries.
pub const MARKER: &str = "oa:gym-results:v1";

/// The most boards one publication lists.
pub const MAX_BOARDS: usize = 64;

/// The longest board ID.
pub const MAX_BOARD_ID_CHARS: usize = 128;

/// A verified `3195`: what the publisher signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    /// The event ID.
    pub id: String,
    /// The signer's hex public key.
    pub publisher: String,
    pub created_at: u64,
    /// The leaderboard's content digest, 64 lowercase hex.
    pub digest: String,
    /// The commit the evidence was read at, 40 lowercase hex.
    pub commit: String,
    /// The board IDs, in the leaderboard's order.
    pub boards: Vec<String>,
}

/// The parts of a `3195` for the leaderboard with content digest `digest`,
/// read at `commit`, holding `boards`.
///
/// # Errors
///
/// A digest that isn't 64 lowercase hex, a commit that isn't 40, or a
/// board list that is empty, too long, repeats an ID, or has a bad ID.
pub fn publication(
    digest: &str,
    commit: &str,
    boards: &[String],
) -> Result<Unsigned, ContractError> {
    check(digest, commit, boards)?;
    let content = json!({
        "v": PUBLICATION_VERSION,
        "requires": [],
        "schema": LEADERBOARD_SCHEMA,
        "digest": digest,
        "commit": commit,
        "boards": boards,
    });
    Ok(Unsigned {
        kind: PUBLICATION_KIND,
        tags: vec![
            tag(&["t", MARKER]),
            tag(&["x", digest]),
            tag(&[
                "alt",
                &format!("OpenAgents Gym results publication {digest}"),
            ]),
        ],
        content: content.to_string(),
    })
}

/// Checks a signed `3195` and returns what it states.
///
/// # Errors
///
/// A typed refusal naming the first check that failed: the kind, the
/// signature, the marker, the body's shape and version, or an `x` tag
/// that disagrees with the body.
pub fn parse_publication(event: &Event) -> Result<Publication, ContractError> {
    if event.kind != PUBLICATION_KIND {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let markers: Vec<&str> = t_values(event).filter(|t| t.starts_with("oa:")).collect();
    if markers != [MARKER] {
        return Err(mismatch("gym-results tag"));
    }
    let value = parse_strict(event.content.as_bytes())?;
    let object: &Map<String, Value> = value.as_object().ok_or_else(|| malformed("publication"))?;
    reject(
        object,
        &["v", "requires", "schema", "digest", "commit", "boards"],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(PUBLICATION_VERSION) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    if text(object, "schema")? != LEADERBOARD_SCHEMA {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "schema",
        ));
    }
    let digest = text(object, "digest")?;
    let commit = text(object, "commit")?;
    let boards = require(object, "boards")?
        .as_array()
        .ok_or_else(|| malformed("boards"))?
        .iter()
        .map(|b| {
            b.as_str()
                .map(str::to_string)
                .ok_or_else(|| malformed("boards"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    check(&digest, &commit, &boards)?;
    if one_tag(event, "x")? != digest {
        return Err(mismatch("x tag"));
    }
    Ok(Publication {
        id: event.id.clone(),
        publisher: event.pubkey.clone(),
        created_at: event.created_at,
        digest,
        commit,
        boards,
    })
}

fn check(digest: &str, commit: &str, boards: &[String]) -> Result<(), ContractError> {
    if !is_hex(digest) {
        return Err(malformed("digest"));
    }
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(malformed("commit"));
    }
    if boards.is_empty() {
        return Err(malformed("boards"));
    }
    if boards.len() > MAX_BOARDS {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "boards"));
    }
    for (i, board) in boards.iter().enumerate() {
        let valid = !board.is_empty()
            && board.chars().count() <= MAX_BOARD_ID_CHARS
            && board
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
        if !valid {
            return Err(malformed(format!("board {board}")));
        }
        if boards[..i].contains(board) {
            return Err(malformed(format!("board {board} twice")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
