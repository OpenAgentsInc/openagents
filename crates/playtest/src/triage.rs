//! Triage: from opened reports to drafted GitHub issues and the triage log.
//!
//! The triage inbox (`openagents playtest inbox`) opens the triage key's
//! reports ([`crate::report::open`]), keeps the ones it hasn't seen
//! ([`fresh`]), and drafts an issue for each ([`draft`]) for a person to
//! edit and approve. Deduplication here is by exact identity only: the gift
//! wrap's ID, the report message's ID, and the report's digest. Two reports
//! about the same problem are the triager's call, recorded as a
//! `duplicate` decision, never guessed from their words.
//!
//! The **triage log** is an append-only list of [`Entry`] values, one JSON
//! object per line. It records every report received and every decision,
//! filing, session, and verification, with the tester's key, so it can say
//! later which accepted contributions back which playtest awards
//! ([`Log::acceptances`]). Accepted only means a person on the OpenAgents
//! side filed or recorded it; receiving a report earns nothing.
//!
//! This module reads and writes values; the command does the file and
//! network work.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::report::{Kind, Opened, Platform};
use crate::session::{self, Route, Tab};

/// The label every playtest issue carries.
pub const LABEL: &str = "playtest";

/// What the triager decided about a report that isn't filed as a new issue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// The same problem as an existing issue; linked, earns nothing.
    Duplicate,
    /// Couldn't be reproduced yet; it can earn later if it is.
    NotReproducible,
    /// A design point, kept for the weekly evaluation.
    Design,
    /// An idea, kept for the weekly evaluation.
    Idea,
    /// Declined, with a reason; earns nothing.
    Declined,
}

/// What an accepted contribution was, as the `playtest` NIP-XP rule names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Contribution {
    /// Accepted feedback: a new `playtest` issue from a bug, confusing
    /// state, or design point.
    Feedback,
    /// A reproducible bug report, reproduced by the triager.
    Bug,
    /// A design finding that shipped.
    Design,
}

/// Issue severity, as the triage section of `docs/game/playtesting.md`
/// defines it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Loses money, leaks a key, or bricks the app.
    P0,
    /// Blocks a scripted task.
    P1,
    /// Hurts, with a way around.
    P2,
    /// Polish.
    P3,
}

/// How a session or diary was run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Unmoderated,
    Moderated,
    Group,
    Diary,
}

/// The reporter's check of a fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verified {
    /// The reporter confirmed the fix on the fixing build.
    Yes,
    /// The reporter still sees the problem on the fixing build.
    No,
    /// Closed as fixed without the reporter's confirmation.
    Unverified,
}

/// One line of the triage log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Entry {
    /// A report arrived and was drafted.
    Received {
        at: u64,
        code: String,
        rumor_id: String,
        wrap_id: String,
        /// Lowercase hex SHA-256 of the report's exact content.
        digest: String,
        /// The tester's key, hex.
        tester: String,
        /// `1.0.0 (15)`.
        build: String,
        platform: Platform,
        tab: Tab,
        route: Route,
        kind: Kind,
    },
    /// The triager decided not to file it as a new issue.
    Decided {
        at: u64,
        code: String,
        decision: Decision,
        reason: String,
        /// The issue it duplicates or was kept under, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issue: Option<u64>,
    },
    /// Accepted: filed as a new `playtest` issue. `at` is the acceptance
    /// time an award cites.
    Filed {
        at: u64,
        code: String,
        issue: u64,
        contribution: Contribution,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        severity: Option<Severity>,
        /// The key of the person who accepted it, hex, when known. It is
        /// never the tester's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        triager: Option<String>,
    },
    /// A completed session script or diary week whose report was accepted.
    Session {
        at: u64,
        tester: String,
        /// `session-1`, `session-2`, `raid`, `diary`, and so on.
        script: String,
        format: Format,
        build: String,
        /// The accepted report's code, when it came through the inbox.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moderator: Option<String>,
    },
    /// The fix for an issue shipped, and the reporter checked it or not.
    Verified {
        at: u64,
        issue: u64,
        fix_build: String,
        verified: Verified,
    },
}

