//! Transcript rail model shared by desktop and browser (#12270).
use openagents_doc::{MessagePart, MessageRole, SessionMessageEntry};
use std::collections::HashSet;

/// 48rem — the container width below which the rail (and wide gutters) collapse.
pub const RAIL_MIN_CONTAINER_WIDTH: f32 = 768.0;

pub fn rail_visible(container_width: f32) -> bool {
    container_width >= RAIL_MIN_CONTAINER_WIDTH
}

/// Preview text caps (grapheme-unaware char cut is fine for a preview card).
pub const PREVIEW_PROMPT_CHARS: usize = 160;
pub const PREVIEW_REPLY_CHARS: usize = 200;

/// One rail tick: a user prompt and the opening of the reply that followed.
#[derive(Debug, Clone, PartialEq)]
pub struct RailTick {
    /// Message id — equals the user row's id in the transcript row model.
    pub message_id: String,
    pub prompt: String,
    pub reply: Option<String>,
}

fn user_text(entry: &SessionMessageEntry) -> String {
    // Ordinary long prompts need only their opening; attachment-only messages
    // and context blocks still use the complete attachment parser.
    if let Some(MessagePart::Text { text, .. }) = entry.parts.first() {
        let prefix: String = text.chars().take(1024).collect();
        if openagents_proto::view::single_line(&prefix).chars().count() > PREVIEW_PROMPT_CHARS
            && !prefix
                .as_bytes()
                .windows(b"attached images (local files".len())
                .any(|bytes| bytes.eq_ignore_ascii_case(b"attached images (local files"))
            && !prefix.contains(crate::appshots::CONTEXT_MARKER)
            && !prefix.contains('<')
        {
            return truncate_preview(&prefix, PREVIEW_PROMPT_CHARS);
        }
    }
    let raw = entry
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    // Attachment refs ride the message text — the rail shows the visible
    // prompt, or "Attached image(s)" for image-only sends
    // (message-attachments.ts `userMessageRailText`).
    crate::attachments::user_message_rail_text(&raw)
}

fn first_reply_text(entries: &[SessionMessageEntry]) -> Option<String> {
    entries
        .iter()
        .find(|e| e.role == MessageRole::Assistant)
        .and_then(|entry| {
            entry.parts.iter().find_map(|part| match part {
                MessagePart::Text { text, .. } if !text.trim().is_empty() => {
                    Some(truncate_preview(text.trim(), PREVIEW_REPLY_CHARS))
                }
                _ => None,
            })
        })
}

/// Extract rail ticks from the transcript: one per user entry (doc entries
/// first, then unconfirmed echoes — matching transcript row order). Each tick
/// carries the opening of the assistant reply that followed it, for the hover
/// preview card.
pub fn rail_ticks(
    entries: &[SessionMessageEntry],
    echoes: &[SessionMessageEntry],
) -> Vec<RailTick> {
    let mut ticks: Vec<RailTick> = Vec::new();
    let mut reply = None;
    for entry in entries.iter().rev() {
        if entry.role == MessageRole::Assistant {
            reply = first_reply_text(std::slice::from_ref(entry));
        }
        if entry.role != MessageRole::User {
            continue;
        }
        ticks.push(RailTick {
            message_id: entry.id.clone(),
            prompt: user_text(entry),
            reply: reply.clone(),
        });
    }
    ticks.reverse();
    let mut ids: HashSet<String> = ticks.iter().map(|tick| tick.message_id.clone()).collect();
    for echo in echoes {
        if echo.role == MessageRole::User && ids.insert(echo.id.clone()) {
            ticks.push(RailTick {
                message_id: echo.id.clone(),
                prompt: user_text(echo),
                reply: None,
            });
        }
    }
    ticks
}

/// The active tick for a scroll position: the last tick whose transcript row is
/// at or above the viewport-top row (the prompt whose section you're reading).
/// Before the first tick's row, the first tick is active.
pub fn active_tick(tick_rows: &[usize], top_row: usize) -> Option<usize> {
    if tick_rows.is_empty() {
        return None;
    }
    match tick_rows.iter().rposition(|&row| row <= top_row) {
        Some(ix) => Some(ix),
        None => Some(0),
    }
}

// ---------------------------------------------------------------------------
// Fixed-footprint outline (shadcn MessageScroller "Transcript Outline")
// ---------------------------------------------------------------------------

/// One tick's hit-row height and the gap between ticks (message-rail.tsx).
pub const TICK_SLOT: f32 = 10.0;
pub const TICK_GAP: f32 = 3.0;
/// Vertical breathing room kept clear above/below the tick stack.
pub const RAIL_V_MARGIN: f32 = 24.0;

/// Hard cap on visible ticks — the always-compact shadcn outline (its demo
/// trigger holds a small fixed stack of micro-marks regardless of window
/// height), not "fill the viewport, then condense".
pub const MAX_RAIL_TICKS: usize = 12;

/// How many tick slots fit in a rail of `height` px (always ≥ 1).
pub fn rail_capacity(height: f32) -> usize {
    let usable = (height - 2.0 * RAIL_V_MARGIN).max(TICK_SLOT);
    (((usable + TICK_GAP) / (TICK_SLOT + TICK_GAP)).floor() as usize).max(1)
}

/// Slots the rail actually uses: what fits the viewport, hard-capped at
/// [`MAX_RAIL_TICKS`] so the outline stays compact even on tall windows.
pub fn rail_slots(height: f32) -> usize {
    rail_capacity(height).min(MAX_RAIL_TICKS)
}

/// shadcn's Transcript Outline keeps the always-visible rail a FIXED footprint
/// (tiny marks in a fixed-size trigger; the full turn list lives behind it) —
/// it never grows with the conversation. Same idea here: when prompts
/// outnumber the slots that fit the viewport, ticks become evenly-sized
/// BUCKETS over the conversation (a downsampled minimap) instead of
/// overflowing. Returns each bucket's `[start, end)` tick range; with
/// `n <= capacity` every bucket is a single tick — the identity, i.e. the
/// old per-prompt rail.
pub fn tick_buckets(n: usize, capacity: usize) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }
    let cap = capacity.clamp(1, n);
    (0..cap).map(|k| (k * n / cap, (k + 1) * n / cap)).collect()
}

/// The bucket containing tick `ix` (for active/hover mapping).
pub fn bucket_of(buckets: &[(usize, usize)], ix: usize) -> Option<usize> {
    buckets.iter().position(|&(s, e)| ix >= s && ix < e)
}

/// Char-cap a preview with an ellipsis. Whitespace runs (including newlines —
/// prompts and replies are free text) collapse to single spaces first: the
/// preview card's title is a one-line surface (message-rail.tsx line-clamp-1).
pub fn truncate_preview(text: &str, max_chars: usize) -> String {
    let flat = openagents_proto::view::single_line(text);
    if flat.chars().count() <= max_chars {
        return flat;
    }
    let cut: String = flat.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}
