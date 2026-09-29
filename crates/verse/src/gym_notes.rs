//! Agents comparing notes in the Gym.
//!
//! When two trainers stand in the Grid's Gym and both have switched on
//! **Compare notes**, their agents trade short notes about the eval results
//! they published: which tool they tested, on which test set, and what came
//! of it. One agent opens with its newest result and asks who else ran that
//! test set; another answers once with its own result on the same test set
//! and how the two compare, or says it hasn't run it.
//!
//! A note is a NIP-MV world chat line (NIP-C7 kind `9`) in the Gym's zone
//! (`t=zone`, `z=gym`), labeled with NIP-32 (`L=openagents.gym`,
//! `l=note`), citing the result publications it speaks about with `e` tags
//! marked `source`. An answer quotes the note it answers with NIP-C7's `q`
//! tag and names its author with `p`. Any NIP-C7 client reads the text; a
//! Verse client places it in the Gym.
//!
//! Notes are grounded by construction:
//!
//! - A note carries only public event IDs and text computed from them. It
//!   never carries chat content, files, keys, or anything the trainer did
//!   not already publish.
//! - A reader re-checks every cited result ([`crate::gym_evals::verified`])
//!   and requires each to be the note author's own (its trainer is the
//!   author). A note that cites anything else is dropped.
//! - A reader never shows a note's text. It renders its own from the
//!   verified results ([`render`]), so a note can't put words on another
//!   trainer's board that its sources don't support.
//!
//! What an agent says is typed ([`Plan`]) and chosen by fixed rules over
//! exact event fields (the same test set is the same release ID), so no
//! free text is interpreted anywhere. The agent speaks in the plural, as
//! OpenAgents does. [`Policy`] bounds how often it speaks.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nostr::domain::{Event, Tag};
use nostr::eval_ext::{Publication, Verdict};

use crate::gym_evals::{Names, short};

/// The `z` district every note carries: the Gym.
pub const ZONE: &str = "gym";
/// The NIP-32 namespace of the note label.
pub const NAMESPACE: &str = "openagents.gym";
/// The NIP-32 label every note carries.
pub const LABEL: &str = "note";
/// The `e` marker for a cited result.
pub const SOURCE: &str = "source";
/// The longest note, as any other chat line.
pub const MAX_TEXT: usize = crate::chat::MAX_LINE;
/// The most results one note cites.
pub const MAX_SOURCES: usize = 1;
/// The shortest time between two notes that open a conversation, and
/// between an agent's notes of any kind before it opens again.
pub const OPEN_INTERVAL: u64 = 15 * 60;
/// The shortest time between two answers to the same trainer.
pub const PEER_INTERVAL: u64 = 15 * 60;
/// The most notes an agent sends in an hour.
pub const MAX_PER_HOUR: usize = 4;
/// How old a note may be and still get an answer.
pub const ANSWER_WINDOW: u64 = 10 * 60;
/// How far in the future a note's time may be.
pub const FUTURE_SKEW: u64 = 5 * 60;
/// How far back a reader asks for notes.
pub const HISTORY: u64 = 60 * 60;
/// The most notes a reader keeps.
pub const MAX_NOTES: usize = 100;

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_owned()).collect())
}

fn is_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A note as the wire carries it, before its sources are checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub author: String,
    pub created_at: u64,
    /// Cited results, at most [`MAX_SOURCES`].
    pub sources: Vec<String>,
    /// The note this one answers, and its author.
    pub answers: Option<(String, String)>,
    /// The text for plain NIP-C7 clients. Verse never shows it.
    pub content: String,
}

impl Note {
    /// Opens a conversation rather than answering one.
    #[must_use]
    pub fn opens(&self) -> bool {
        self.answers.is_none()
    }
}

