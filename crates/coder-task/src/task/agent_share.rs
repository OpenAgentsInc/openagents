//! A workshop agent's knowledge drafts (`docs/verse/generative-agents.md`,
//! item 6): a checked insight that is a general lesson becomes a NIP-KB
//! entry draft that the owner may publish.
//!
//! Agents share what they learned as cited knowledge entries, not free
//! chat. After a reflection stores insights (`agent_reflect`), [`share`]
//! considers each one:
//!
//! 1. Only an active insight the agent wrote is considered, and only when
//!    every record it rests on resolves to journal rows that exist. An
//!    insight that rests on a note or a preference, which is what the
//!    owner told her or how the owner wants work done, stays private.
//! 1. Jev answers the two Noul questions in
//!    `questions/insight-share.json`: whether the insight is a general
//!    lesson, and whether it states a fact about the owner. A fact about
//!    the owner is never drafted, and neither is a lesson below the gate.
//! 1. One model call writes the entry: an `environment`, `edge-case`, or
//!    `slip` with a title, summary, `applies_when`, tags, details, and how
//!    to check it.
//! 1. Code builds the entry as a candidate whose `provenance.cites` names
//!    every journal row behind the insight with that row's digest, and
//!    checks it ([`check_draft`]): it parses, the knowledge base's lint
//!    passes, the secret screen passes the whole file, and every citation
//!    names a journal row of this agent that exists and still holds what
//!    was cited.
//!
//! A draft is a file in `agents/NAME/kb-drafts/`, and each draft and each
//! insight kept private is journaled. Nothing here signs or publishes, and
//! this module has no key or relay to do it with: publishing is the
//! owner's action, through `microcoder kb publish --dir DIR ID`, which
//! signs with the owner's knowledge key. Other agents read a published
//! entry through `kb sync` and their own trust file, where an untrusted
//! author's entry is a candidate at most (`knowledge::remote::load`).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use knowledge::lint::{Corpus, lint};
use serde::Deserialize;

use super::agent::{Entry, Kind, Store};
use super::agent_memory::{Author, Memory, MemoryEntry, MemoryKind, MemoryState};
use super::agent_recall::{Body, Record, Ref};
use super::agent_reflect::{Reply, Writer, shown_line};
use crate::questions::{Fill, Set};

/// The directory under the agent's own where drafts wait for the owner.
pub const DRAFTS: &str = "kb-drafts";
/// The gate: the insight is a general lesson.
pub const GENERAL: &str = "general";
/// The insight states a fact about the owner.
pub const ABOUT_OWNER: &str = "about_owner";
/// The entry kinds a draft may be.
pub const KINDS: [knowledge::Kind; 3] = [
    knowledge::Kind::Environment,
    knowledge::Kind::EdgeCase,
    knowledge::Kind::Slip,
];
/// Hex characters of a cited row's digest that a citation carries.
pub const CITE_DIGEST: usize = 16;
/// The longest slug an entry ID takes from its title.
pub const SLUG_MAX: usize = 60;
/// The most records a chain of memory entries may pass through before a
/// draft gives up on it.
const WALK_MAX: usize = 64;
/// How a drafting run's record starts in the journal.
pub const RUN_PREFIX: &str = "knowledge drafting";

const SET_JSON: &str = include_str!("../../../../questions/insight-share.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the insight-share set parses");
    set.validate()
        .expect("the insight-share set is one this host asks");
    set
});

/// The insight-share question set.
#[must_use]
pub fn share_set() -> &'static Set {
    &SET
}

/// The probability at or above which question `id` reads as yes.
#[must_use]
pub fn threshold(id: &str) -> f64 {
    SET.decisions
        .get(id)
        .and_then(|decision| decision.threshold)
        .map_or(0.5, jev::decision::Threshold::value)
}

/// Jev's answers for one insight.
#[derive(Clone, Debug, PartialEq)]
pub struct Lesson {
    /// The probability the insight is a general lesson.
    pub general: f64,
    /// The probability it states a fact about the owner.
    pub about_owner: f64,
    pub model: String,
}

