//! The secret screen: credential-shape rules shared by every place that
//! keeps or publishes text a model or a person wrote.
//!
//! The rules moved here from the Gym leaderboard's trace scrubber
//! (`gym-leaderboard::scrub`), which still uses them for published traces.
//! The workshop agent's memory, journal, and reports use them as a screen
//! (`docs/verse/workshop-agent.md`, "Memory"): [`Screen::check`] refuses
//! text that matches a credential shape, or that holds the exact value of
//! a credential on this host, as `tbench retain`'s scan compares.
//! [`redact`] replaces each match with `[redacted:<rule>]` instead.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

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
            // Claude Code's subscription token variable carries a
            // claude.ai login whatever the value's shape
            // (`docs/cloud/claude-code-byo.md`, rule 3).
            rule(
                "claude-oauth-env",
                true,
                r#"CLAUDE_CODE_OAUTH_TOKEN(["']?\s*[=:]\s*["']?)[^\s"',;}]+"#,
                "CLAUDE_CODE_OAUTH_TOKEN${1}[redacted:claude-oauth-env]",
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

/// The first credential rule `text` matches, by name.
#[must_use]
pub fn credential_in(text: &str) -> Option<&'static str> {
    rules()
        .iter()
        .filter(|r| r.credential)
        .find(|r| r.pattern.is_match(text))
        .map(|r| r.name)
}

/// `text` with every rule's matches redacted, counting each rule's
/// matches in `counts`.
pub fn redact_counted(text: &str, counts: &mut BTreeMap<String, u32>) -> String {
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
        *counts.entry(rule.name.to_owned()).or_default() +=
            u32::try_from(count).unwrap_or(u32::MAX);
    }
    out
}

/// `text` with every credential rule's matches redacted; personal data,
/// such as a home directory, stays.
#[must_use]
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    for rule in rules().iter().filter(|r| r.credential) {
        if rule.pattern.is_match(&out) {
            out = rule
                .pattern
                .replace_all(&out, rule.replacement)
                .into_owned();
        }
    }
    out
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

/// Why the screen refused text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It matches the credential rule with this name.
    Shape(&'static str),
    /// It holds the exact value of a credential on this host.
    Exact,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(rule) => write!(f, "it looks like a credential ({rule})"),
            Self::Exact => f.write_str("it holds a credential this computer keeps"),
        }
    }
}

/// The environment variable that carries a claude.ai subscription token.
pub const CLAUDE_CODE_OAUTH_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// Whether `text` holds a Claude.ai login: an OAuth access or refresh
/// token (including a `claude setup-token` value), Claude Code's
/// credentials document, or a `CLAUDE_CODE_OAUTH_TOKEN` assignment.
/// OpenAgents never collects, stores, or forwards one
/// (`docs/cloud/claude-code-byo.md`, rule 2). An Anthropic API key is a
/// different custody class and does not match.
#[must_use]
pub fn claude_login_in(text: &str) -> bool {
    static LOGIN: OnceLock<Regex> = OnceLock::new();
    LOGIN
        .get_or_init(|| {
            Regex::new(r"sk-ant-o[ar]t[0-9]*-|claudeAiOauth|CLAUDE_CODE_OAUTH_TOKEN\s*[=:]")
                .unwrap_or_else(|e| panic!("claude login: {e}"))
        })
        .is_match(text)
}

/// Whether a relative or absolute path names Claude Code's login file
/// (`~/.claude/.credentials.json`, or `.credentials.json` under
/// `$CLAUDE_CONFIG_DIR`). Evidence, export, and saved images exclude it.
#[must_use]
pub fn claude_login_path(path: &str) -> bool {
    path.split(['/', '\\'])
        .any(|part| part == ".credentials.json")
}

/// Engine login files under a home directory: Claude Code's and Codex's.
/// No evidence, export, or image carries them, and every value in them is
/// redacted wherever it appears (`coder_environment::evidence::Redactor`).
pub const ENGINE_LOGIN_FILES: &[&str] = &[".claude/.credentials.json", ".codex/auth.json"];

/// Environment variables that carry engine logins or keys.
pub const ENGINE_LOGIN_ENV: &[&str] = &[
    CLAUDE_CODE_OAUTH_TOKEN,
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
];

/// Whether a relative or absolute path names an engine login file:
/// Claude Code's `.credentials.json` anywhere, or Codex's `auth.json`
/// under `.codex`.
#[must_use]
pub fn engine_login_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    claude_login_path(path)
        || parts
            .windows(2)
            .any(|pair| pair[0] == ".codex" && pair[1] == "auth.json")
}

