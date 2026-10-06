//! The sheet's rules page: this computer's background rules, drawn
//! natively beside the foreground work (#10668).
//!
//! The page reads the host's own rule store and state through
//! `openagents --json background list`: each rule's identity, version and
//! digest, whether it is on or paused, the plugin that brings it, its last
//! check, last run and result, its escalation, and whether a host process
//! runs the rules at all. A rule belongs to the host, not to the page:
//! closing the page changes nothing, and reopening it shows the rule's real
//! last outcome. Pausing and resuming go through the existing
//! `openagents background pause|resume ID`, each only after CONFIRM; editing
//! and removing stay with `openagents background edit` and the plugin's own
//! off switch, which the page names. There is no scheduler or rules store
//! here.

use serde::Deserialize;
use std::sync::mpsc::Receiver;

/// The most bytes of helper output the page reads.
pub const READ_MAX: usize = 1024 * 1024;

/// A rule's state as the host keeps it, the fields the page shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct State {
    #[serde(default)]
    pub last_check: Option<u64>,
    #[serde(default)]
    pub next_check: Option<u64>,
    #[serde(default)]
    pub last_run: Option<u64>,
    #[serde(default)]
    pub last_run_id: Option<String>,
    #[serde(default)]
    pub last_result: Option<String>,
    #[serde(default)]
    pub last_escalation: Option<u64>,
    #[serde(default)]
    pub last_blocked: Option<String>,
}

/// One rule as `openagents background list` reports it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub paused_until: Option<u64>,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Rule {
    /// `on`, `paused`, `paused until ...`, or `broken`, at `now`.
    #[must_use]
    pub fn status(&self, now: u64) -> String {
        match (&self.error, self.enabled, self.paused_until) {
            (Some(_), ..) => "broken".into(),
            (None, false, _) => "paused".into(),
            (None, true, Some(until)) if until > now => format!("paused for {}", span(until - now)),
            (None, true, _) => "on".into(),
        }
    }

    /// Whether the rule runs now: the command that changes it is `pause`;
    /// otherwise it is `resume`.
    #[must_use]
    pub fn running(&self, now: u64) -> bool {
        self.error.is_none() && self.enabled && self.paused_until.is_none_or(|until| until <= now)
    }
}

/// The host's rules and whether a host process runs them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rules {
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub host: Option<String>,
}

/// What reading the rules answered: the rules, or why they can't be read.
pub type Read = Result<Rules, String>;

fn last_json(bytes: &[u8]) -> Option<serde_json::Value> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    serde_json::from_str(line).ok()
}

fn error_of(stdout: &[u8], stderr: &[u8]) -> String {
    last_json(stdout)
        .or_else(|| last_json(stderr))
        .and_then(|value| value["error"].as_str().map(crate::ascii::ascii))
        .unwrap_or_else(|| "the host's answer was not readable".into())
}

/// Decodes `openagents --json background list`.
#[must_use]
pub fn decode(stdout: &[u8], stderr: &[u8]) -> Read {
    if stdout.len() > READ_MAX {
        return Err("the rules are too large to show".into());
    }
    match last_json(stdout) {
        Some(value) if value.get("rules").is_some() => {
            serde_json::from_value(value).map_err(|_| "the host's rules were not readable".into())
        }
        _ => Err(error_of(stdout, stderr)),
    }
}

/// Decodes the answer to `openagents --json background pause|resume ID`:
/// the rule as saved, or the refusal.
#[must_use]
pub fn decode_change(stdout: &[u8], stderr: &[u8], id: &str) -> Result<(), String> {
    match last_json(stdout) {
        Some(value) if value["rule"]["id"].as_str() == Some(id) => Ok(()),
        _ => Err(error_of(stdout, stderr)),
    }
}

/// A pause or resume waiting for CONFIRM, sent, or answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    /// `pause` or `resume`.
    pub verb: &'static str,
    pub state: Changed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changed {
    Armed,
    Sending,
    Done,
    Refused(String),
}

/// The page's state.
#[derive(Default)]
pub struct Page {
    pub open: bool,
    pub shown: Option<Read>,
    pub reading: Option<Receiver<Read>>,
    pub dirty: bool,
    pub selected: usize,
    pub scroll: usize,
    pub reads: u64,
    pub change: Option<Change>,
    pub sending: Option<Receiver<Result<(), String>>>,
}

impl Page {
    /// The picked rule, when the rules are read.
    #[must_use]
    pub fn picked(&self) -> Option<&Rule> {
        match &self.shown {
            Some(Ok(rules)) => rules.rules.get(self.selected),
            _ => None,
        }
    }
}