/// Asks whether an insight is a lesson to share.
pub trait Judge {
    /// # Errors
    /// When nothing answered; the insight isn't drafted.
    fn judge(&mut self, agent: &str, insight: &str, cited: &[&Record]) -> Result<Lesson, String>;
}

/// What drafting runs on.
pub struct Services {
    pub writer: Box<dyn Writer>,
    pub judge: Box<dyn Judge>,
    /// The benchmark tasks the lint keeps a draft from naming or quoting.
    pub corpus: Corpus,
}

impl Services {
    /// The agent's model through the capacity book, Jev from the decision
    /// profile, and the installed benchmark corpus.
    ///
    /// # Errors
    /// When no runtime starts, or Jev isn't set up: an insight nothing can
    /// judge is never drafted.
    pub fn live(store: &Store) -> Result<Self, String> {
        super::sales::privacy::model_available(store)?;
        let client = crate::decision::from_env()
            .map_err(|e| format!("Jev: {e}"))?
            .ok_or("Jev isn't set up, so no insight can be judged for sharing")?;
        Ok(Self {
            writer: Box::new(super::agent::LiveModel::new()?),
            judge: Box::new(JevJudge::new(client)?),
            corpus: Corpus::read(&knowledge::lint::default_corpora()),
        })
    }
}

/// Makes the [`Services`] for one agent's drafting.
pub type ServicesFactory = Arc<dyn Fn(&Store) -> Result<Services, String> + Send + Sync>;

/// The live services, or, in a unit test, a refusal: no model runs there.
#[must_use]
pub fn default_factory() -> ServicesFactory {
    if cfg!(test) {
        Arc::new(|_: &Store| Err("no drafting model runs in a unit test".to_string()))
    } else {
        Arc::new(Services::live)
    }
}

/// Jev answers `questions/insight-share.json`.
pub struct JevJudge {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevJudge {
    /// # Errors
    /// When the runtime doesn't start.
    pub fn new(client: jev::Client) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start a runtime: {e}"))?;
        Ok(Self { client, runtime })
    }
}

impl Judge for JevJudge {
    fn judge(&mut self, agent: &str, insight: &str, cited: &[&Record]) -> Result<Lesson, String> {
        let request = judge_request(agent, insight, cited)?;
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        let noul = |id: &str| match response.answers.get(id) {
            Some(jev::Answer::Noul(answer)) => Ok(answer.noul),
            _ => Err(format!("Jev didn't answer `{id}` in {}", SET.id)),
        };
        Ok(Lesson {
            general: noul(GENERAL)?,
            about_owner: noul(ABOUT_OWNER)?,
            model: response.model.clone(),
        })
    }
}

/// The decide request for one insight: the reflection's state shape.
///
/// # Errors
/// When the state is larger than the set's policy admits.
pub fn judge_request(
    agent: &str,
    insight: &str,
    cited: &[&Record],
) -> Result<jev::SystemOneRequest, String> {
    let state = super::agent_reflect::verify_state(agent, insight, cited);
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    Ok(jev::SystemOneRequest::new(state, SET.build(&Fill::None)?))
}

/// `agents/NAME/kb-drafts`.
#[must_use]
pub fn drafts_dir(store: &Store) -> PathBuf {
    store.dir().join(DRAFTS)
}

/// The command the owner runs to publish draft `id` from `dir`.
#[must_use]
pub fn publish_command(dir: &Path, id: &str) -> String {
    format!(
        "microcoder kb publish --dir {} --relay RELAY {id}",
        dir.display()
    )
}

/// What a draft records about its insight: `NAME memory:ID`.
#[must_use]
pub fn written_from(agent: &str, insight: u64) -> String {
    format!("{agent} memory:{insight}")
}

/// The citation of journal row `pos`: `NAME journal:POS DATE sha256:HEX`,
/// the digest being the first [`CITE_DIGEST`] hex characters of the row's
/// record digest (its kind and text).
#[must_use]
pub fn cite(agent: &str, pos: usize, row: &Entry) -> String {
    let record = Record {
        reference: Ref::Journal(pos),
        body: Body::Journal(row.clone()),
    };
    let digest = record.digest();
    format!(
        "{agent} journal:{pos} {} sha256:{}",
        knowledge::date(row.at),
        &digest[..CITE_DIGEST.min(digest.len())]
    )
}

