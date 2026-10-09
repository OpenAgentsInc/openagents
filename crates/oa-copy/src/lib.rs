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

/// A string literal in Rust source, with the 1-based line it starts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceString {
    pub line: usize,
    pub text: String,
}

/// Macros and calls whose string arguments are logs, panics or developer
/// errors, never copy a person sees.
const NOT_COPY_CALLS: &[&str] = &[
    "debug!(",
    "info!(",
    "warn!(",
    "error!(",
    "trace!(",
    "eprintln!(",
    "println!(",
    "panic!(",
    "unreachable!(",
    "todo!(",
    "unimplemented!(",
    "assert!(",
    "assert_eq!(",
    "assert_ne!(",
    "debug_assert!(",
    "expect(",
    "expect_err(",
    "bail!(",
    "anyhow!(",
    "context(",
];

/// The string literals in Rust `src` that could be copy: comments, items
/// under `#[cfg(test)]`, attribute arguments, and arguments to logging,
/// panic and assertion macros are skipped. Raw and byte strings count;
/// escapes stay as written.
#[must_use]
pub fn source_strings(src: &str) -> Vec<SourceString> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut depth = 0usize;
    // Pending `#[cfg(test)]`: skip the next item, either up to `;` or across
    // its braces.
    let mut pending_test = false;
    let mut skip_until_depth: Option<usize> = None;
    let mut attr_depth: Option<usize> = None;
    let mut bracket = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut nest = 1;
            i += 2;
            while i < b.len() && nest > 0 {
                if b[i] == b'\n' {
                    line += 1;
                }
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    nest += 1;
                    i += 1;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    nest -= 1;
                    i += 1;
                }
                i += 1;
            }
            continue;
        }
        // Raw strings: r"..", r#".."#, br#".."#.
        let raw_start = if c == b'r' {
            Some(i + 1)
        } else if c == b'b' && b.get(i + 1) == Some(&b'r') {
            Some(i + 2)
        } else {
            None
        };
        let ident_before = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if let (Some(mut j), false) = (raw_start, ident_before) {
            let mut hashes = 0;
            while b.get(j) == Some(&b'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) == Some(&b'"') {
                let start = j + 1;
                let mut k = start;
                let close = loop {
                    if k >= b.len() {
                        break b.len();
                    }
                    if b[k] == b'"'
                        && b[k + 1..]
                            .iter()
                            .take(hashes)
                            .filter(|&&h| h == b'#')
                            .count()
                            == hashes
                    {
                        break k;
                    }
                    k += 1;
                };
                let text = &src[start..close.min(src.len())];
                let at = line;
                line += text.matches('\n').count();
                if keep(src, i, pending_test, skip_until_depth, attr_depth) {
                    out.push(SourceString {
                        line: at,
                        text: text.to_owned(),
                    });
                }
                i = (close + 1 + hashes).min(b.len());
                continue;
            }
        }
        if c == b'"' {
            let start = i + 1;
            let mut k = start;
            while k < b.len() && b[k] != b'"' {
                if b[k] == b'\\' {
                    k += 1;
                }
                k += 1;
            }
            let close = k.min(b.len());
            let text = &src[start..close];
            let at = line;
            line += text.matches('\n').count();
            let lit_start = if i > 0 && b[i - 1] == b'b' { i - 1 } else { i };
            if keep(src, lit_start, pending_test, skip_until_depth, attr_depth) {
                out.push(SourceString {
                    line: at,
                    text: text.to_owned(),
                });
            }
            i = close + 1;
            continue;
        }
        if c == b'\'' {
            // A char literal ('x', '\n', '\u{..}') or a lifetime ('a).
            if b.get(i + 1) == Some(&b'\\') {
                let mut k = i + 2;
                while k < b.len() && b[k] != b'\'' {
                    k += 1;
                }
                i = k + 1;
                continue;
            }
            let ch_len = src[i + 1..].chars().next().map_or(1, char::len_utf8);
            if b.get(i + 1 + ch_len) == Some(&b'\'') {
                i += 2 + ch_len;
                continue;
            }
            i += 1;
            continue;
        }
        if src[i..].starts_with("#[cfg(test)]") {
            pending_test = true;
            i += "#[cfg(test)]".len();
            continue;
        }
        if c == b'#'
            && (b.get(i + 1) == Some(&b'[')
                || (b.get(i + 1) == Some(&b'!') && b.get(i + 2) == Some(&b'[')))
        {
            if attr_depth.is_none() {
                attr_depth = Some(bracket);
            }
            i += 1;
            continue;
        }
        match c {
            b'[' => bracket += 1,
            b']' => {
                bracket = bracket.saturating_sub(1);
                if attr_depth == Some(bracket) {
                    attr_depth = None;
                }
            }
            b'{' => {
                if pending_test && skip_until_depth.is_none() && attr_depth.is_none() {
                    skip_until_depth = Some(depth);
                    pending_test = false;
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                if skip_until_depth == Some(depth) {
                    skip_until_depth = None;
                }
            }
            b';' if pending_test && skip_until_depth.is_none() && attr_depth.is_none() => {
                pending_test = false;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn keep(
    src: &str,
    at: usize,
    pending_test: bool,
    skip_until_depth: Option<usize>,
    attr_depth: Option<usize>,
) -> bool {
    if pending_test || skip_until_depth.is_some() || attr_depth.is_some() {
        return false;
    }
    let before = src[..at].trim_end();
    let before = before
        .get(before.len().saturating_sub(48)..)
        .unwrap_or(before);
    !NOT_COPY_CALLS.iter().any(|call| before.ends_with(call))
}

/// Whether a source string reads like words a person could see: it has a
/// space between letters, or it is a single capitalized word ("Retained").
/// Lowercase single tokens are wire names, keys and identifiers.
#[must_use]
pub fn looks_like_copy(text: &str) -> bool {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 3 {
        return false;
    }
    if text.split_whitespace().nth(1).is_some() {
        return true;
    }
    let mut chars = text.chars();
    chars.next().is_some_and(char::is_uppercase) && chars.all(char::is_alphabetic)
}

/// Every machine-talk hit in the copy-like string literals of Rust `src`,
/// with the line each literal starts on.
#[must_use]
pub fn source_violations(src: &str, allow: &[&str]) -> Vec<(usize, Violation)> {
    source_strings(src)
        .into_iter()
        .filter(|s| looks_like_copy(&s.text))
        .flat_map(|s| {
            violations(&unescape(&s.text), allow)
                .into_iter()
                .map(move |v| (s.line, v))
        })
        .collect()
}

/// A literal's escapes as the reader sees them, so a term right after `\n`
/// still starts a word: whitespace escapes and line continuations become a
/// space, and any other escaped character stands for itself.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 't' | 'r' | '\n') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Scan every `.rs` file under `dir` (skipping `tests.rs`, `*_tests.rs`,
/// `tests/` directories, and any file whose path relative to `dir` starts
/// with an entry of `skip`) and report each hit as `path:line: term in
/// context`. A surface's guard test calls this on its sources and fails on
/// any line it returns. `skip` is for files that hold no user-facing copy
/// (scenario drivers, protocol tables); give each entry a reason.
#[must_use]
pub fn scan_dir(dir: &std::path::Path, skip: &[&str], allow: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files);
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let rel = file.strip_prefix(dir).unwrap_or(&file).to_string_lossy();
        if skip.iter().any(|s| rel.starts_with(s)) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (line, v) in source_violations(&src, allow) {
            out.push(format!(
                "{}:{line}: {:?} in {:?}",
                file.display(),
                v.term,
                v.context
            ));
        }
    }
    out
}

fn collect_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name != "tests" {
                collect_rs(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(path);
        }
    }
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
    fn source_strings_skip_comments_tests_attributes_and_logs() {
        let src = r##"
// "retained in a comment"
/* "retained in a block" */
#[serde(rename = "retained value")]
fn view() -> &'static str {
    let c = '"';
    tracing::debug!("the journal was retained");
    let raw = r#"Raw "quoted" retained copy"#;
    "The conversation is retained"
}
#[cfg(test)]
mod tests {
    fn t() { let s = "retained in a test"; }
}
fn after() -> &'static str { "Still scanned after tests" }
"##;
        let texts: Vec<_> = source_strings(src).into_iter().map(|s| s.text).collect();
        assert_eq!(
            texts,
            [
                "Raw \"quoted\" retained copy",
                "The conversation is retained",
                "Still scanned after tests"
            ]
        );
        let hits = source_violations(src, &[]);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].0, 9);
    }

    #[test]
    fn terms_after_escapes_are_still_words() {
        let src = r#"fn f() -> &'static str { "Origin\nProvenance: {}\tjournal" }"#;
        let terms: Vec<_> = source_violations(src, &[])
            .into_iter()
            .map(|(_, v)| v.term)
            .collect();
        assert_eq!(terms, ["provenance", "journal"]);
    }

    #[test]
    fn copy_heuristic_skips_wire_names() {
        assert!(looks_like_copy("Archive this chat"));
        assert!(looks_like_copy("Retained"));
        assert!(!looks_like_copy("work.dispatch"));
        assert!(!looks_like_copy("dispatch"));
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