/// `seconds` as a short span: `45 s`, `12 min`, `3 h`, or `2 days`.
#[must_use]
pub fn span(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min", seconds / 60),
        3600..172_800 => format!("{} h", seconds / 3600),
        _ => format!("{} days", seconds / 86_400),
    }
}

fn ago(at: u64, now: u64) -> String {
    format!("{} ago", span(now.saturating_sub(at)))
}

/// The page's text before wrapping, at `now` in Unix seconds.
#[must_use]
pub fn lines(page: &Page, now: u64) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    let mut out = Vec::new();
    let rules = match &page.shown {
        None => {
            out.push(("RULES on this computer  [reading]".into(), Tone::Loud));
            return out;
        }
        Some(Err(why)) => {
            out.push(("RULES on this computer  [unavailable]".into(), Tone::Loud));
            out.push((
                format!("The rules can't be read now: {why}. F11 twice reads them again."),
                Tone::Present,
            ));
            out.push(("CONTROLS none until the rules are read".into(), Tone::Quiet));
            return out;
        }
        Some(Ok(rules)) => rules,
    };
    let on = rules.rules.iter().filter(|rule| rule.running(now)).count();
    out.push((
        format!(
            "RULES on this computer  {} rules, {on} on  [{}]",
            rules.rules.len(),
            if page.reading.is_some() {
                "reading again"
            } else {
                "current"
            }
        ),
        Tone::Loud,
    ));
    out.push((
        match &rules.host {
            Some(host) => format!("HOST {}", ascii(host)),
            None => "HOST no host process runs the rules now; they run when one does".into(),
        },
        Tone::Present,
    ));
    out.push((
        "A rule belongs to this computer, not to this page: closing the page changes nothing."
            .into(),
        Tone::Quiet,
    ));
    if let Some(change) = &page.change {
        let state = match &change.state {
            Changed::Armed => "ENTER confirms, ESC rejects".to_owned(),
            Changed::Sending => "sent; waiting for the host".to_owned(),
            Changed::Done => "done".to_owned(),
            Changed::Refused(why) => format!("refused: {why}"),
        };
        out.push((
            format!(
                "{} {}: {state}",
                change.verb.to_uppercase(),
                ascii(&change.id)
            ),
            Tone::Loud,
        ));
    }
    out.push((String::new(), Tone::Quiet));
    if rules.rules.is_empty() {
        out.push((
            "No background rules are defined here.".into(),
            Tone::Present,
        ));
    }
    for (index, rule) in rules.rules.iter().enumerate() {
        let picked = index == page.selected;
        let name = if rule.name.is_empty() || rule.name == rule.id {
            String::new()
        } else {
            format!(" ({})", ascii(&rule.name))
        };
        out.push((
            format!(
                "{} {}{name}  {}  version {}",
                if picked { ">" } else { " " },
                ascii(&rule.id),
                rule.status(now),
                rule.version
            ),
            if picked { Tone::Loud } else { Tone::Present },
        ));
        if let Some(error) = &rule.error {
            out.push((
                format!("    can't be read: {}", ascii(error)),
                Tone::Present,
            ));
        }
        let last = match (&rule.state.last_result, rule.state.last_run) {
            (Some(result), Some(at)) => format!("last run {}: {}", ago(at, now), ascii(result)),
            (Some(result), None) => format!("last run: {}", ascii(result)),
            (None, _) => "not run yet".into(),
        };
        out.push((format!("    {last}"), Tone::Present));
        let mut checks = Vec::new();
        if let Some(at) = rule.state.last_check {
            checks.push(format!("checked {}", ago(at, now)));
        }
        if let Some(at) = rule.state.next_check
            && rule.running(now)
        {
            checks.push(if at > now {
                format!("next check in {}", span(at - now))
            } else {
                "next check due".into()
            });
        }
        if let Some(at) = rule.state.last_escalation {
            checks.push(format!("started a Coder run {}", ago(at, now)));
        }
        if let Some(plugin) = &rule.plugin {
            checks.push(format!("from plugin {}", ascii(plugin)));
        }
        if !checks.is_empty() {
            out.push((format!("    {}", checks.join(", ")), Tone::Quiet));
        }
        if let Some(blocked) = &rule.state.last_blocked {
            out.push((format!("    needs you: {}", ascii(blocked)), Tone::Present));
        }
        if picked {
            let short = rule.digest.trim_start_matches("sha256:");
            out.push((
                format!(
                    "    digest {}  edit: openagents background edit {} --message TEXT",
                    &short[..short.len().min(12)],
                    ascii(&rule.id)
                ),
                Tone::Quiet,
            ));
        }
    }
    out
}