/// Reads a note in `world`. Anything that isn't one is an error naming why.
///
/// # Errors
///
/// When the event isn't a well-formed Gym note in `world`.
pub fn parse(event: &Event, world: &str) -> Result<Note, &'static str> {
    if event.kind != 9 {
        return Err("kind");
    }
    fn one<'a>(event: &'a Event, name: &'a str) -> Option<&'a str> {
        let mut values = event.tag_values(name);
        let first = values.next();
        if values.next().is_some() { None } else { first }
    }
    if one(event, "w") != Some(world)
        || one(event, "z") != Some(ZONE)
        || one(event, "t") != Some("zone")
    {
        return Err("scope");
    }
    if one(event, "L") != Some(NAMESPACE) {
        return Err("label namespace");
    }
    let labeled = event.tags.iter().any(|t| {
        let v = t.as_slice();
        v.len() >= 3 && v[0] == "l" && v[1] == LABEL && v[2] == NAMESPACE
    });
    if !labeled {
        return Err("label");
    }
    let mut sources = Vec::new();
    for t in &event.tags {
        let v = t.as_slice();
        if v.first().map(String::as_str) != Some("e") {
            continue;
        }
        if v.get(3).map(String::as_str) != Some(SOURCE) || !is_hex64(&v[1]) {
            return Err("source");
        }
        if !sources.contains(&v[1]) {
            sources.push(v[1].clone());
        }
    }
    if sources.len() > MAX_SOURCES {
        return Err("too many sources");
    }
    let quotes: Vec<&Tag> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some("q"))
        .collect();
    let answers = match quotes.as_slice() {
        [] => None,
        [q] => {
            let v = q.as_slice();
            let (Some(id), Some(author)) = (v.get(1), v.get(3)) else {
                return Err("quote");
            };
            if !is_hex64(id) || !is_hex64(author) || author == &event.pubkey {
                return Err("quote");
            }
            Some((id.clone(), author.clone()))
        }
        _ => return Err("quote"),
    };
    if answers.is_none() && sources.is_empty() {
        return Err("an opening note cites a result");
    }
    let chars = event.content.chars().count();
    if chars == 0 || chars > MAX_TEXT || event.content.chars().any(char::is_control) {
        return Err("text");
    }
    Ok(Note {
        id: event.id.clone(),
        author: event.pubkey.clone(),
        created_at: event.created_at,
        sources,
        answers,
        content: event.content.clone(),
    })
}

/// What an agent says, before it becomes a note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Open with our result `ours`.
    Open { ours: String },
    /// Answer the note `note` by `peer`, citing our result on the same
    /// test set when we have one.
    Answer {
        note: String,
        peer: String,
        ours: Option<String>,
    },
}

/// The tags of the note `plan` makes in `world`, for
/// [`nostr::domain::RelaySigner::sign`]. `relay` is the relay hint an
/// answer's quote carries, as NIP-C7 asks.
#[must_use]
pub fn tags(world: &str, relay: &str, plan: &Plan) -> Vec<Tag> {
    let mut out = vec![
        tag(&["w", world]),
        tag(&["t", "zone"]),
        tag(&["z", ZONE]),
        tag(&["L", NAMESPACE]),
        tag(&["l", LABEL, NAMESPACE]),
    ];
    match plan {
        Plan::Open { ours } => out.push(tag(&["e", ours, "", SOURCE])),
        Plan::Answer { note, peer, ours } => {
            if let Some(ours) = ours {
                out.push(tag(&["e", ours, "", SOURCE]));
            }
            out.push(tag(&["q", note, relay, peer]));
            out.push(tag(&["p", peer]));
        }
    }
    out
}

fn verdict_sentence(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "It helped.",
        Verdict::Fail => "It made things worse.",
        Verdict::Inconclusive => "No clear change.",
    }
}

fn counts(p: &Publication) -> String {
    let h = p.report.profile.headline;
    match h.baseline_passed {
        Some(without) => format!(
            "{} of {} cases passed with it, {} of {} without",
            h.subject_passed, h.total, without, h.total
        ),
        None => format!(
            "{} of {} cases passed with it; we didn't run it without",
            h.subject_passed, h.total
        ),
    }
}

/// Cases the tool added over the run without it; `None` without a
/// baseline.
fn lift(p: &Publication) -> Option<i64> {
    let h = p.report.profile.headline;
    h.baseline_passed
        .map(|without| h.subject_passed as i64 - without as i64)
}

fn signed(n: i64) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

fn clip(mut text: String) -> String {
    if text.chars().count() > MAX_TEXT {
        text = text.chars().take(MAX_TEXT - 1).collect();
        text.push('…');
    }
    text
}

/// The text of an opening note about our result `ours`.
#[must_use]
pub fn render_open(ours: &Publication, names: &Names) -> String {
    let set = names.test_set(ours);
    clip(format!(
        "We tested {} on {set}: {}. {} Has anyone here run {set}?",
        names.tool(ours),
        counts(ours),
        verdict_sentence(ours.verdict()),
    ))
}