impl Entry {
    /// The report code the entry is about, if any.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Received { code, .. } | Self::Decided { code, .. } | Self::Filed { code, .. } => {
                Some(code)
            }
            Self::Session { code, .. } => code.as_deref(),
            Self::Verified { .. } => None,
        }
    }

    /// The entry received for an opened report at `at`.
    #[must_use]
    pub fn received(opened: &Opened, at: u64) -> Self {
        let context = &opened.report.context;
        Self::Received {
            at,
            code: opened.code.clone(),
            rumor_id: opened.rumor_id.clone(),
            wrap_id: opened.wrap_id.clone(),
            digest: opened.digest.clone(),
            tester: opened.tester.clone(),
            build: context.build_label(),
            platform: context.platform,
            tab: context.tab,
            route: context.route,
            kind: opened.report.kind,
        }
    }
}

/// One accepted contribution, with everything a playtest award needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Acceptance {
    /// The tester's key, hex.
    pub tester: String,
    /// `feedback`, `bug`, `design`, `verified-fix`, `session`, or `diary`.
    pub contribution: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// The build the report or session was on.
    pub build: String,
    pub accepted_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<Verified>,
}

/// The triage log, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub entries: Vec<Entry>,
}

impl Log {
    /// Reads a log from its JSON-lines text. Blank lines are skipped.
    ///
    /// # Errors
    ///
    /// The first line that isn't a log entry, by number.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            entries.push(
                serde_json::from_str(line)
                    .map_err(|e| format!("triage log line {}: {e}", index + 1))?,
            );
        }
        Ok(Self { entries })
    }

    /// One entry as a log line, with its newline.
    #[must_use]
    pub fn line(entry: &Entry) -> String {
        let mut line = serde_json::to_string(entry).unwrap_or_default();
        line.push('\n');
        line
    }

    /// Whether a delivery with any of these identities is already logged.
    #[must_use]
    pub fn seen(&self, opened: &Opened) -> bool {
        self.entries.iter().any(|entry| match entry {
            Entry::Received {
                rumor_id,
                wrap_id,
                digest,
                ..
            } => {
                *rumor_id == opened.rumor_id
                    || *wrap_id == opened.wrap_id
                    || *digest == opened.digest
            }
            _ => false,
        })
    }

    /// The received entry for `code`.
    #[must_use]
    pub fn report(&self, code: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| matches!(entry, Entry::Received { code: c, .. } if c == code))
    }

    /// Codes received with no decision or filing yet, in arrival order.
    #[must_use]
    pub fn pending(&self) -> Vec<&str> {
        let handled: BTreeSet<&str> = self
            .entries
            .iter()
            .filter(|e| matches!(e, Entry::Decided { .. } | Entry::Filed { .. }))
            .filter_map(Entry::code)
            .collect();
        self.entries
            .iter()
            .filter(|e| matches!(e, Entry::Received { .. }))
            .filter_map(Entry::code)
            .filter(|code| !handled.contains(code))
            .collect()
    }

    /// Checks that `entry` can follow this log: it names a received report
    /// that hasn't been decided or filed, an issue isn't accepted twice,
    /// and the triager isn't the tester.
    ///
    /// # Errors
    ///
    /// A sentence naming the problem.
    pub fn admit(&self, entry: &Entry) -> Result<(), String> {
        match entry {
            Entry::Received { .. } => Ok(()),
            Entry::Decided { code, .. } | Entry::Filed { code, .. } => {
                let Some(Entry::Received { tester, .. }) = self.report(code) else {
                    return Err(format!("no report {code} in the triage log"));
                };
                if !self.pending().contains(&code.as_str()) {
                    return Err(format!("{code} is already decided or filed"));
                }
                if let Entry::Filed { issue, triager, .. } = entry {
                    if triager.as_deref() == Some(tester.as_str()) {
                        return Err("the triager can't accept their own report".into());
                    }
                    let taken = self
                        .entries
                        .iter()
                        .any(|e| matches!(e, Entry::Filed { issue: i, .. } if i == issue));
                    if taken {
                        return Err(format!(
                            "issue #{issue} already has its accepted reporter; record this report as a duplicate"
                        ));
                    }
                }
                Ok(())
            }
            Entry::Session {
                tester,
                script,
                moderator,
                ..
            } => {
                if moderator.as_deref() == Some(tester.as_str()) {
                    return Err("the moderator can't accept their own session".into());
                }
                let repeat = self.entries.iter().any(|e| {
                    matches!(e, Entry::Session { tester: t, script: s, .. } if t == tester && s == script)
                });
                if repeat {
                    return Err(format!("{script} is already recorded for this tester"));
                }
                Ok(())
            }
            Entry::Verified { issue, .. } => {
                let filed = self
                    .entries
                    .iter()
                    .any(|e| matches!(e, Entry::Filed { issue: i, .. } if i == issue));
                if filed {
                    Ok(())
                } else {
                    Err(format!(
                        "issue #{issue} wasn't filed from a playtest report"
                    ))
                }
            }
        }
    }

    /// Every accepted contribution, with the tester's key: each filed
    /// report, each fix its reporter verified, and each recorded session.
    /// Duplicates, declines, and reports never decided don't appear.
    #[must_use]
    pub fn acceptances(&self) -> Vec<Acceptance> {
        let mut received: BTreeMap<&str, (&str, &str, &str)> = BTreeMap::new();
        for entry in &self.entries {
            if let Entry::Received {
                code,
                tester,
                digest,
                build,
                ..
            } = entry
            {
                received.insert(code, (tester, digest, build));
            }
        }
        let mut verified: BTreeMap<u64, (&str, Verified, u64)> = BTreeMap::new();
        for entry in &self.entries {
            if let Entry::Verified {
                at,
                issue,
                fix_build,
                verified: check,
            } = entry
            {
                verified.insert(*issue, (fix_build, *check, *at));
            }
        }
        let mut out = Vec::new();
        for entry in &self.entries {
            match entry {
                Entry::Filed {
                    at,
                    code,
                    issue,
                    contribution,
                    severity,
                    ..
                } => {
                    let Some((tester, digest, build)) = received.get(code.as_str()) else {
                        continue;
                    };
                    let check = verified.get(issue);
                    let accepted = Acceptance {
                        tester: (*tester).to_owned(),
                        contribution: session::name(contribution),
                        code: Some(code.clone()),
                        digest: Some((*digest).to_owned()),
                        issue: Some(*issue),
                        severity: *severity,
                        script: None,
                        build: (*build).to_owned(),
                        accepted_at: *at,
                        fix_build: check.map(|c| c.0.to_owned()),
                        verified: check.map(|c| c.1),
                    };
                    if let Some((fix_build, Verified::Yes, verified_at)) = check {
                        out.push(Acceptance {
                            contribution: "verified-fix".into(),
                            accepted_at: *verified_at,
                            severity: None,
                            build: (*fix_build).to_owned(),
                            ..accepted.clone()
                        });
                    }
                    out.push(accepted);
                }
                Entry::Session {
                    at,
                    tester,
                    script,
                    format,
                    build,
                    code,
                    ..
                } => out.push(Acceptance {
                    tester: tester.clone(),
                    contribution: if *format == Format::Diary {
                        "diary".into()
                    } else {
                        "session".into()
                    },
                    code: code.clone(),
                    digest: code
                        .as_deref()
                        .and_then(|c| received.get(c))
                        .map(|r| r.1.to_owned()),
                    issue: None,
                    severity: None,
                    script: Some(script.clone()),
                    build: build.clone(),
                    accepted_at: *at,
                    fix_build: None,
                    verified: None,
                }),
                _ => {}
            }
        }
        out.sort_by_key(|a| a.accepted_at);
        out
    }
}