/// The agent, journal position, and digest a citation names, when it is
/// one [`cite`] wrote.
#[must_use]
pub fn parse_cite(text: &str) -> Option<(String, usize, String)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let [agent, reference, _date, digest] = words.as_slice() else {
        return None;
    };
    let Some(Ref::Journal(pos)) = Ref::parse(reference) else {
        return None;
    };
    let digest = digest.strip_prefix("sha256:")?;
    Some(((*agent).to_string(), pos, digest.to_string()))
}

/// The journal rows behind `reference`: the row itself, or, for a memory
/// entry, the rows behind each of its sources. An insight that rests on
/// a note or a preference, which the owner wrote or accepted, is refused,
/// and so is any reference that doesn't exist.
///
/// # Errors
/// Why the reference can't be followed to journal rows.
pub fn rows_behind(
    reference: Ref,
    journal: &HashMap<usize, &Entry>,
    memory: &HashMap<u64, &MemoryEntry>,
) -> Result<BTreeSet<usize>, String> {
    fn walk(
        reference: Ref,
        journal: &HashMap<usize, &Entry>,
        memory: &HashMap<u64, &MemoryEntry>,
        seen: &mut BTreeSet<u64>,
        rows: &mut BTreeSet<usize>,
    ) -> Result<(), String> {
        match reference {
            Ref::Journal(pos) => {
                if !journal.contains_key(&pos) {
                    return Err(format!("it rests on {reference}, which doesn't exist"));
                }
                rows.insert(pos);
                Ok(())
            }
            Ref::Memory(id) => {
                let entry = memory
                    .get(&id)
                    .ok_or_else(|| format!("it rests on {reference}, which doesn't exist"))?;
                if matches!(entry.kind, MemoryKind::Note | MemoryKind::Preference) {
                    return Err(format!(
                        "it rests on {reference}, a {} about the owner",
                        entry.kind.word()
                    ));
                }
                if !seen.insert(id) || seen.len() > WALK_MAX {
                    return Err(format!("its sources loop or run too deep at {reference}"));
                }
                let mut any = false;
                for source in &entry.sources {
                    let Some(inner) = Ref::parse(source.trim()) else {
                        continue;
                    };
                    walk(inner, journal, memory, seen, rows)?;
                    any = true;
                }
                if !any {
                    return Err(format!("{reference} cites no journal row"));
                }
                Ok(())
            }
        }
    }
    let mut rows = BTreeSet::new();
    walk(reference, journal, memory, &mut BTreeSet::new(), &mut rows)?;
    Ok(rows)
}

/// An entry as the model writes it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Written {
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub applies_when: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub details: String,
    pub check: String,
}

/// The draft call's system text and prompt for `insight` over the rows
/// behind it.
#[must_use]
pub fn draft_prompt(agent: &str, insight: &str, rows: &[Record]) -> (String, String) {
    let system = format!(
        "You are {agent}, a workshop agent, writing a knowledge base entry that other agents on \
         other computers will read. Run no commands: finish on this step and put only the JSON \
         asked for in your reply."
    );
    let lines: String = rows.iter().map(shown_line).collect();
    let user = format!(
        "Your checked insight: {insight}\n\nThe journal rows it rests on:\n{lines}\nWrite it as a \
         knowledge base entry: a general lesson another agent can use elsewhere. Say only what \
         the rows show. Don't name your owner or any person, this computer's paths, task or issue \
         numbers, dates, or a benchmark task. `kind` is `environment` (how a class of environment \
         or build behaves), `edge-case` (an input or state that breaks common code), or `slip` (a \
         mistake agents make and how to avoid it). The summary is one or two sentences. Reply \
         with JSON: {{\"kind\": \"environment\", \"title\": \"...\", \"summary\": \"...\", \
         \"applies_when\": \"...\", \"tags\": [\"...\"], \"details\": \"...\", \"check\": \
         \"...\"}}"
    );
    (system, user)
}

