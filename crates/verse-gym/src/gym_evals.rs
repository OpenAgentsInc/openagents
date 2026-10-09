//! The Gym's EVALS board: published extension eval results, grouped by test
//! set and tool, with their checks and the credit behind them.
//!
//! The board reads the same records chat reads: NIP-EVAL result
//! publications (`3189`) under the extension evaluation profile
//! (`oa:ext-eval:v1`), each checked with [`eval_ext::parse_publication`]
//! before it counts. A check (a `3189` that cites another with the `check`
//! marker) is folded into the result it checks, as a confirmation or a
//! dispute ([`eval_ext::linkage`]); a publication that only claims to check
//! is shown as a result of its own. Test set and tool names come from their
//! NIP-EXT releases (`3184`) when the relay has them, and from the result's
//! own references otherwise. Credit is the trainer's XP from `eval-check`
//! and `eval-adopt` awards under the reader's trust list, which the caller
//! derives with the XP reader.
//!
//! Results are never pooled across test sets: each test set is its own group,
//! and a row states its own counts. Nothing here fetches or runs anything;
//! [`crate::gym_hall`] is the relay reader that feeds it.

use std::collections::{BTreeMap, BTreeSet};

use nostr::domain::Event;
use nostr::eval_ext::{self, Linkage, Publication, Verdict};
use serde::Serialize;

pub mod fixture;
#[cfg(test)]
mod tests;

/// The `t` marker every result under the profile carries.
pub const MARKER: &str = eval_ext::PROFILE_MARKER;
/// The kind of a result publication.
pub const RESULT_KIND: u16 = nostr::kb::EVIDENCE_KIND;
/// The kind of a NIP-EXT release.
pub const RELEASE_KIND: u16 = nostr::kinds::EXT_RELEASE;
/// The most results the board reads.
pub const RESULT_LIMIT: usize = 500;
/// The most rows one test set shows.
pub const MAX_ROWS: usize = 50;
/// The most test sets the board shows.
pub const MAX_GROUPS: usize = 50;

/// What the board says under its list, in plain words.
pub const NOTE: &str = "Results added to the Gym and checked on this phone. Each test set \
is compared only with itself. Credit is XP from checks and adoptions; it can't be spent.";

/// A NIP-EXT release as the board names it: the package's slug and its
/// version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub slug: String,
    pub version: String,
}

/// Reads a `3184` release's name, after checking its signature and body.
/// Anything else is `None`.
#[must_use]
pub fn release(event: &Event) -> Option<Release> {
    if event.kind != RELEASE_KIND {
        return None;
    }
    let body = nostr::ext::parse_record(event).ok()?;
    if body.get("type")?.as_str()? != "release" {
        return None;
    }
    let package = body.get("package")?.as_str()?;
    let (_, slug) = package.split_once(':')?;
    Some(Release {
        slug: slug.to_owned(),
        version: body.get("version")?.as_str()?.to_owned(),
    })
}

/// Names the board and the agents' notes use: releases by event ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    pub releases: BTreeMap<String, Release>,
}

impl Names {
    /// Reads every valid release among `events`.
    #[must_use]
    pub fn from_events<'a>(events: impl IntoIterator<Item = &'a Event>) -> Self {
        let releases = events
            .into_iter()
            .filter_map(|event| release(event).map(|r| (event.id.clone(), r)))
            .collect();
        Self { releases }
    }

    /// The test set's name: its release's slug and version, or the start
    /// of its release ID when the relay hasn't served the release.
    #[must_use]
    pub fn test_set(&self, publication: &Publication) -> String {
        let id = &publication.suite_release.id;
        match self.releases.get(id) {
            Some(r) => format!("{} {}", r.slug, r.version),
            None => format!("test set {}", short(id)),
        }
    }

    /// The tool's name: its package and component from the subject's
    /// qualified ID (`<pubkey>:<package>/<component>`), with the release's
    /// version when the relay has it.
    #[must_use]
    pub fn tool(&self, publication: &Publication) -> String {
        let definition = &publication.report.subject.definition;
        let name = tool_name(&definition.id);
        match publication
            .subject_release
            .as_ref()
            .and_then(|r| self.releases.get(&r.id))
        {
            Some(r) => format!("{name} {}", r.version),
            None => name,
        }
    }
}

