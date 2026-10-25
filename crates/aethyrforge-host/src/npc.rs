//! Version 8 non-player characters: what the engine reports, and what a host may ask.
//!
//! Lines come from the engine's checked-in text dataset; no model is called.
//! The host shows them (for example in a chat card) and may ask one character
//! for a hint, a reply, or an emote. Addressing is by exact character ID.
use crate::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// At most this many characters are listed in one world.
pub const MAX_NPCS: usize = 16;
/// Status keeps the newest lines only; it is a latest value, not a log.
pub const MAX_NPC_LINES: usize = 4;
pub const MAX_NPC_NAME_BYTES: usize = 64;
pub const MAX_NPC_TEXT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NpcLineKind {
    /// Said once when the player comes near.
    Greet,
    /// The next step, when asked.
    Hint,
    /// An answer when the character is named.
    Reply,
    /// A gesture with a short description.
    Emote,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NpcEmote {
    Wave,
    Hop,
    Spin,
    Bow,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TalkIntent {
    Reply,
    Hint,
    Emote,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NpcInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NpcLine {
    /// Strictly increasing within one engine session.
    pub sequence: u64,
    pub npc: String,
    pub name: String,
    pub kind: NpcLineKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emote: Option<NpcEmote>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NpcView {
    pub npcs: Vec<NpcInfo>,
    /// The character close enough to talk to, if any.
    pub nearby: Option<String>,
    /// The newest lines, oldest first.
    pub lines: Vec<NpcLine>,
}

fn name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= MAX_NPC_NAME_BYTES && !s.chars().any(char::is_control)
}

impl NpcView {
    pub fn validate(&self) -> Result<(), Error> {
        let known = |id: &str| self.npcs.iter().any(|n| n.id == id);
        let valid = self.npcs.len() <= MAX_NPCS
            && self.npcs.iter().enumerate().all(|(i, n)| {
                crate::protocol::id(&n.id)
                    && name(&n.name)
                    && !self.npcs[..i].iter().any(|o| o.id == n.id)
            })
            && self.nearby.as_deref().is_none_or(known)
            && self.lines.len() <= MAX_NPC_LINES
            && self.lines.iter().enumerate().all(|(i, l)| {
                known(&l.npc)
                    && name(&l.name)
                    && !l.text.trim().is_empty()
                    && l.text.len() <= MAX_NPC_TEXT_BYTES
                    && !l.text.contains('\0')
                    && l.sequence > 0
                    && l.sequence <= crate::MAX_SAFE_INTEGER
                    && (i == 0 || self.lines[i - 1].sequence < l.sequence)
                    && (l.kind == NpcLineKind::Emote) == l.emote.is_some()
            });
        if valid {
            Ok(())
        } else {
            Err(Error::Invalid("npc"))
        }
    }
}

/// The character a chat message names with a leading `@id`, e.g. `@ghost hello`.
///
/// This is an exact, bounded ID parse of the addressing prefix only. The rest
/// of the message is never inspected to choose what the character says.
pub fn npc_mention(text: &str) -> Option<&str> {
    let rest = text.trim_start().strip_prefix('@')?;
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    let id = &rest[..end];
    (!id.is_empty() && id.len() <= 128).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> NpcView {
        NpcView {
            npcs: vec![
                NpcInfo {
                    id: "ghost".into(),
                    name: "Ghost".into(),
                },
                NpcInfo {
                    id: "cat".into(),
                    name: "Black Cat".into(),
                },
            ],
            nearby: Some("cat".into()),
            lines: vec![
                NpcLine {
                    sequence: 1,
                    npc: "cat".into(),
                    name: "Black Cat".into(),
                    kind: NpcLineKind::Greet,
                    text: "Mrrow.".into(),
                    emote: None,
                },
                NpcLine {
                    sequence: 2,
                    npc: "cat".into(),
                    name: "Black Cat".into(),
                    kind: NpcLineKind::Emote,
                    text: "flicks her tail".into(),
                    emote: Some(NpcEmote::Spin),
                },
            ],
        }
    }
    #[test]
    fn a_valid_view_passes_and_each_bound_fails_closed() {
        view().validate().unwrap();
        let cases: Vec<fn(&mut NpcView)> = vec![
            |v| v.nearby = Some("witch".into()),
            |v| v.lines[0].npc = "witch".into(),
            |v| v.lines[1].sequence = 1,
            |v| v.lines[0].sequence = 0,
            |v| v.lines[0].text = "x".repeat(MAX_NPC_TEXT_BYTES + 1),
            |v| v.lines[0].text = " ".into(),
            |v| v.lines[0].emote = Some(NpcEmote::Hop),
            |v| v.lines[1].emote = None,
            |v| v.npcs[1].id = "ghost".into(),
            |v| v.npcs[0].name = "\u{7}".into(),
            |v| {
                v.lines = (1..=5)
                    .map(|s| NpcLine {
                        sequence: s,
                        ..v.lines[0].clone()
                    })
                    .collect()
            },
        ];
        for change in cases {
            let mut v = view();
            change(&mut v);
            assert!(v.validate().is_err(), "{v:?}");
        }
    }
    #[test]
    fn mentions_parse_only_the_leading_id() {
        assert_eq!(npc_mention("@ghost where next?"), Some("ghost"));
        assert_eq!(npc_mention("  @black-cat"), Some("black-cat"));
        assert_eq!(npc_mention("@witch, hi"), Some("witch"));
        assert_eq!(npc_mention("hello @ghost"), None);
        assert_eq!(npc_mention("@"), None);
        assert_eq!(npc_mention("@ ghost"), None);
    }
}