/// The entry in a draft call's reply.
///
/// # Errors
/// When the reply isn't the JSON asked for, names another kind, or leaves
/// a field empty.
pub fn parse_written(text: &str) -> Result<Written, String> {
    let start = text.find('{').ok_or("the reply holds no JSON object")?;
    let end = text.rfind('}').ok_or("the reply holds no JSON object")?;
    if end < start {
        return Err("the reply holds no JSON object".into());
    }
    let written: Written = serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the reply isn't an entry: {e}"))?;
    kind_of(&written.kind)?;
    for (field, value) in [
        ("title", &written.title),
        ("summary", &written.summary),
        ("applies_when", &written.applies_when),
        ("details", &written.details),
        ("check", &written.check),
    ] {
        if value.trim().is_empty() {
            return Err(format!("the entry's {field} is empty"));
        }
    }
    Ok(written)
}

fn kind_of(word: &str) -> Result<knowledge::Kind, String> {
    knowledge::Kind::parse(word.trim())
        .filter(|kind| KINDS.contains(kind))
        .ok_or_else(|| format!("`{word}` isn't environment, edge-case, or slip"))
}

/// The ID slug of `title`: lowercase letters and digits, hyphens between
/// words, at most [`SLUG_MAX`] characters.
#[must_use]
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out.chars().take(SLUG_MAX).collect();
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The entry file for `written`, drafted by `agent` from insight `insight`
/// and citing `rows`.
///
/// # Errors
/// When the kind isn't one a draft may be, or the title gives no ID.
pub fn build(
    agent: &str,
    insight: u64,
    written: &Written,
    rows: &[(usize, &Entry)],
) -> Result<knowledge::Entry, String> {
    let kind = kind_of(&written.kind)?;
    let clean = |text: &str| super::agent::ascii(text.trim());
    let title = clean(&written.title);
    let name = slug(&title);
    if name.is_empty() {
        return Err("the title gives no entry ID".into());
    }
    let id = format!("{agent}.{name}");
    if !nostr::kb::valid_entry_id(&id) {
        return Err(format!("`{id}` isn't an entry ID"));
    }
    let tags: Vec<String> = written
        .tags
        .iter()
        .map(|t| slug(t))
        .filter(|t| !t.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(12)
        .collect();
    Ok(knowledge::Entry {
        id,
        version: 1,
        kind,
        title,
        summary: clean(&written.summary),
        tags,
        applies_when: clean(&written.applies_when),
        status: knowledge::Status::Candidate,
        author: agent.to_string(),
        written_from: vec![written_from(agent, insight)],
        cites: rows
            .iter()
            .map(|(pos, row)| cite(agent, *pos, row))
            .collect(),
        evidence: Vec::new(),
        answer: None,
        ui: None,
        body: format!(
            "## Details\n\n{}\n\n## How to check\n\n{}",
            clean(&written.details),
            clean(&written.check)
        ),
        digest: String::new(),
    })
}

/// Checks a draft file `text` of `agent` against her `journal`: it parses
/// as a candidate of a kind a draft may be, the knowledge base's lint
/// passes it against `corpus`, the secret screen passes the whole file,
/// and it cites at least one row, each citation naming a journal row of
/// this agent that exists and still has the digest cited.
///
/// # Errors
/// The first check that fails.
pub fn check_draft(
    text: &str,
    agent: &str,
    journal: &HashMap<usize, &Entry>,
    corpus: &Corpus,
    screen: &secret_screen::Screen,
) -> Result<knowledge::Entry, String> {
    let entry = knowledge::Entry::parse(text).map_err(|e| format!("it doesn't parse: {e}"))?;
    if !KINDS.contains(&entry.kind) {
        return Err(format!("its kind {} isn't one a draft may be", entry.kind));
    }
    if entry.status != knowledge::Status::Candidate {
        return Err("a draft is a candidate".into());
    }
    if let Some(problem) = lint(std::slice::from_ref(&entry), corpus).first() {
        return Err(format!("the lint refused it: {}", problem.message));
    }
    if let Err(why) = screen.check(text) {
        return Err(format!("the secret screen refused it: {why}"));
    }
    if entry.cites.is_empty() {
        return Err("it cites no journal row".into());
    }
    for cited in &entry.cites {
        let (who, pos, digest) =
            parse_cite(cited).ok_or_else(|| format!("`{cited}` isn't a journal row citation"))?;
        if who != agent {
            return Err(format!("`{cited}` cites another agent's journal"));
        }
        let row = journal
            .get(&pos)
            .ok_or_else(|| format!("it cites journal:{pos}, which doesn't exist"))?;
        if cite(agent, pos, row) != *cited {
            return Err(format!(
                "it cites journal:{pos} as sha256:{digest}, which isn't what that row holds"
            ));
        }
    }
    Ok(entry)
}

/// One insight's drafted entry.
#[derive(Clone, Debug)]
pub struct Draft {
    pub insight: u64,
    pub entry: knowledge::Entry,
    /// The file, as it is written.
    pub text: String,
    /// The journal rows it cites.
    pub rows: Vec<usize>,
    pub lesson: Lesson,
}

/// What drafting decided about one insight.
#[derive(Clone, Debug)]
pub enum Outcome {
    Drafted(Draft),
    /// Not drafted, with why.
    Kept {
        insight: u64,
        why: String,
    },
}

/// A drafting run's result, before anything is written.
#[derive(Clone, Debug, Default)]
pub struct Shared {
    pub outcomes: Vec<Outcome>,
    /// The models that wrote, in call order without repeats.
    pub models: Vec<String>,
    pub known_usd: f64,
    /// Calls that reported no cost.
    pub unmetered: usize,
}

impl Shared {
    /// The cost, when every call reported one.
    #[must_use]
    pub fn usd(&self) -> Option<f64> {
        (self.unmetered == 0).then_some(self.known_usd)
    }

    /// The drafts made.
    pub fn drafts(&self) -> impl Iterator<Item = &Draft> {
        self.outcomes.iter().filter_map(|o| match o {
            Outcome::Drafted(draft) => Some(draft),
            Outcome::Kept { .. } => None,
        })
    }
}

/// What drafting reads.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub agent: &'a str,
    /// The journal with positions, oldest first.
    pub journal: &'a [(usize, Entry)],
    /// Every memory entry, in any state.
    pub memory: &'a [MemoryEntry],
    /// Insights drafts already exist for, by memory ID.
    pub drafted: &'a BTreeSet<u64>,
}