/// `<pubkey>:<package>/<component>` as `package`, or `package/component`
/// when the component isn't the package's own name.
#[must_use]
pub fn tool_name(id: &str) -> String {
    let local = id.split_once(':').map_or(id, |(_, rest)| rest);
    match local.split_once('/') {
        Some((package, component)) if component == package || component.is_empty() => {
            package.to_owned()
        }
        Some((package, component)) => format!("{package}/{component}"),
        None => local.to_owned(),
    }
}

/// The first eight hex characters: how the Grid names a player.
#[must_use]
pub fn short(hex: &str) -> String {
    hex.chars().take(8).collect()
}

/// The verdict in the board's words.
#[must_use]
pub fn verdict_words(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "Better",
        Verdict::Fail => "Worse",
        Verdict::Inconclusive => "No clear change",
    }
}

/// The headline counts in plain words: `6 of 8 with it, 4 of 8 without`.
#[must_use]
pub fn headline(publication: &Publication) -> String {
    let h = publication.report.profile.headline;
    match h.baseline_passed {
        Some(without) => format!(
            "{} of {} passed with it, {} of {} without",
            h.subject_passed, h.total, without, h.total
        ),
        None => format!(
            "{} of {} passed with it, not run without it",
            h.subject_passed, h.total
        ),
    }
}

/// Every valid result publication among `events`, by event ID. Events may
/// repeat; anything that fails [`eval_ext::parse_publication`] is left out.
#[must_use]
pub fn verified<'a>(events: impl IntoIterator<Item = &'a Event>) -> BTreeMap<String, Publication> {
    let mut out = BTreeMap::new();
    for event in events {
        if event.kind != RESULT_KIND
            || out.contains_key(&event.id)
            || !event.tag_values("t").any(|t| t == MARKER)
        {
            continue;
        }
        if let Ok(publication) = eval_ext::parse_publication(event)
            && !is_sample(&publication.report.subject.definition.id)
        {
            out.insert(event.id.clone(), publication);
        }
    }
    out
}

/// The starter catalog's placeholder key, which names the sample plugins'
/// Wasm guests (`ext_eval::author::catalog::STARTER_KEY`).
const STARTER_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Whether a subject's definition ID names one of the hosted runner's
/// sample plugins (`deploy/eval-runner/catalog`): signed by the runner's
/// key or the starter catalog's. They are the runner's test fixtures and
/// never stand on the board.
#[must_use]
pub fn is_sample(definition_id: &str) -> bool {
    definition_id
        .split_once(':')
        .is_some_and(|(key, _)| key == eval_ext::hosted::RUNNER || key == STARTER_KEY)
}

/// One result on the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The result's `3189` event ID.
    pub id: String,
    pub tool: String,
    /// The trainer's hex public key: a hosted run's requester, otherwise
    /// the evaluator.
    pub trainer: String,
    /// The trainer's first eight hex characters.
    pub trainer_tag: String,
    /// This player's own result.
    pub mine: bool,
    pub headline: String,
    /// `pass`, `fail`, or `inconclusive`.
    pub verdict: &'static str,
    /// `Better`, `Worse`, or `No clear change`.
    pub verdict_words: &'static str,
    pub confirmed: usize,
    pub disputed: usize,
    /// The checks in plain words, empty without any.
    pub checks: String,
    /// The trainer's XP from `eval-check` and `eval-adopt` awards.
    pub credit_xp: u64,
    /// Served by the hosted runner rather than the trainer's own computer.
    pub hosted: bool,
    pub published_at: u64,
}

/// One test set and its results, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Group {
    pub test_set: String,
    /// The test set's release event ID.
    pub release: String,
    /// The test set's author's first eight hex characters.
    pub author_tag: String,
    pub rows: Vec<Row>,
    /// Rows past [`MAX_ROWS`], not shown.
    pub more: usize,
}

