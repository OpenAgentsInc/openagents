//! Harness-internal context tags, kept out of agent text (#11974).
//!
//! Claude Code and Codex inject context into the model's input as bracketed
//! tags, such as `<system-reminder>` and `<total_tokens>`. A model can echo
//! one back as its own text, and a person must never see that text in a
//! transcript. The drivers run their agent text through [`TagFilter`] before
//! it becomes an `AgentEvent`.
//!
//! The filter is narrow on purpose, so that legitimate content survives:
//!
//! - Only the tags in [`INTERNAL_TAGS`] are removed.
//! - Only a whole span is removed: the opening tag, its content, and the
//!   matching closing tag. An unclosed tag is kept.
//! - The span must stand on lines of its own, the way harnesses inject it.
//!   A mention inside a sentence, such as "the `<total_tokens>` line", stays.
//!
//! Text arrives in token-sized deltas, so the filter holds back only a line
//! that starts with `<` and could still become an internal tag. Every other
//! character passes through at once.

/// Tags that harnesses inject into the model's context. Extend this list
/// when a harness adds a new one.
pub(crate) const INTERNAL_TAGS: &[&str] = &[
    // Claude Code.
    "system-reminder",
    "total_tokens",
    "budget:token_budget",
    "local-command-caveat",
    "user-prompt-submit-hook",
    // Codex.
    "environment_context",
    "user_instructions",
    "turn_aborted",
];

/// Upper bound on held text while waiting for a closing tag. Past it, the
/// opening tag is treated as ordinary text, so a stray tag can't swallow a
/// whole reply.
const HOLD_CAP: usize = 64 * 1024;

/// Removes whole internal-tag spans from a stream of text deltas.
#[derive(Debug)]
pub(crate) struct TagFilter {
    held: String,
    line_start: bool,
}

impl Default for TagFilter {
    fn default() -> Self {
        Self {
            held: String::new(),
            line_start: true,
        }
    }
}

enum Scan {
    /// A whole span of this many bytes, with its trailing line break.
    Span(usize),
    /// The text could still become an internal span.
    NeedMore,
    /// Ordinary text.
    Text,
}

impl TagFilter {
    /// Feed one delta. Returns the text that is safe to show now.
    pub(crate) fn push(&mut self, text: &str) -> String {
        self.held.push_str(text);
        self.drain(false)
    }

    /// End of the message: release whatever is held, minus a whole span.
    pub(crate) fn finish(&mut self) -> String {
        let out = self.drain(true);
        self.line_start = true;
        out
    }

    fn drain(&mut self, end: bool) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < self.held.len() {
            let rest = &self.held[i..];
            if !self.line_start {
                match rest.find('\n') {
                    Some(n) => {
                        out.push_str(&rest[..=n]);
                        i += n + 1;
                        self.line_start = true;
                    }
                    None => {
                        out.push_str(rest);
                        i = self.held.len();
                    }
                }
                continue;
            }
            let body = rest.trim_start_matches([' ', '\t']);
            let indent = rest.len() - body.len();
            if !body.starts_with('<') {
                // Indentation alone keeps the line start; anything else is
                // ordinary text.
                out.push_str(&rest[..indent]);
                i += indent;
                if !body.is_empty() {
                    self.line_start = false;
                }
                continue;
            }
            match scan(body, end) {
                Scan::Span(len) => i += indent + len,
                Scan::NeedMore => break,
                Scan::Text => self.line_start = false,
            }
        }
        self.held.drain(..i);
        out
    }
}

/// Classify `s`, which starts with `<` at the start of a line.
fn scan(s: &str, end: bool) -> Scan {
    let mut verdict = Scan::Text;
    for tag in INTERNAL_TAGS {
        let open = format!("<{tag}");
        if s.len() < open.len() {
            if !end && open.starts_with(s) {
                verdict = Scan::NeedMore;
            }
            continue;
        }
        if !s.starts_with(&open) {
            continue;
        }
        match s[open.len()..].chars().next() {
            None if !end => return Scan::NeedMore,
            Some(c) if c == '>' || c.is_whitespace() => {}
            _ => continue,
        }
        let close = format!("</{tag}>");
        let Some(at) = s.find(&close) else {
            return if end || s.len() > HOLD_CAP {
                Scan::Text
            } else {
                Scan::NeedMore
            };
        };
        let stop = at + close.len();
        let tail = &s[stop..];
        let after = tail.trim_start_matches([' ', '\t', '\r']);
        return if after.starts_with('\n') {
            Scan::Span(stop + (tail.len() - after.len()) + 1)
        } else if after.is_empty() {
            if end {
                Scan::Span(s.len())
            } else {
                Scan::NeedMore
            }
        } else {
            Scan::Text
        };
    }
    verdict
}

/// Remove whole internal-tag spans from a complete text.
pub(crate) fn strip(text: &str) -> String {
    let mut filter = TagFilter::default();
    let mut out = filter.push(text);
    out.push_str(&filter.finish());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn streamed(chunks: &[&str]) -> String {
        let mut filter = TagFilter::default();
        let mut out: String = chunks.iter().map(|c| filter.push(c)).collect();
        out.push_str(&filter.finish());
        out
    }

    #[test]
    fn a_whole_span_is_removed() {
        assert_eq!(
            strip("<total_tokens>14998803 tokens left</total_tokens>"),
            ""
        );
        assert_eq!(
            strip("<system-reminder>\nnote\n</system-reminder>\nHello"),
            "Hello"
        );
        assert_eq!(
            strip("Done.\n<total_tokens>9 tokens left</total_tokens>\nNext."),
            "Done.\nNext."
        );
    }

    #[test]
    fn a_span_split_across_deltas_is_removed() {
        let chunks = [
            "<",
            "total",
            "_tokens>149",
            "98803 tokens left</total",
            "_tokens>",
        ];
        assert_eq!(streamed(&chunks), "");
        let chunks = ["Hi.\n", "<system-", "reminder>x</system-reminder>", "\nBye"];
        assert_eq!(streamed(&chunks), "Hi.\nBye");
    }

    #[test]
    fn legitimate_text_survives() {
        for text in [
            "the `<total_tokens>…</total_tokens>` line leaked",
            "<total_tokens> without a close",
            "<total_tokens>x</total_tokens> and more on the line",
            "<div>html</div>",
            "<system-reminders>not ours</system-reminders>",
            "a < b and c > d",
            "  indented text",
        ] {
            assert_eq!(strip(text), text, "{text}");
            let chars: Vec<String> = text.chars().map(String::from).collect();
            let chunks: Vec<&str> = chars.iter().map(String::as_str).collect();
            assert_eq!(streamed(&chunks), text, "{text} streamed");
        }
    }

    #[test]
    fn ordinary_text_is_not_held() {
        let mut filter = TagFilter::default();
        assert_eq!(filter.push("Hello"), "Hello");
        assert_eq!(filter.push(" world\n"), " world\n");
        assert_eq!(filter.push("<"), "");
        assert_eq!(filter.push("b>bold"), "<b>bold");
    }
}