/// Considers each insight in `insights` for a draft with `services`;
/// writes nothing.
#[must_use]
pub fn share(
    inputs: &Inputs<'_>,
    insights: &[u64],
    services: &mut Services,
    screen: &secret_screen::Screen,
) -> Shared {
    let journal: HashMap<usize, &Entry> = inputs.journal.iter().map(|(p, e)| (*p, e)).collect();
    let memory: HashMap<u64, &MemoryEntry> = inputs.memory.iter().map(|e| (e.id, e)).collect();
    let mut shared = Shared::default();
    for &id in insights {
        let outcome = match one(inputs, id, &journal, &memory, services, screen, &mut shared) {
            Ok(draft) => Outcome::Drafted(draft),
            Err(why) => Outcome::Kept { insight: id, why },
        };
        shared.outcomes.push(outcome);
    }
    shared
}

fn one(
    inputs: &Inputs<'_>,
    id: u64,
    journal: &HashMap<usize, &Entry>,
    memory: &HashMap<u64, &MemoryEntry>,
    services: &mut Services,
    screen: &secret_screen::Screen,
    shared: &mut Shared,
) -> Result<Draft, String> {
    let insight = memory
        .get(&id)
        .ok_or_else(|| format!("memory:{id} doesn't exist"))?;
    if insight.kind != MemoryKind::Insight
        || insight.state != MemoryState::Active
        || insight.author != Author::Agent
    {
        return Err(format!(
            "memory:{id} is a {} {}, and only a stored insight is drafted",
            insight.state.word(),
            insight.kind.word()
        ));
    }
    if inputs.drafted.contains(&id) {
        return Err("it is drafted already".into());
    }
    if insight.sources.is_empty() {
        return Err("it cites no record".into());
    }
    let rows = rows_behind(Ref::Memory(id), journal, memory)?;
    if let Err(why) = screen.check(&insight.text) {
        return Err(format!("the secret screen refused it: {why}"));
    }
    let records: Vec<Record> = rows
        .iter()
        .map(|pos| Record {
            reference: Ref::Journal(*pos),
            body: Body::Journal(journal[pos].clone()),
        })
        .collect();
    let cited: Vec<&Record> = records.iter().collect();
    let lesson = services
        .judge
        .judge(inputs.agent, &insight.text, &cited)
        .map_err(|why| format!("Jev couldn't judge it: {why}"))?;
    if lesson.about_owner >= threshold(ABOUT_OWNER) {
        return Err(format!(
            "it is about the owner (Jev {:.2}), so it stays private",
            lesson.about_owner
        ));
    }
    if lesson.general < threshold(GENERAL) {
        return Err(format!(
            "it isn't a general lesson (Jev {:.2}, under {:.2})",
            lesson.general,
            threshold(GENERAL)
        ));
    }
    let (system, user) = draft_prompt(inputs.agent, &insight.text, &records);
    let reply: Reply = services.writer.write(&system, &user)?;
    if !shared.models.contains(&reply.model) {
        shared.models.push(reply.model.clone());
    }
    match reply.usd {
        Some(usd) => shared.known_usd += usd,
        None => shared.unmetered += 1,
    }
    let written = parse_written(&reply.text)?;
    let behind: Vec<(usize, &Entry)> = rows.iter().map(|pos| (*pos, journal[pos])).collect();
    let entry = build(inputs.agent, id, &written, &behind)?;
    let text = entry.render();
    let entry = check_draft(&text, inputs.agent, journal, &services.corpus, screen)?;
    Ok(Draft {
        insight: id,
        entry,
        text,
        rows: rows.into_iter().collect(),
        lesson,
    })
}

