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
//! credential-shaped match fails `check` so a person looks at it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::contract::Text;

/// A named shape rule.
struct Rule {
    name: &'static str,
    /// Whether a match means a credential leaked (as opposed to personal
    /// data that is routine to redact, such as a home directory).
    credential: bool,
    pattern: Regex,
    replacement: &'static str,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let rule = |name, credential, pattern: &str, replacement| Rule {
            name,
            credential,
            pattern: Regex::new(pattern).unwrap_or_else(|e| panic!("rule {name}: {e}")),
            replacement,
        };
        vec![
            rule(
                "private-key",
                true,
                r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)",
                "[redacted:private-key]",
            ),
            rule(
                "anthropic-key",
                true,
                r"sk-ant-[A-Za-z0-9_\-]{16,}",
                "[redacted:anthropic-key]",
            ),
            rule(
                "openai-key",
                true,
                r"\bsk-(proj-|svcacct-)?[A-Za-z0-9_\-]{20,}",
                "[redacted:openai-key]",
            ),
            rule(
                "openagents-key",
                true,
                r"\b(oak_[A-Za-z0-9]+\.[A-Za-z0-9_\-]{8,}|oa_agent_[A-Za-z0-9_\-]{12,})",
                "[redacted:openagents-key]",
            ),
            rule(
                "github-token",
                true,
                r"\b(gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})",
                "[redacted:github-token]",
            ),
            rule(
                "slack-token",
                true,
                r"\bxox[abprs]-[A-Za-z0-9\-]{10,}",
                "[redacted:slack-token]",
            ),
            rule(
                "aws-key",
                true,
                r"\b(AKIA|ASIA)[0-9A-Z]{16}\b",
                "[redacted:aws-key]",
            ),
            rule(
                "google-key",
                true,
                r"\bAIza[0-9A-Za-z_\-]{35}",
                "[redacted:google-key]",
            ),
            rule(
                "nostr-secret",
                true,
                r"\bnsec1[02-9ac-hj-np-z]{58}\b",
                "[redacted:nostr-secret]",
            ),
            rule(
                "jwt",
                true,
                r"\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
                "[redacted:jwt]",
            ),
            rule(
                "bearer",
                true,
                r"(?i)\bbearer\s+[A-Za-z0-9._\-~+/]{20,}=*",
                "Bearer [redacted:bearer]",
            ),
            rule(
                "home-directory",
                false,
                r"/(Users|home)/[A-Za-z0-9._\-]+",
                "/$1/[redacted]",
            ),
            rule(
                "email",
                false,
                r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
                "[redacted:email]",
            ),
        ]
    })
}

/// Rule names whose match means a credential leaked.
#[must_use]
pub fn credential_rules() -> Vec<&'static str> {
    rules()
        .iter()
        .filter(|r| r.credential)
        .map(|r| r.name)
        .collect()
}

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
        let mut out = text.to_owned();
        for rule in rules() {
            let count = rule.pattern.find_iter(&out).count();
            if count == 0 {
                continue;
            }
            // `$1` in a replacement refers to the rule's first group.
            out = rule
                .pattern
                .replace_all(&out, rule.replacement)
                .into_owned();
            *self.redactions.entry(rule.name.to_owned()).or_default() +=
                u32::try_from(count).unwrap_or(u32::MAX);
        }
        out
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

/// The first and last parts of `text`, within `bound` bytes in all, split
/// on character boundaries, with a marker between them.
#[must_use]
pub fn head_and_tail(text: &str, bound: usize) -> String {
    const MARK: &str = "\n[… cut …]\n";
    if text.len() <= bound {
        return text.to_owned();
    }
    let room = bound.saturating_sub(MARK.len());
    let head = floor_boundary(text, room * 2 / 3);
    let tail_len = room - head;
    let tail_start = ceil_boundary(text, text.len() - tail_len);
    format!("{}{MARK}{}", &text[..head], &text[tail_start..])
}

fn floor_boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn ceil_boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
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