/// The text of an answer to a note about `theirs`, with our result on the
/// same test set when we have one.
#[must_use]
pub fn render_answer(ours: Option<&Publication>, theirs: &Publication, names: &Names) -> String {
    let set = names.test_set(theirs);
    let Some(ours) = ours else {
        return clip(format!(
            "We haven't run {set} yet, so we have nothing to compare with your {}.",
            names.tool(theirs)
        ));
    };
    let (our_tool, their_tool) = (names.tool(ours), names.tool(theirs));
    let mut text = format!(
        "We ran {set} too, with {our_tool}: {}. {}",
        counts(ours),
        verdict_sentence(ours.verdict())
    );
    let same_tool = ours.report.subject.definition.id == theirs.report.subject.definition.id;
    if same_tool {
        if ours.verdict() == theirs.verdict() {
            text.push_str(" Same tool as yours, and our verdicts agree.");
        } else {
            text.push_str(
                " Same tool as yours, but our verdicts differ. What's different about your project?",
            );
        }
    } else if let (Some(a), Some(b)) = (lift(ours), lift(theirs)) {
        if a == b {
            text.push_str(&format!(
                " {our_tool} and your {their_tool} added the same ({} cases).",
                signed(a)
            ));
        } else {
            let (more, less, big, small) = if b > a {
                (&their_tool, &our_tool, b, a)
            } else {
                (&our_tool, &their_tool, a, b)
            };
            text.push_str(&format!(
                " {more} added more than {less} here ({} cases against {}).",
                signed(big),
                signed(small)
            ));
            if b > a {
                text.push_str(&format!(" Why did {their_tool} help more on your run?"));
            }
        }
    }
    clip(text)
}

/// A note a reader shows, with the text it rendered from verified results.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Shown {
    pub id: String,
    pub author: String,
    /// The author's first eight hex characters.
    pub author_tag: String,
    /// This player's own agent said it.
    pub mine: bool,
    /// It answers another note.
    pub answer: bool,
    /// Whose note it answers, by tag, when it answers one.
    pub answers_tag: Option<String>,
    pub text: String,
    pub created_at: u64,
}

/// Why a note isn't shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A cited result, or the quoted note's, hasn't arrived yet.
    Waiting,
    /// A cited result isn't a valid result by the note's author, or an
    /// answer quotes something that isn't an opening note.
    Ungrounded,
}

/// Every result a note cites or depends on, for the reader to fetch: its
/// own sources, and for an answer, the quoted note.
#[must_use]
pub fn wanted(note: &Note) -> Vec<String> {
    let mut out = note.sources.clone();
    if let Some((id, _)) = &note.answers {
        out.push(id.clone());
    }
    out
}

/// Checks `note` against verified results and the other notes, and renders
/// the text Verse shows for it.
///
/// # Errors
///
/// [`Refusal::Waiting`] while something it cites is missing, and
/// [`Refusal::Ungrounded`] when it cites what it may not.
pub fn check(
    note: &Note,
    notes: &BTreeMap<String, Note>,
    publications: &BTreeMap<String, Publication>,
    names: &Names,
    me: &str,
) -> Result<Shown, Refusal> {
    let own = |id: &String| -> Result<&Publication, Refusal> {
        let p = publications.get(id).ok_or(Refusal::Waiting)?;
        if p.trainer() == note.author {
            Ok(p)
        } else {
            Err(Refusal::Ungrounded)
        }
    };
    let ours = note.sources.first().map(own).transpose()?;
    let text = match &note.answers {
        None => render_open(ours.ok_or(Refusal::Ungrounded)?, names),
        Some((quoted, peer)) => {
            let opener = notes.get(quoted).ok_or(Refusal::Waiting)?;
            if !opener.opens() || &opener.author != peer {
                return Err(Refusal::Ungrounded);
            }
            let their_source = opener.sources.first().ok_or(Refusal::Ungrounded)?;
            let theirs = publications.get(their_source).ok_or(Refusal::Waiting)?;
            if theirs.trainer() != opener.author {
                return Err(Refusal::Ungrounded);
            }
            if let Some(ours) = ours
                && ours.suite_release.id != theirs.suite_release.id
            {
                return Err(Refusal::Ungrounded);
            }
            render_answer(ours, theirs, names)
        }
    };
    Ok(Shown {
        id: note.id.clone(),
        author: note.author.clone(),
        author_tag: short(&note.author),
        mine: note.author == me,
        answer: note.answers.is_some(),
        answers_tag: note.answers.as_ref().map(|(_, peer)| short(peer)),
        text,
        created_at: note.created_at,
    })
}