/// The credential values inside a login or credential document: every
/// string under a key that names a token, secret, key, or password (for
/// example `accessToken`, `refresh_token`, `OPENAI_API_KEY`), at any
/// depth. Text that is not JSON has none.
#[must_use]
pub fn credential_fragments(document: &str) -> Vec<String> {
    fn walk(value: &serde_json::Value, named: bool, found: &mut Vec<String>) {
        match value {
            serde_json::Value::String(s) if named => found.push(s.clone()),
            serde_json::Value::Object(fields) => {
                for (name, value) in fields {
                    let name = name.to_ascii_lowercase();
                    let named = ["token", "secret", "key", "password"]
                        .iter()
                        .any(|w| name.contains(w));
                    walk(value, named, found);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    walk(value, named, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    if let Ok(document) = serde_json::from_str::<serde_json::Value>(document) {
        walk(&document, false, &mut found);
    }
    found
}

/// The shortest credential value the exact comparison checks: shorter
/// values match ordinary words.
pub const EXACT_MIN: usize = 12;

/// The secret screen: the shape rules and the exact credential values on
/// this host. It never prints or keeps a value outside this process.
#[derive(Clone, Default)]
pub struct Screen {
    exact: Vec<String>,
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Screen({} exact values)", self.exact.len())
    }
}

impl Screen {
    /// A screen with the shape rules only.
    #[must_use]
    pub fn shapes() -> Self {
        Self::default()
    }

    /// A screen with the shape rules and the exact values of this
    /// process's environment variables whose names say they hold a
    /// credential (`*KEY*`, `*TOKEN*`, `*SECRET*`, `*PASSWORD*`), at least
    /// [`EXACT_MIN`] bytes long.
    #[must_use]
    pub fn host() -> Self {
        let mut screen = Self::default();
        for (name, value) in std::env::vars() {
            let upper = name.to_ascii_uppercase();
            let named = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD"]
                .iter()
                .any(|word| upper.contains(word));
            if named {
                screen.add(value);
            }
        }
        screen
    }

    /// Adds an exact value to refuse, when it is long enough to compare.
    pub fn add(&mut self, value: impl Into<String>) {
        let value = value.into();
        let value = value.trim();
        if value.len() >= EXACT_MIN && !self.exact.iter().any(|v| v == value) {
            self.exact.push(value.to_owned());
        }
    }

    /// Refuses `text` when it matches a credential shape or holds an
    /// exact credential value.
    ///
    /// # Errors
    /// Says which.
    pub fn check(&self, text: &str) -> Result<(), Refusal> {
        if let Some(rule) = credential_in(text) {
            return Err(Refusal::Shape(rule));
        }
        if self.exact.iter().any(|value| text.contains(value.as_str())) {
            return Err(Refusal::Exact);
        }
        Ok(())
    }

    /// `text` with credential shapes and exact values redacted.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let mut out = redact(text);
        for value in &self.exact {
            if out.contains(value.as_str()) {
                out = out.replace(value.as_str(), "[redacted:exact]");
            }
        }
        out
    }
}

/// What [`Screen::redact_document`] took out, by rule name, with how many
/// of each. `exact` counts values this host holds as credentials.
pub type Counts = BTreeMap<String, u32>;

fn walk_strings(value: &mut serde_json::Value, each: &mut dyn FnMut(&mut String)) {
    match value {
        serde_json::Value::String(text) => each(text),
        serde_json::Value::Array(values) => {
            for value in values {
                walk_strings(value, each);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                walk_strings(value, each);
            }
        }
        _ => {}
    }
}

/// `text` with the markers redaction leaves (`[redacted:rule]`) blanked,
/// so a marker after a variable name doesn't read as its value.
fn without_markers(text: &str) -> std::borrow::Cow<'_, str> {
    static MARKER: OnceLock<Regex> = OnceLock::new();
    MARKER
        .get_or_init(|| Regex::new(r"\[redacted(:[a-z\-]+)?\]").expect("marker pattern"))
        .replace_all(text, " ")
}

/// The first credential rule any string in `document` (values and object
/// keys, at every depth) matches, ignoring what redaction already took
/// out. An uploaded or published document is refused when this is `Some`.
#[must_use]
pub fn credential_in_document(document: &serde_json::Value) -> Option<&'static str> {
    match document {
        serde_json::Value::String(text) => credential_in(&without_markers(text)),
        serde_json::Value::Array(values) => values.iter().find_map(credential_in_document),
        serde_json::Value::Object(fields) => fields.iter().find_map(|(key, value)| {
            credential_in(&without_markers(key)).or_else(|| credential_in_document(value))
        }),
        _ => None,
    }
}

/// Replace every string (and object key) in `document` that
/// [`credential_in_document`] would still refuse with
/// `[redacted:credential]`, whole. Redaction replaces matches inside a
/// string; once the markers are taken out, what is left can still read as
/// a credential (a match split around a redacted part, a key name), and an
/// upload refused for that helps nobody. Returns how many were replaced.
pub fn scrub_document(document: &mut serde_json::Value) -> u32 {
    const SCRUBBED: &str = "[redacted:credential]";
    let flagged = |text: &str| credential_in(&without_markers(text)).is_some();
    match document {
        serde_json::Value::String(text) => {
            if flagged(text) {
                *text = SCRUBBED.to_owned();
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().map(scrub_document).sum(),
        serde_json::Value::Object(fields) => {
            let mut count = 0;
            let keys: Vec<String> = fields.keys().filter(|key| flagged(key)).cloned().collect();
            for (index, key) in keys.into_iter().enumerate() {
                if let Some(value) = fields.remove(&key) {
                    fields.insert(format!("{SCRUBBED}{index}"), value);
                    count += 1;
                }
            }
            count + fields.values_mut().map(scrub_document).sum::<u32>()
        }
        _ => 0,
    }
}

impl Screen {
    /// Redacts every string in `document`, at every depth: credential
    /// shapes, this host's exact credential values, and personal data
    /// (home folder names and email addresses). Returns what was taken
    /// out, by rule. Object keys are left as they are.
    pub fn redact_document(&self, document: &mut serde_json::Value) -> Counts {
        let mut counts = Counts::new();
        walk_strings(document, &mut |text| {
            let mut out = redact_counted(text, &mut counts);
            for value in &self.exact {
                let found = out.matches(value.as_str()).count();
                if found > 0 {
                    out = out.replace(value.as_str(), "[redacted:exact]");
                    *counts.entry("exact".to_owned()).or_default() +=
                        u32::try_from(found).unwrap_or(u32::MAX);
                }
            }
            if out != *text {
                *text = out;
            }
        });
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_still_reads_as_a_credential_is_scrubbed_whole() {
        let key = format!("sk-{}", "a1".repeat(12));
        // A match that only forms once a marker is taken out (the bearer
        // marker, then a long word on the next line), and an object key.
        let mut joined = format!(
            "curl -H Authorization: Bearer {}\nabcdefghijklmnopqrstuvwxyz0123",
            "t0k3n".repeat(6)
        );
        joined = redact(&joined);
        assert!(joined.contains("[redacted:bearer]"));
        let mut document = serde_json::json!({
            "steps": [{"message": joined, "ok": "plain text"}],
            key.clone(): 1,
        });
        assert!(credential_in_document(&document).is_some());
        assert_eq!(scrub_document(&mut document), 2);
        assert_eq!(credential_in_document(&document), None);
        assert_eq!(document["steps"][0]["ok"], "plain text");
        assert_eq!(document["steps"][0]["message"], "[redacted:credential]");
        assert!(!document.to_string().contains(&key));
    }

    #[test]
    fn a_document_is_redacted_at_every_depth_and_then_passes() {
        let github = format!("ghp_{}", "Z9".repeat(18));
        let mut screen = Screen::shapes();
        let value = format!("plain{}", "x7".repeat(8));
        screen.add(value.clone());
        let mut document = serde_json::json!({
            "steps": [
                {"message": format!("use {github} please"),
                 "extra": {"env": "CLAUDE_CODE_OAUTH_TOKEN=opaque-value"}},
                {"observation": {"results": [
                    {"content": format!("cat /Users/alice/x {value} bob@example.com")}
                ]}}
            ]
        });
        assert!(credential_in_document(&document).is_some());
        let counts = screen.redact_document(&mut document);
        let text = document.to_string();
        for gone in [
            github.as_str(),
            "opaque-value",
            value.as_str(),
            "alice",
            "bob@example.com",
        ] {
            assert!(!text.contains(gone), "{gone}: {text}");
        }
        for rule in [
            "github-token",
            "claude-oauth-env",
            "exact",
            "home-directory",
            "email",
        ] {
            assert_eq!(counts.get(rule), Some(&1), "{rule}: {counts:?}");
        }
        assert_eq!(credential_in_document(&document), None);
        let keyed = serde_json::json!({ github.clone(): 1 });
        assert_eq!(credential_in_document(&keyed), Some("github-token"));
    }

    #[test]
    fn credential_shapes_are_redacted_and_counted() {
        let mut counts = BTreeMap::new();
        // Assembled at run time so no credential-shaped literal sits in
        // the source.
        let anthropic = format!("sk-ant-{}", "a1".repeat(20));
        let github = format!("ghp_{}", "Z9".repeat(18));
        let nsec = format!("nsec1{}", "q".repeat(58));
        let bearer = format!("Authorization: Bearer {}", "t0k".repeat(10));
        let aws = format!("AKIA{}", "ABCDEFGH23456789");
        let input = format!("{anthropic} {github} {nsec} {bearer} {aws}");
        let out = redact_counted(&input, &mut counts);
        for secret in [&anthropic, &github, &nsec, &aws] {
            assert!(!out.contains(secret.as_str()), "{out}");
        }
        for rule in [
            "anthropic-key",
            "github-token",
            "nostr-secret",
            "bearer",
            "aws-key",
        ] {
            assert_eq!(counts.get(rule), Some(&1), "{rule}: {out}");
        }
        assert_eq!(redact(&github), "[redacted:github-token]");
    }

    #[test]
    fn the_screen_refuses_shapes_and_exact_values() {
        let mut screen = Screen::shapes();
        assert_eq!(screen.check("run cargo test -p atif"), Ok(()));
        let github = format!("ghp_{}", "Z9".repeat(18));
        assert_eq!(
            screen.check(&format!("token {github}")),
            Err(Refusal::Shape("github-token"))
        );
        // A value with no known shape is refused only once the host
        // names it.
        let value = format!("plain{}", "x7".repeat(8));
        assert_eq!(screen.check(&format!("password is {value}")), Ok(()));
        screen.add(value.clone());
        screen.add("short");
        assert_eq!(
            screen.check(&format!("password is {value}")),
            Err(Refusal::Exact)
        );
        assert_eq!(screen.redact(&value), "[redacted:exact]");
        assert!(format!("{screen:?}").contains("1 exact"));
    }

    #[test]
    fn claude_logins_are_recognized_and_redacted() {
        // Assembled at run time so no credential-shaped literal sits in
        // the source.
        let access = format!("sk-ant-oat01-{}", "Q7".repeat(30));
        let refresh = format!("sk-ant-ort01-{}", "R8".repeat(30));
        let document = format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{access}","refreshToken":"{refresh}"}}}}"#
        );
        for text in [
            access.as_str(),
            refresh.as_str(),
            document.as_str(),
            "CLAUDE_CODE_OAUTH_TOKEN=opaque",
        ] {
            assert!(claude_login_in(text), "{text}");
            assert!(credential_in(text).is_some(), "{text}");
        }
        let api_key = format!("sk-ant-api03-{}", "k9".repeat(30));
        assert!(!claude_login_in(&api_key));
        assert!(!claude_login_in("run claude in the terminal"));
        let env = "export CLAUDE_CODE_OAUTH_TOKEN='opaque-value' && claude";
        let out = redact(env);
        assert!(!out.contains("opaque-value"), "{out}");
        assert!(out.contains("CLAUDE_CODE_OAUTH_TOKEN='[redacted:claude-oauth-env]"));
        let json = format!(r#"{{"CLAUDE_CODE_OAUTH_TOKEN": "{access}"}}"#);
        assert!(!redact(&json).contains(&access));
        assert!(!redact(&document).contains(&refresh));
        assert!(claude_login_path("/home/user/.claude/.credentials.json"));
        assert!(claude_login_path(".claude/.credentials.json"));
        assert!(!claude_login_path("docs/credentials.md"));
    }

    #[test]
    fn engine_logins_are_named_in_one_place() {
        assert!(engine_login_path("/home/u/.claude/.credentials.json"));
        assert!(engine_login_path("home/.codex/auth.json"));
        assert!(!engine_login_path("crates/auth.json"));
        for file in ENGINE_LOGIN_FILES {
            assert!(engine_login_path(file), "{file}");
        }
        assert!(ENGINE_LOGIN_ENV.contains(&CLAUDE_CODE_OAUTH_TOKEN));
        let access = format!("sk-ant-oat01-{}", "Q7".repeat(30));
        let document = format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{access}","scopes":["user"]}},"tokens":[{{"id_token":"abc"}}],"OPENAI_API_KEY":"k"}}"#
        );
        let mut found = credential_fragments(&document);
        found.sort();
        assert_eq!(found, vec!["abc".to_string(), "k".to_string(), access]);
        assert!(credential_fragments("not json").is_empty());
    }

    #[test]
    fn a_long_text_keeps_its_head_and_tail() {
        let text = format!("start{}é{}end", "x".repeat(200), "y".repeat(200));
        let cut = head_and_tail(&text, 64);
        assert!(cut.len() <= 64 && cut.starts_with("start") && cut.ends_with("end"));
    }
}