/// The reports in `opened` that neither the log nor an earlier report in
/// the batch has: exact wrap ID, message ID, or content digest.
#[must_use]
pub fn fresh(opened: Vec<Opened>, log: &Log) -> Vec<Opened> {
    let mut seen = BTreeSet::new();
    opened
        .into_iter()
        .filter(|report| {
            !log.seen(report)
                && seen.insert(report.wrap_id.clone())
                && seen.insert(format!("rumor:{}", report.rumor_id))
                && seen.insert(format!("digest:{}", report.digest))
        })
        .collect()
}

/// A drafted issue, for a person to edit and approve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub code: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

impl Draft {
    /// The draft as an editable Markdown file: the title as its first
    /// heading, then the body.
    #[must_use]
    pub fn markdown(&self) -> String {
        format!("# {}\n\n{}", self.title, self.body)
    }

    /// Reads an edited Markdown file back: the first `# ` line is the
    /// title, the rest is the body.
    ///
    /// # Errors
    ///
    /// When the file has no title line.
    pub fn from_markdown(code: &str, labels: Vec<String>, text: &str) -> Result<Self, String> {
        let text = text.trim_start();
        let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
        let title = first
            .strip_prefix("# ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or("the draft's first line must be `# Title`")?;
        Ok(Self {
            code: code.to_owned(),
            title: title.to_owned(),
            body: rest.trim_start_matches('\n').to_owned(),
            labels,
        })
    }
}

/// The paraphrase placeholder a draft carries when the tester didn't allow
/// quoting. Filing refuses a draft that still holds it.
pub const PARAPHRASE: &str =
    "<!-- The tester didn't allow quoting. Write this in your own words. -->";

fn area(tab: Tab) -> &'static str {
    match tab {
        Tab::Coder => "coder",
        Tab::Verse => "grid",
        Tab::Wallet => "wallet",
        Tab::Account => "account",
    }
}

