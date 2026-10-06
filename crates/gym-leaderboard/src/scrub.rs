//! Scrubbing and bounding text before it leaves the repository as a
//! published trace.
//!
//! The retained traces already passed `tbench retain`'s credential scan,
//! which compares every file against the exact credential values on the
//! host that ran them. That scan can't run anywhere else, so this is the
//! second, host-independent layer: shape rules for credentials and
//! personal data, applied to every string a bundle carries. A match is
//! replaced with `[redacted:<rule>]` and counted in the bundle's
//! [`ScrubReport`](crate::contract::ScrubReport); a publication with any
//! credential-shaped match fails `check` so a person looks at it. The rules
//! live in the shared `secret-screen` crate.

use std::collections::BTreeMap;

use crate::contract::Text;

/// The shape rules, shared with the workshop agent's secret screen.
pub use secret_screen::{credential_rules, head_and_tail};

/// Counts redactions and truncations across one bundle.
#[derive(Debug, Default)]
pub struct Scrubber {
    pub redactions: BTreeMap<String, u32>,
    pub truncated: u32,
    /// The per-field bound in bytes.
    pub bound: usize,
}

impl Scrubber {
    #[must_use]
    pub fn new(bound: usize) -> Self {
        Self {
            bound,
            ..Self::default()
        }
    }

    /// Redacts every rule's matches.
    pub fn redact(&mut self, text: &str) -> String {
        secret_screen::redact_counted(text, &mut self.redactions)
    }

    /// Redacts, then bounds to the scrubber's field bound.
    pub fn text(&mut self, text: &str) -> Text {
        let bound = self.bound;
        self.text_within(text, bound)
    }

    /// Redacts, then bounds to `bound` bytes, keeping the head and the
    /// tail (the tail of a command's output is usually its verdict).
    pub fn text_within(&mut self, text: &str, bound: usize) -> Text {
        let clean = self.redact(text);
        if clean.len() <= bound {
            return Text {
                text: clean,
                original_bytes: None,
            };
        }
        self.truncated += 1;
        Text {
            text: head_and_tail(&clean, bound),
            original_bytes: Some(clean.len() as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_shapes_are_redacted_and_counted() {
        let mut s = Scrubber::new(10_000);
        // Assembled at run time so no credential-shaped literal sits in
        // the source.
        let anthropic = format!("sk-ant-{}", "a1".repeat(20));
        let github = format!("ghp_{}", "Z9".repeat(18));
        let nsec = format!("nsec1{}", "q".repeat(58));
        let bearer = format!("Authorization: Bearer {}", "t0k".repeat(10));
        let aws = format!("AKIA{}", "ABCDEFGH23456789");
        let input = format!("{anthropic} {github} {nsec} {bearer} {aws}");
        let out = s.redact(&input);
        for secret in [&anthropic, &github, &nsec, &aws] {
            assert!(!out.contains(secret.as_str()), "{out}");
        }
        assert!(!out.contains(&"t0k".repeat(10)));
        for rule in [
            "anthropic-key",
            "github-token",
            "nostr-secret",
            "bearer",
            "aws-key",
        ] {
            assert_eq!(s.redactions.get(rule), Some(&1), "{rule}: {out}");
        }
    }

    #[test]
    fn personal_paths_and_emails_are_redacted_but_task_paths_stay() {
        let mut s = Scrubber::new(10_000);
        let out = s.redact("cd /Users/someone/work && cat /app/Main.v; mail a.b@example.org");
        assert_eq!(
            out,
            "cd /Users/[redacted]/work && cat /app/Main.v; mail [redacted:email]"
        );
        assert!(!credential_rules().contains(&"home-directory"));
    }

    #[test]
    fn ordinary_text_is_unchanged() {
        let mut s = Scrubber::new(10_000);
        let text =
            "sha256 f63a17e1b40e723d9495cdff4a2629c753248c63da96c25e8192f0381a66f782 task-sk-model";
        assert_eq!(s.redact(text), text);
        assert!(s.redactions.is_empty());
    }

    #[test]
    fn a_long_field_keeps_its_head_and_tail_within_the_bound() {
        let mut s = Scrubber::new(64);
        let text = format!("start{}é{}end", "x".repeat(200), "y".repeat(200));
        let t = s.text(&text);
        assert!(t.text.len() <= 64, "{}", t.text.len());
        assert!(t.text.starts_with("start"));
        assert!(t.text.ends_with("end"));
        assert_eq!(t.original_bytes, Some(text.len() as u64));
        assert_eq!(s.truncated, 1);
        let short = s.text("short");
        assert_eq!(short.original_bytes, None);
    }
}
