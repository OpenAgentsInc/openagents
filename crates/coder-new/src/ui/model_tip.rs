//! A click on a transcript message shows, for a few seconds, the model that
//! produced it at the right end of the clicked row, in the dim gray the
//! transcript uses for secondary text: "Codex · GPT-6.1-Sol". It is drawn
//! over the frame only; the transcript, its copies, sync and export never
//! carry it.

use std::time::{Duration, Instant};

use ratatui::{buffer::Buffer, layout::Rect, style::Color, style::Style};
use unicode_width::UnicodeWidthStr;

use crate::live::Entry;
use crate::theme as t;

/// How long the model shows after a click.
pub(crate) const SHOWN_FOR: Duration = Duration::from_secs(3);
/// The last part of [`SHOWN_FOR`], where the label fades into the background.
const FADE: Duration = Duration::from_millis(600);

/// The model shown beside a clicked message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelTip {
    /// The transcript row clicked.
    pub row: usize,
    /// What shows: the harness and the model's name.
    pub label: String,
    pub shown_at: Instant,
}

impl ModelTip {
    pub(crate) fn visible(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.shown_at) < SHOWN_FOR
    }

    /// The label's color at `now`: the dim gray, fading toward the
    /// background over the last [`FADE`].
    fn color(&self, now: Instant) -> Color {
        let left = SHOWN_FOR.saturating_sub(now.saturating_duration_since(self.shown_at));
        if left >= FADE {
            return t::GRAY_DIM;
        }
        let (Color::Rgb(r1, g1, b1), Color::Rgb(r0, g0, b0)) = (t::GRAY_DIM, t::BG_BASE) else {
            return t::GRAY_DIM;
        };
        let keep = left.as_secs_f32() / FADE.as_secs_f32();
        let mix =
            |from: u8, to: u8| (f32::from(to) + (f32::from(from) - f32::from(to)) * keep) as u8;
        Color::Rgb(mix(r1, r0), mix(g1, g0), mix(b1, b0))
    }
}

/// The model behind the entry at `index`: an agent reply's own model, else
/// the model that answered its turn (the person's message that opened it,
/// its thinking and tool rows): the turn's first reply after the entry
/// that names one, else an earlier one in the same turn. `None` while the
/// turn has no reply that names its model.
pub(crate) fn turn_model<'a>(entries: &[&'a Entry], index: usize) -> Option<&'a str> {
    let model = |entry: &&'a Entry| match entry {
        Entry::Assistant {
            model: Some(model), ..
        } if !model.is_empty() => Some(model.as_str()),
        _ => None,
    };
    let entry = entries.get(index)?;
    if let Some(own) = model(entry) {
        return Some(own);
    }
    let start = entries[..=index]
        .iter()
        .rposition(|entry| matches!(entry, Entry::User(_)))
        .unwrap_or(0);
    let end = entries[index + 1..]
        .iter()
        .position(|entry| matches!(entry, Entry::User(_)))
        .map_or(entries.len(), |at| index + 1 + at);
    entries[index + 1..end]
        .iter()
        .find_map(model)
        .or_else(|| entries[start..index].iter().rev().find_map(model))
}

/// The harness and model as the rail names them: "Codex · GPT-6.1-Sol",
/// "Claude · Opus 5.5", "Coder · Gemini 3.8 Flash".
pub(crate) fn label(model: &str) -> String {
    let model = model.trim();
    // Codex's own login names its turns `codex:<id>`; settings ride after
    // the next colon (`:high`).
    let (codex_login, slug) = match model.strip_prefix("codex:") {
        Some(rest) => (true, rest),
        None => (false, model),
    };
    let base = slug.split(':').next().unwrap_or(slug);
    if codex_login || crate::codex::offered(base) {
        return format!("Codex · {}", crate::codex::name(base));
    }
    if !base.contains('/')
        && (base.starts_with("claude-")
            || crate::claude::MODELS.iter().any(|(id, _, _)| *id == base))
    {
        let known = crate::claude::MODELS.iter().any(|(id, _, _)| *id == base);
        let name = if known {
            crate::claude::name(base)
        } else {
            super::rail_model_name(base)
        };
        return format!("Claude · {name}");
    }
    if let Some(rest) = base.strip_prefix("opencode/") {
        return format!("OpenCode · {}", super::rail_model_name(rest));
    }
    format!("Coder · {}", super::rail_model_name(model))
}