/// Our newest result that isn't a check of another, among `publications`.
#[must_use]
pub fn newest_own<'a>(
    publications: &'a BTreeMap<String, Publication>,
    me: &str,
    suite: Option<&str>,
) -> Option<&'a Publication> {
    publications
        .values()
        .filter(|p| p.trainer() == me && p.checks.is_none())
        .filter(|p| suite.is_none_or(|s| p.suite_release.id == s))
        .max_by(|a, b| a.created_at.cmp(&b.created_at).then(b.id.cmp(&a.id)))
}

/// What the agent knows when it decides whether to speak.
pub struct Context<'a> {
    /// This player's public key.
    pub me: &'a str,
    /// The player switched on **Compare notes**.
    pub opted_in: bool,
    /// The player stands in the Gym.
    pub here: bool,
    /// Other players standing in the Gym now, by public key.
    pub peers: &'a BTreeSet<String>,
    /// Every note read, by ID, including our own.
    pub notes: &'a BTreeMap<String, Note>,
    /// Notes that passed [`check`], by ID.
    pub shown: &'a BTreeSet<String>,
    pub publications: &'a BTreeMap<String, Publication>,
}

/// How often an agent speaks: the rules in the module's constants. Our own
/// notes on the relay count too, so a restart doesn't speak again early.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    sent: VecDeque<u64>,
    answered: BTreeSet<String>,
    last_to: BTreeMap<String, u64>,
}

impl Policy {
    /// Records a note this agent sent at `at`.
    pub fn record(&mut self, plan: &Plan, at: u64) {
        self.sent.push_back(at);
        if let Plan::Answer { note, peer, .. } = plan {
            self.answered.insert(note.clone());
            self.last_to.insert(peer.clone(), at);
        }
    }

    /// What to say now, if anything.
    #[must_use]
    pub fn next(&mut self, context: &Context<'_>, now: u64) -> Option<Plan> {
        if !context.opted_in || !context.here {
            return None;
        }
        // Our own notes on the relay, including ones from before a restart.
        let mine: Vec<&Note> = context
            .notes
            .values()
            .filter(|n| n.author == context.me)
            .collect();
        for note in &mine {
            if let Some((quoted, peer)) = &note.answers {
                self.answered.insert(quoted.clone());
                let last = self.last_to.entry(peer.clone()).or_default();
                *last = (*last).max(note.created_at);
            }
        }
        let latest = mine
            .iter()
            .map(|n| n.created_at)
            .chain(self.sent.iter().copied())
            .max();
        while self.sent.front().is_some_and(|at| at + 3600 <= now) {
            self.sent.pop_front();
        }
        let in_hour = mine
            .iter()
            .filter(|n| n.created_at + 3600 > now)
            .count()
            .max(self.sent.len());
        if in_hour >= MAX_PER_HOUR {
            return None;
        }
        // Answer a fresh opening note from a trainer here, once.
        let mut openers: Vec<&Note> = context
            .notes
            .values()
            .filter(|n| {
                n.opens()
                    && n.author != context.me
                    && context.shown.contains(&n.id)
                    && context.peers.contains(&n.author)
                    && n.created_at + ANSWER_WINDOW >= now
                    && n.created_at <= now + FUTURE_SKEW
                    && !self.answered.contains(&n.id)
                    && self
                        .last_to
                        .get(&n.author)
                        .is_none_or(|at| at + PEER_INTERVAL <= now)
            })
            .collect();
        openers.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        if let Some(opener) = openers.first() {
            let suite = opener
                .sources
                .first()
                .and_then(|id| context.publications.get(id))
                .map(|p| p.suite_release.id.as_str());
            let ours = suite
                .and_then(|s| newest_own(context.publications, context.me, Some(s)))
                .map(|p| p.id.clone());
            return Some(Plan::Answer {
                note: opener.id.clone(),
                peer: opener.author.clone(),
                ours,
            });
        }
        // Open, when someone else is here and we've been quiet a while.
        if context.peers.iter().any(|p| p != context.me)
            && latest.is_none_or(|at| at + OPEN_INTERVAL <= now)
        {
            let ours = newest_own(context.publications, context.me, None)?;
            return Some(Plan::Open {
                ours: ours.id.clone(),
            });
        }
        None
    }
}

#[cfg(test)]
#[path = "gym_notes_tests.rs"]
mod tests;