/// The whole board.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Board {
    /// Test sets with the newest result first.
    pub groups: Vec<Group>,
    /// Results shown, checks folded in.
    pub results: usize,
    /// Checks folded into the results they check.
    pub checks: usize,
}

fn check_words(confirmed: usize, disputed: usize) -> String {
    let count = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    match (confirmed, disputed) {
        (0, 0) => String::new(),
        (c, 0) => format!("Confirmed by {}", count(c, "check", "checks")),
        (0, d) => format!("Disputed by {}", count(d, "check", "checks")),
        (c, d) => format!(
            "Confirmed by {}, disputed by {}",
            count(c, "check", "checks"),
            count(d, "check", "checks")
        ),
    }
}

/// Builds the board from verified publications. `me` is this player's
/// public key, and `credit` each trainer's XP from `eval-check` and
/// `eval-adopt` awards.
#[must_use]
pub fn board(
    publications: &BTreeMap<String, Publication>,
    names: &Names,
    credit: &BTreeMap<String, u64>,
    me: &str,
) -> Board {
    // Fold every true check into the result it checks.
    let mut tally: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut folded: BTreeSet<&str> = BTreeSet::new();
    for check in publications.values() {
        let Some(original) = check.checks.as_deref().and_then(|id| publications.get(id)) else {
            continue;
        };
        match eval_ext::linkage(original, check) {
            Linkage::Confirm => tally.entry(original.id.as_str()).or_default().0 += 1,
            Linkage::Dispute => tally.entry(original.id.as_str()).or_default().1 += 1,
            Linkage::NotACheck => continue,
        }
        folded.insert(check.id.as_str());
    }
    let mut groups: BTreeMap<&str, Vec<&Publication>> = BTreeMap::new();
    for publication in publications.values() {
        if !folded.contains(publication.id.as_str()) {
            groups
                .entry(publication.suite_release.id.as_str())
                .or_default()
                .push(publication);
        }
    }
    let mut out = Board {
        results: publications.len() - folded.len(),
        checks: folded.len(),
        groups: Vec::new(),
    };
    for (release, mut members) in groups {
        members.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
        let more = members.len().saturating_sub(MAX_ROWS);
        let rows = members
            .iter()
            .take(MAX_ROWS)
            .map(|p| {
                let (confirmed, disputed) = tally.get(p.id.as_str()).copied().unwrap_or_default();
                let trainer = p.trainer().to_owned();
                Row {
                    id: p.id.clone(),
                    tool: names.tool(p),
                    trainer_tag: short(&trainer),
                    mine: trainer == me,
                    credit_xp: credit.get(&trainer).copied().unwrap_or(0),
                    trainer,
                    headline: headline(p),
                    verdict: p.verdict().word(),
                    verdict_words: verdict_words(p.verdict()),
                    confirmed,
                    disputed,
                    checks: check_words(confirmed, disputed),
                    hosted: p.report.profile.requester.is_some(),
                    published_at: p.created_at,
                }
            })
            .collect();
        out.groups.push(Group {
            test_set: names.test_set(members[0]),
            release: release.to_owned(),
            author_tag: short(members[0].suite_author()),
            rows,
            more,
        });
    }
    out.groups.sort_by(|a, b| {
        let newest = |g: &Group| g.rows.first().map_or(0, |r| r.published_at);
        newest(b)
            .cmp(&newest(a))
            .then_with(|| a.release.cmp(&b.release))
    });
    out.groups.truncate(MAX_GROUPS);
    out
}

/// Each trainer's XP from `eval-check` and `eval-adopt` credits, given as
/// `(rule, pubkey, xp)`.
#[must_use]
pub fn eval_credit<'a>(
    credits: impl IntoIterator<Item = (&'a str, &'a str, u64)>,
) -> BTreeMap<String, u64> {
    let mut out: BTreeMap<String, u64> = BTreeMap::new();
    for (rule, pubkey, xp) in credits {
        if rule == nostr::xp::EVAL_CHECK || rule == nostr::xp::EVAL_ADOPT {
            *out.entry(pubkey.to_owned()).or_default() += xp;
        }
    }
    out
}
