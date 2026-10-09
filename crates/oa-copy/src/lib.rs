//! The machine-talk guard for user-facing copy (#11031).
//!
//! **Machine talk** is user-facing text that narrates the system's internals
//! instead of telling the person what happened to them or what they can do:
//! internal mechanism words (retained, projection, superseded, canonical,
//! admitted, provenance, epoch, reconcile, digest, lane, journal, ...),
//! narration of internal steps, reassurance about guarantees nobody asked
//! about, and hedged legalistic phrasing. The test: would a normal person
//! using a chat app say this sentence out loud? If not, it is machine talk.
//! Precise terms belong in docs, logs, protocols and code, never in the UI.
//!
//! Surfaces call [`violations`] on the text a person actually sees (rendered
//! HTML with tags stripped, native view labels, served help pages) in their
//! tests, and fail on any hit. A legitimate use goes in that surface's own
//! small allowlist with a reason, never here.

/// Whole words (case-insensitive) that are machine talk in user-facing copy.
pub const TERMS: &[&str] = &[
    "retained",
    "projection",
    "projections",
    "superseded",
    "canonical",
    "admitted",
    "admission",
    "qualification",
    "qualified",
    "provenance",
    "epoch",
    "epochs",
    "reconcile",
    "reconciled",
    "reconciling",
    "reconciliation",
    "digest",
    "digests",
    "journal",
    "journaled",
    "journalled",
    "idempotent",
    "materialize",
    "materialized",
    "hydrate",
    "hydrated",
    "hydration",
    "durable",
    "custody",
    "attestation",
    "attested",
    "enrollment",
    "enrolled",
    "resident",
    "grant standing",
    "revision fence",
    "original bytes",
    "lane",
    "lanes",
    "cursor",
    "dispatch",
    "dispatched",
    "redispatch",
    "intermediate",
    "upstream",
    "invariant",
    "invariants",
    "nonce",
    "principal",
];

/// Phrases (case-insensitive substrings) that are machine talk even when
/// each word alone could be fine.
pub const PHRASES: &[&str] = &[
    "outcome unknown",
    "original request",
    "remain available",
    "remains available",
    "never generates",
    "is not available yet",
    "are not available yet",
    "this page never",
    "nothing was re-derived",
    "same original",
    "fresh enrollment",
];

/// One machine-talk hit: the term or phrase, and the text around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub term: &'static str,
    pub context: String,
}

/// Every machine-talk term or phrase in `text`, skipping any term listed in
/// `allow` (a surface's own reviewed exceptions).
#[must_use]
pub fn violations(text: &str, allow: &[&str]) -> Vec<Violation> {
    let lower = text.to_lowercase();
    let mut hits = Vec::new();
    for term in TERMS.iter().filter(|t| !allow.contains(t)) {
        let mut from = 0;
        while let Some(at) = lower[from..].find(term) {
            let start = from + at;
            let end = start + term.len();
            let before = lower[..start].chars().next_back();
            let after = lower[end..].chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !word(before) && !word(after) {
                hits.push(Violation {
                    term,
                    context: context(text, start, end),
                });
            }
            from = end;
        }
    }
    for phrase in PHRASES.iter().filter(|p| !allow.contains(p)) {
        if let Some(start) = lower.find(phrase) {
            hits.push(Violation {
                term: phrase,
                context: context(text, start, start + phrase.len()),
            });
        }
    }
    hits
}

/// The visible text of an HTML document or fragment: tags, scripts, styles
/// and attribute values removed, entities left as written.
#[must_use]
pub fn visible_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        out.push(' ');
        let tail = &rest[lt..];
        let lower = tail.get(..8).unwrap_or(tail).to_ascii_lowercase();
        let skip_to = if lower.starts_with("<script") {
            "</script>"
        } else if lower.starts_with("<style") {
            "</style>"
        } else {
            ">"
        };
        match tail.to_ascii_lowercase().find(skip_to) {
            Some(i) => rest = &tail[i + skip_to.len()..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn context(text: &str, start: usize, end: usize) -> String {
    let lo = text[..start]
        .char_indices()
        .rev()
        .nth(40)
        .map_or(0, |(i, _)| i);
    let hi = text[end..]
        .char_indices()
        .nth(40)
        .map_or(text.len(), |(i, _)| end + i);
    text[lo..hi]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_sentence_is_caught() {
        let hits = violations(
            "Resumed from the retained snapshot; 1 intermediate projections were superseded. All original messages remain available.",
            &[],
        );
        let terms: Vec<_> = hits.iter().map(|h| h.term).collect();
        for t in [
            "retained",
            "projections",
            "superseded",
            "intermediate",
            "remain available",
        ] {
            assert!(terms.contains(&t), "{t} in {terms:?}");
        }
    }

    #[test]
    fn plain_copy_passes_and_words_inside_words_do_not_count() {
        assert!(violations("Start a new chat. Download OpenAgents for Mac.", &[]).is_empty());
        assert!(violations("Planes and elanes", &[]).is_empty());
    }

    #[test]
    fn allowlisted_terms_are_skipped() {
        assert!(violations("A cursor blinks", &["cursor"]).is_empty());
    }

    #[test]
    fn visible_text_drops_markup_scripts_and_attributes() {
        let t = visible_text(
            "<p class=\"retained\">Hi</p><script>retained()</script><style>.x{}</style> there",
        );
        assert!(t.contains("Hi") && t.contains("there"));
        assert!(!t.contains("retained"));
    }
}