/// Draw `tip` at the right end of its row when that row is on screen.
pub(crate) fn paint(buf: &mut Buffer, area: Rect, first: usize, tip: &ModelTip, now: Instant) {
    if !tip.visible(now) || tip.row < first {
        return;
    }
    let offset = tip.row - first;
    if offset >= usize::from(area.height) {
        return;
    }
    let text = format!(" {} ", tip.label);
    let width = text.width() as u16;
    // One column stays free at the edge, where the scroll arrow sits.
    if width + 1 > area.width {
        return;
    }
    let x = area.right() - 1 - width;
    let y = area.y + offset as u16;
    buf.set_string(x, y, text, Style::default().fg(tip.color(now)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(model: Option<&str>) -> Entry {
        Entry::Assistant {
            text: "Done.".into(),
            model: model.map(str::to_owned),
            elapsed_ms: None,
        }
    }

    #[test]
    fn labels_name_the_harness_and_the_model() {
        assert_eq!(label("codex:gpt-6.1-sol:high"), "Codex · GPT-6.1-Sol");
        assert_eq!(label("gpt-6.1-sol"), "Codex · GPT-6.1-Sol");
        assert_eq!(label("claude-opus-5-5"), "Claude · Opus 5.5");
        assert_eq!(label("google/gemini-3.8-flash"), "Coder · Gemini 3.8 Flash");
    }

    #[test]
    fn a_message_takes_its_turns_model() {
        let entries = [
            Entry::User("first".into()),
            reply(Some("google/gemini-3.8-flash")),
            Entry::User("second".into()),
            Entry::Tool {
                name: "Read".into(),
                input: serde_json::json!({}),
                output: serde_json::json!({}),
                running: false,
            },
            reply(Some("claude-opus-5-5")),
            Entry::User("third".into()),
        ];
        let entries: Vec<&Entry> = entries.iter().collect();
        assert_eq!(turn_model(&entries, 0), Some("google/gemini-3.8-flash"));
        assert_eq!(turn_model(&entries, 1), Some("google/gemini-3.8-flash"));
        assert_eq!(turn_model(&entries, 2), Some("claude-opus-5-5"));
        assert_eq!(turn_model(&entries, 3), Some("claude-opus-5-5"));
        // No reply yet: nothing to show.
        assert_eq!(turn_model(&entries, 5), None);
    }

    #[test]
    fn the_label_is_gone_after_the_timeout() {
        let shown_at = Instant::now();
        let tip = ModelTip {
            row: 1,
            label: "Codex · GPT-6.1-Sol".into(),
            shown_at,
        };
        let area = Rect::new(0, 0, 40, 4);
        let text =
            |buf: &Buffer| -> String { (0..area.width).map(|x| buf[(x, 1)].symbol()).collect() };
        let mut buf = Buffer::empty(area);
        paint(&mut buf, area, 0, &tip, shown_at + Duration::from_secs(1));
        assert!(text(&buf).trim_end().ends_with("Codex · GPT-6.1-Sol"));
        let at = text(&buf).find("Codex").unwrap();
        let column = text(&buf)[..at].chars().count() as u16;
        assert_eq!(buf[(column, 1)].fg, t::GRAY_DIM);
        // Fading in the last moment, then gone.
        let mut buf = Buffer::empty(area);
        paint(
            &mut buf,
            area,
            0,
            &tip,
            shown_at + Duration::from_millis(2_800),
        );
        assert_ne!(buf[(column, 1)].fg, t::GRAY_DIM);
        let mut buf = Buffer::empty(area);
        paint(&mut buf, area, 0, &tip, shown_at + SHOWN_FOR);
        assert!(text(&buf).trim().is_empty());
    }
}