fn quote(text: &str) -> String {
    if text.trim().is_empty() {
        return "_Not given._".into();
    }
    text.lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Drafts the GitHub issue for an opened report. The tester's words appear
/// only when they allowed quoting; otherwise the draft holds
/// [`PARAPHRASE`] slots. No screenshot or session log is embedded: the
/// draft says they are attached to the private report.
#[must_use]
pub fn draft(opened: &Opened) -> Draft {
    let report = &opened.report;
    let c = &report.context;
    let kind = session::name(&report.kind);
    let place = format!("{}/{}", session::name(&c.tab), session::name(&c.route));
    let title = if report.quote {
        let first = report.happened.lines().next().unwrap_or_default().trim();
        let mut short: String = first.chars().take(72).collect();
        if first.chars().count() > 72 {
            short.push('…');
        }
        format!("{}: {short}", area(c.tab))
    } else {
        format!("{}: {kind} on {place} (write a title)", area(c.tab))
    };
    let text = |value: &str| {
        if report.quote {
            quote(value)
        } else {
            PARAPHRASE.to_owned()
        }
    };
    let mut attachments = Vec::new();
    if report.screenshot.is_some() {
        attachments.push("- A screenshot is attached to the private report; it isn't published.");
    }
    if report.session.is_some() {
        attachments.push("- The tester's session log is attached to the private report.");
    }
    if report.task.is_some() {
        attachments.push("- The Coder chat's task ID is in the private report.");
    }
    for note in &report.notes {
        attachments.push(match note {
            crate::report::Note::ScreenshotDropped => {
                "- The app left the screenshot out because it was too large."
            }
            crate::report::Note::SessionTrimmed => {
                "- The app left out the oldest session events to fit."
            }
        });
    }
    let platform = match c.platform {
        Platform::Ios => "iOS",
        Platform::Android => "Android",
    };
    let body = format!(
        "Playtest report `{code}` ({kind}), seen on OpenAgents {build} on {platform} {os} ({device}), in {place}.\n\n\
         Severity: P? (P0 loses money, leaks a key, or bricks the app; P1 blocks a scripted task; P2 hurts with a way around; P3 polish).\n\n\
         ## What happened\n\n{happened}\n\n\
         ## Expected\n\n{expected}\n\n\
         ## Steps\n\n{steps}\n\n\
         {attachments}\
         The tester's words are {quoted}. The report's digest is `sha256:{digest}`; the tester's key is in the triage log.\n",
        code = opened.code,
        build = c.build_label(),
        os = c.os_version,
        device = c.device,
        happened = text(&report.happened),
        expected = text(&report.expected),
        steps = text(&report.steps),
        attachments = if attachments.is_empty() {
            String::new()
        } else {
            format!("## Attachments\n\n{}\n\n", attachments.join("\n"))
        },
        quoted = if report.quote {
            "quoted with their permission"
        } else {
            "not quoted: they didn't allow it"
        },
        digest = opened.digest,
    );
    Draft {
        code: opened.code.clone(),
        title,
        body,
        labels: labels(opened),
    }
}

/// `playtest`, `build:1.0.0-15`, and `area:<surface>`.
#[must_use]
pub fn labels(opened: &Opened) -> Vec<String> {
    let c = &opened.report.context;
    vec![
        LABEL.to_owned(),
        format!("build:{}-{}", c.app_version, c.build),
        format!("area:{}", area(c.tab)),
    ]
}

#[cfg(test)]
mod tests;