/// The drafts waiting in `dir`, in name order. A file that doesn't read
/// as an entry is left out.
#[must_use]
pub fn read_drafts(dir: &Path) -> Vec<knowledge::Entry> {
    if !dir.is_dir() {
        return Vec::new();
    }
    knowledge::Base::read(dir).0
}

/// The drafts waiting in `store`'s drafts directory, as a device reads
/// them, each with the command that publishes it.
/// # Errors
/// When private draft custody or customer copy screening is unavailable.
pub fn draft_rows(store: &Store) -> Result<Vec<coder_host::access::agent::DraftRow>, String> {
    let dir = drafts_dir(store);
    let entries = super::sales::privacy::read_agent_drafts(store)?;
    Ok(entries
        .into_iter()
        .filter_map(|text| knowledge::Entry::parse(&text).ok())
        .map(|entry| coder_host::access::agent::DraftRow {
            publish: publish_command(&dir, &entry.id),
            kind: entry.kind.to_string(),
            title: entry.title,
            id: entry.id,
        })
        .collect())
}

/// The insights `entries` were drafted from, by memory ID.
#[must_use]
pub fn drafted(agent: &str, entries: &[knowledge::Entry]) -> BTreeSet<u64> {
    let prefix = format!("{agent} memory:");
    entries
        .iter()
        .flat_map(|e| &e.written_from)
        .filter_map(|w| w.strip_prefix(&prefix).and_then(|n| n.parse().ok()))
        .collect()
}

fn refs(rows: &[usize]) -> String {
    rows.iter()
        .map(|p| format!("journal:{p}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes `shared`: each draft as `<drafts>/<id>.md`, a journal row per
/// draft with the command that would publish it, a journal row per insight
/// kept private with why, and the run record last. A draft whose file
/// already exists is kept, not replaced. Returns the paths written.
///
/// # Errors
/// When the journal cannot be written.
pub fn apply(store: &Store, shared: &Shared, now: u64) -> Result<Vec<PathBuf>, String> {
    for draft in shared.drafts() {
        super::sales::privacy::check_agent_draft(store, &draft.text)?;
        super::sales::privacy::check_agent_copy(
            store,
            &serde_json::to_string(&draft.entry).map_err(|_| "agent draft serialization failed")?,
        )?;
    }
    let dir = drafts_dir(store);
    let mut written = Vec::new();
    let mut kept = 0;
    for outcome in &shared.outcomes {
        let line = match outcome {
            Outcome::Drafted(draft) => match write_new(&dir, &draft.entry.id, &draft.text) {
                Ok(path) => {
                    written.push(path);
                    format!(
                        "drafted knowledge entry {} ({}) from insight entry {}; cites {}; \
                         nothing is published until the owner runs: {}",
                        draft.entry.id,
                        draft.entry.kind,
                        draft.insight,
                        refs(&draft.rows),
                        publish_command(&dir, &draft.entry.id)
                    )
                }
                Err(why) => {
                    kept += 1;
                    format!(
                        "did not draft insight entry {} as a knowledge entry: {why}",
                        draft.insight
                    )
                }
            },
            Outcome::Kept { insight, why } => {
                kept += 1;
                format!("did not draft insight entry {insight} as a knowledge entry: {why}")
            }
        };
        store.append(&Entry::new(now, Kind::Memory, &line))?;
    }
    if !shared.outcomes.is_empty() {
        let cost = match shared.usd() {
            Some(usd) => format!("${usd:.4}"),
            None => format!(
                "unknown ({} calls reported none; at least ${:.4})",
                shared.unmetered, shared.known_usd
            ),
        };
        let models = if shared.models.is_empty() {
            "none".to_string()
        } else {
            shared.models.join(", ")
        };
        store.append(&Entry::new(
            now,
            Kind::Memory,
            &format!(
                "{RUN_PREFIX}: model {models}; cost {cost}; drafted {}, kept {kept}",
                written.len()
            ),
        ))?;
    }
    Ok(written)
}

fn write_new(dir: &Path, id: &str, text: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    super::prepare_directory(dir)
        .map_err(|_| "private knowledge draft directory is unavailable")?;
    let path = dir.join(format!("{id}.md"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!("a draft named {id} already waits"));
        }
        Err(e) => return Err(format!("cannot write {}: {e}", path.display())),
    };
    file.write_all(text.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

impl Memory {
    /// Considers each insight in `insights` for a knowledge draft and
    /// writes the result ([`apply`]).
    ///
    /// # Errors
    /// When the files cannot be read or the journal written.
    pub fn share(
        &self,
        services: &mut Services,
        insights: &[u64],
        now: u64,
    ) -> Result<(Shared, Vec<PathBuf>), String> {
        let store = self.store();
        super::sales::privacy::model_available(store)?;
        let journal = store.journal_rows()?;
        let memory = self.entries()?;
        let drafted = drafted(store.name(), &read_drafts(&drafts_dir(store)));
        let shared = share(
            &Inputs {
                agent: store.name(),
                journal: &journal,
                memory: &memory,
                drafted: &drafted,
            },
            insights,
            services,
            self.screen(),
        );
        let written = apply(store, &shared, now)?;
        Ok((shared, written))
    }
}

/// A judge that answers every insight the same, and a writer that
/// replies the same, for tests; each counts its calls.
#[cfg(test)]
pub(crate) mod fake {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Judge, Lesson, Record, Reply, Writer};

    pub(crate) struct Fixed {
        pub general: f64,
        pub about_owner: f64,
        pub calls: Arc<AtomicUsize>,
    }

    impl Judge for Fixed {
        fn judge(&mut self, _: &str, _: &str, _: &[&Record]) -> Result<Lesson, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Lesson {
                general: self.general,
                about_owner: self.about_owner,
                model: "fake-jev".into(),
            })
        }
    }

    pub(crate) struct Same {
        pub text: String,
        pub calls: Arc<AtomicUsize>,
    }

    impl Writer for Same {
        fn write(&mut self, _: &str, _: &str) -> Result<Reply, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Reply {
                text: self.text.clone(),
                model: "fake-writer".into(),
                usd: Some(0.01),
            })
        }
    }
}

#[cfg(test)]
#[path = "agent_share_tests.rs"]
mod tests;
