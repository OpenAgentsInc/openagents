//! A workshop agent's reflection with checked citations
//! (`docs/verse/generative-agents.md`, item 2).
//!
//! A reflection runs the paper's procedure over the scored memory stream:
//!
//! 1. One model call reads the [`NEWEST`] newest records and asks the
//!    [`QUESTIONS`] most salient questions.
//! 1. For each question, `agent_recall::retrieve` returns the [`SHOWN`]
//!    best records, each shown with its `journal:POS` or `memory:ID`
//!    reference.
//! 1. One model call per question writes at most [`INSIGHTS`] insights,
//!    each with a `because` list of references.
//!
//! Code then checks every insight ([`check`]): each reference exists and
//! was shown for that question, the reflection depth stays within
//! [`DEPTH_MAX`], the secret screen passes the text, and Jev answers the
//! two Noul questions in `questions/insight-support.json`. An insight that
//! passes is a [`MemoryKind::Insight`] entry with its citations in
//! `sources`; one that states how the owner wants work done is a
//! `preference` candidate instead, which no briefing carries until the
//! owner accepts it. One that fails is journaled as unverified, with why.
//!
//! [`reflect`] is a pure function of the records and three services (a
//! [`Writer`], a [`Verify`], and the stream's own `agent_recall::Services`),
//! so tests run it on scripted fakes. [`Memory::reflect`] reads an agent's
//! files, runs it, and writes the result; the `reflect` standing job
//! (`agent_jobs`) calls that.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};

use super::agent::{Entry, Kind, Store};
use super::agent_memory::{Author, ENTRY_MAX, Memory, MemoryEntry, MemoryKind};
use super::agent_recall::{self, Body, Record, Ref, ScoreRow, Scores};
use crate::questions::{Fill, Set};

#[path = "agent_reflect_live.rs"]
mod live;
pub use live::JevVerify;

/// The newest records the question call reads.
pub const NEWEST: usize = 100;
/// The questions one reflection asks.
pub const QUESTIONS: usize = 3;
/// The records retrieval shows for each question.
pub const SHOWN: usize = 15;
/// The most insights one question keeps; the rest are dropped unread.
pub const INSIGHTS: usize = 5;
/// The deepest reflection tree: an insight citing only records is depth 1,
/// and one citing a depth-3 insight is refused, so every chain ends in
/// journal rows or entries the owner or the host wrote.
pub const DEPTH_MAX: u32 = 3;
/// Summed importance since the last reflection that triggers an early one:
/// the paper's 150, until the importance scale is measured.
pub const EARLY_THRESHOLD: u32 = 150;
/// The most early reflections a day.
pub const EARLY_PER_DAY: u32 = 2;
/// How a reflection's run record starts in the journal.
pub const RUN_PREFIX: &str = "reflection run";

/// The gate: the insight follows from the cited records alone.
pub const SUPPORTED: &str = "supported";
/// The insight states how the owner wants work done.
pub const PREFERENCE: &str = "preference";

const SET_JSON: &str = include_str!("../../../../questions/insight-support.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the insight-support set parses");
    set.validate()
        .expect("the insight-support set is one this host asks");
    set
});

/// The insight-support question set.
#[must_use]
pub fn support_set() -> &'static Set {
    &SET
}

/// The probability at or above which question `id` reads as yes, from the
/// set's decision block, or the midpoint.
#[must_use]
pub fn threshold(id: &str) -> f64 {
    SET.decisions
        .get(id)
        .and_then(|decision| decision.threshold)
        .map_or(0.5, jev::decision::Threshold::value)
}

/// One model call's reply.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub model: String,
    /// Dollars, or `None` when the provider reported no cost.
    pub usd: Option<f64>,
}

/// The model a reflection writes with.
pub trait Writer {
    /// # Errors
    /// When no model answered.
    fn write(&mut self, system: &str, prompt: &str) -> Result<Reply, String>;
}

/// One job on the shared-compute pool: `(system, prompt)` to a reply.
pub type PoolAsk = Box<dyn FnMut(&str, &str) -> Result<Reply, String> + Send>;

/// A writer that sends its low-risk text jobs to the shared-compute pool
/// (`openagents pylon route on`) and falls back to the agent's own model
/// when no pylon answers.
pub struct PoolWriter {
    pub pool: PoolAsk,
    pub fallback: Option<Box<dyn Writer + Send>>,
}

impl Writer for PoolWriter {
    fn write(&mut self, system: &str, prompt: &str) -> Result<Reply, String> {
        match (self.pool)(system, prompt) {
            Ok(reply) => Ok(reply),
            Err(pool) => match self.fallback.as_mut() {
                Some(fallback) => fallback.write(system, prompt),
                None => Err(format!(
                    "the pool didn't answer ({pool}), and no model is set up"
                )),
            },
        }
    }
}

/// Jev's answers for one insight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Support {
    /// The probability the insight follows from the cited records alone.
    pub supported: f64,
    /// The probability it states how the owner wants work done.
    pub preference: f64,
    #[serde(default)]
    pub model: String,
}

/// Asks whether an insight is supported.
pub trait Verify {
    /// # Errors
    /// When nothing answered; the insight is dropped as unverified.
    fn verify(&mut self, agent: &str, insight: &str, cited: &[&Record]) -> Result<Support, String>;
}

/// What a reflection runs on.
pub struct Services {
    pub writer: Box<dyn Writer>,
    pub verify: Box<dyn Verify>,
    pub recall: agent_recall::Services,
}

impl Services {
    /// The agent's model through the capacity book, Jev from the decision
    /// profile, and the stream's live services.
    ///
    /// # Errors
    /// When no runtime starts, or Jev isn't set up: an insight nothing can
    /// check is never written, so the reflection doesn't run.
    pub fn live(store: &Store) -> Result<Self, String> {
        super::sales::privacy::model_available(store)?;
        let client = crate::decision::from_env()
            .map_err(|e| format!("Jev: {e}"))?
            .ok_or("Jev isn't set up, so a reflection's insights can't be checked")?;
        Ok(Self {
            writer: Box::new(super::agent::LiveModel::new()?),
            verify: Box::new(JevVerify::new(client)?),
            recall: agent_recall::Services::live(store),
        })
    }
}

/// Makes the [`Services`] for one agent's reflection.
pub type ServicesFactory = Arc<dyn Fn(&Store) -> Result<Services, String> + Send + Sync>;

/// The live services, or, in a unit test, a refusal: no model runs there.
#[must_use]
pub fn default_factory() -> ServicesFactory {
    if cfg!(test) {
        Arc::new(|_: &Store| Err("no reflection model runs in a unit test".to_string()))
    } else {
        Arc::new(Services::live)
    }
}

/// What code decided about one insight.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Stored as an insight entry of this depth.
    Stored { depth: u32 },
    /// Supported and preference-shaped: a candidate for the owner.
    Proposed { depth: u32 },
    /// Unverified, with why.
    Dropped(String),
}

/// One insight as the model wrote it and as code judged it.
#[derive(Clone, Debug, PartialEq)]
pub struct Insight {
    pub text: String,
    /// The references as the model wrote them.
    pub because: Vec<String>,
    pub verdict: Verdict,
    pub support: Option<Support>,
}

/// One question, what retrieval showed for it, and its insights.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub question: String,
    pub shown: Vec<Ref>,
    pub insights: Vec<Insight>,
    /// Why the insight call failed, when it did.
    pub failed: Option<String>,
}

/// A reflection's result, before anything is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Reflection {
    /// Unix seconds.
    pub at: u64,
    /// What triggered it, such as `nightly`.
    pub trigger: String,
    /// How many records the question call read.
    pub read: usize,
    pub asked: Vec<Asked>,
    /// The models that wrote, in call order without repeats.
    pub models: Vec<String>,
    /// What the calls reported, in dollars.
    pub known_usd: f64,
    /// Calls that reported no cost.
    pub unmetered: usize,
    /// Score rows retrieval set, for the sidecar.
    pub scored: Vec<ScoreRow>,
}

impl Reflection {
    /// The cost, when every call reported one.
    #[must_use]
    pub fn usd(&self) -> Option<f64> {
        (self.unmetered == 0).then_some(self.known_usd)
    }

    fn count(&self, pick: fn(&Verdict) -> bool) -> usize {
        self.asked
            .iter()
            .flat_map(|asked| &asked.insights)
            .filter(|insight| pick(&insight.verdict))
            .count()
    }

    /// The journal text of the run record: questions, records read, model,
    /// cost, and what came of the insights.
    #[must_use]
    pub fn run_record(&self) -> String {
        let cost = match self.usd() {
            Some(usd) => format!("${usd:.4}"),
            None => format!(
                "unknown ({} calls reported none; at least ${:.4})",
                self.unmetered, self.known_usd
            ),
        };
        let models = if self.models.is_empty() {
            "none".to_string()
        } else {
            self.models.join(", ")
        };
        format!(
            "{RUN_PREFIX} ({}): {} questions from the {} newest records, {} shown each; model {models}; \
             cost {cost}; stored {}, proposed {}, dropped {}",
            self.trigger,
            self.asked.len(),
            self.read,
            SHOWN,
            self.count(|v| matches!(v, Verdict::Stored { .. })),
            self.count(|v| matches!(v, Verdict::Proposed { .. })),
            self.count(|v| matches!(v, Verdict::Dropped(_))),
        )
    }
}

/// What a reflection reads.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub agent: &'a str,
    pub workspace: &'a str,
    /// Unix seconds.
    pub now: u64,
    /// What triggered it.
    pub trigger: &'a str,
    /// The journal with positions, oldest first.
    pub journal: &'a [(usize, Entry)],
    /// Every memory entry, in any state.
    pub memory: &'a [MemoryEntry],
}

/// A record as the prompts show it: its reference, then its briefing line.
#[must_use]
pub fn shown_line(record: &Record) -> String {
    format!(
        "[{}] {}",
        record.reference,
        record.line().trim_start_matches("- ")
    )
}

/// The question call's system text and prompt over `records`.
#[must_use]
pub fn questions_prompt(agent: &str, records: &[Record]) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, reflecting on your own recent work. Run no \
         commands: finish on this step and put only the JSON asked for in your reply."
    );
    let lines: String = records.iter().map(shown_line).collect();
    let user = format!(
        "Your {} newest records, oldest first, each with its reference:\n{lines}\nGiven only these \
         records, what are the {QUESTIONS} most salient high-level questions you can answer about \
         your work and how the owner wants it done? Reply with JSON: {{\"questions\": [\"...\"]}}",
        records.len()
    );
    (system, user)
}

/// The insight call's system text and prompt for `question` over `shown`.
#[must_use]
pub fn insights_prompt(agent: &str, question: &str, shown: &[Record]) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, reflecting on your own recent work. Run no \
         commands: finish on this step and put only the JSON asked for in your reply."
    );
    let lines: String = shown.iter().map(shown_line).collect();
    let user = format!(
        "Records for the question, each with its reference:\n{lines}\nQuestion: {question}\n\nWhat \
         at most {INSIGHTS} high-level insights can you infer from these records alone? For each, \
         list in `because` the references of the records it rests on, using only references shown \
         above. Code checks every reference, and an insight the records don't support is dropped. \
         Reply with JSON: {{\"insights\": [{{\"text\": \"...\", \"because\": [\"journal:12\", \
         \"memory:4\"]}}]}}"
    );
    (system, user)
}

fn json_object(text: &str) -> Result<serde_json::Value, String> {
    let start = text.find('{').ok_or("the reply holds no JSON object")?;
    let end = text.rfind('}').ok_or("the reply holds no JSON object")?;
    if end < start {
        return Err("the reply holds no JSON object".into());
    }
    serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the reply's JSON doesn't read: {e}"))
}

/// The questions in a question call's reply, at most [`QUESTIONS`].
///
/// # Errors
/// When the reply isn't the JSON asked for or names no question.
pub fn parse_questions(text: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Questions {
        questions: Vec<String>,
    }
    let parsed: Questions = serde_json::from_value(json_object(text)?)
        .map_err(|e| format!("the reply isn't a questions list: {e}"))?;
    let questions: Vec<String> = parsed
        .questions
        .into_iter()
        .map(|q| super::agent::ascii(q.trim()))
        .filter(|q| !q.trim().is_empty())
        .take(QUESTIONS)
        .collect();
    if questions.is_empty() {
        return Err("the reply names no question".into());
    }
    Ok(questions)
}

/// One insight as the model wrote it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Written {
    pub text: String,
    #[serde(default)]
    pub because: Vec<String>,
}

/// The insights in an insight call's reply, at most [`INSIGHTS`].
///
/// # Errors
/// When the reply isn't the JSON asked for.
pub fn parse_insights(text: &str) -> Result<Vec<Written>, String> {
    #[derive(Deserialize)]
    struct Insights {
        insights: Vec<Written>,
    }
    let parsed: Insights = serde_json::from_value(json_object(text)?)
        .map_err(|e| format!("the reply isn't an insights list: {e}"))?;
    Ok(parsed
        .insights
        .into_iter()
        .filter(|w| !w.text.trim().is_empty())
        .take(INSIGHTS)
        .collect())
}

/// The depth of the record `reference` names in a reflection tree: 0 for a
/// journal row or a non-insight entry, and one more than the deepest
/// record an insight cites. A cycle or a missing entry counts as too deep.
#[must_use]
pub fn depth(reference: Ref, memory: &HashMap<u64, &MemoryEntry>) -> u32 {
    fn walk(reference: Ref, memory: &HashMap<u64, &MemoryEntry>, seen: &mut BTreeSet<u64>) -> u32 {
        let Ref::Memory(id) = reference else {
            return 0;
        };
        let Some(entry) = memory.get(&id) else {
            return DEPTH_MAX + 1;
        };
        if entry.kind != MemoryKind::Insight {
            return 0;
        }
        if !seen.insert(id) {
            return DEPTH_MAX + 1;
        }
        let deepest = entry
            .sources
            .iter()
            .map(|s| Ref::parse(s).map_or(0, |r| walk(r, memory, seen)))
            .max()
            .unwrap_or(0);
        seen.remove(&id);
        deepest.saturating_add(1)
    }
    walk(reference, memory, &mut BTreeSet::new())
}

/// What [`check`] reads besides the insight.
pub struct Context<'a> {
    pub agent: &'a str,
    /// Every journal position and every memory ID that exists.
    pub journal: &'a BTreeSet<usize>,
    pub memory: &'a HashMap<u64, &'a MemoryEntry>,
    /// The records shown for this question.
    pub shown: &'a [Record],
    pub screen: &'a secret_screen::Screen,
}

/// Checks one insight, in order: it cites something; each reference reads,
/// exists, and was shown for this question; its depth is within
/// [`DEPTH_MAX`]; the secret screen passes it; and Jev finds it supported.
/// Then a preference-shaped one is proposed, and the rest are stored.
#[must_use]
pub fn check(written: &Written, context: &Context<'_>, verify: &mut dyn Verify) -> Insight {
    let text = super::agent::ascii(written.text.trim());
    let mut insight = Insight {
        text: text.clone(),
        because: written.because.clone(),
        verdict: Verdict::Dropped(String::new()),
        support: None,
    };
    let drop = |mut insight: Insight, why: String| {
        insight.verdict = Verdict::Dropped(why);
        insight
    };
    if text.len() > ENTRY_MAX {
        return drop(insight, format!("it is over {ENTRY_MAX} bytes"));
    }
    if written.because.is_empty() {
        return drop(insight, "it cites no record".into());
    }
    let mut cited = Vec::new();
    for raw in &written.because {
        let Some(reference) = Ref::parse(raw.trim()) else {
            return drop(insight, format!("`{raw}` isn't a record reference"));
        };
        let exists = match reference {
            Ref::Journal(pos) => context.journal.contains(&pos),
            Ref::Memory(id) => context.memory.contains_key(&id),
        };
        if !exists {
            return drop(
                insight,
                format!("it cites {reference}, which doesn't exist"),
            );
        }
        let Some(record) = context.shown.iter().find(|r| r.reference == reference) else {
            return drop(
                insight,
                format!("it cites {reference}, which wasn't shown for this question"),
            );
        };
        if !cited.iter().any(|r: &&Record| r.reference == reference) {
            cited.push(record);
        }
    }
    let depth = 1 + cited
        .iter()
        .map(|r| depth(r.reference, context.memory))
        .max()
        .unwrap_or(0);
    if depth > DEPTH_MAX {
        return drop(
            insight,
            format!("its reflection depth {depth} is over the cap of {DEPTH_MAX}"),
        );
    }
    if let Err(why) = context.screen.check(&text) {
        return drop(insight, format!("the secret screen refused it: {why}"));
    }
    let support = match verify.verify(context.agent, &text, &cited) {
        Ok(support) => support,
        Err(why) => return drop(insight, format!("Jev couldn't check it: {why}")),
    };
    insight.support = Some(support.clone());
    if support.supported < threshold(SUPPORTED) {
        return drop(
            insight,
            format!(
                "the cited records don't support it (Jev {:.2}, under {:.2})",
                support.supported,
                threshold(SUPPORTED)
            ),
        );
    }
    insight.verdict = if support.preference >= threshold(PREFERENCE) {
        Verdict::Proposed { depth }
    } else {
        Verdict::Stored { depth }
    };
    insight
}

/// Runs a reflection over `inputs` with `services`; writes nothing.
///
/// # Errors
/// When there are no records, or the question call fails or names no
/// question. A failed insight call is recorded on its question.
pub fn reflect(
    inputs: &Inputs<'_>,
    known: &HashMap<String, ScoreRow>,
    services: &mut Services,
    screen: &secret_screen::Screen,
) -> Result<Reflection, String> {
    let mut records = agent_recall::candidates(inputs.journal, inputs.memory);
    records.retain(|r| r.at() <= inputs.now);
    if records.is_empty() {
        return Err("there is nothing to reflect on".into());
    }
    records.sort_by_key(|r| (r.at(), r.reference));
    let newest: Vec<Record> = records
        .iter()
        .skip(records.len().saturating_sub(NEWEST))
        .cloned()
        .collect();
    let mut reflection = Reflection {
        at: inputs.now,
        trigger: inputs.trigger.into(),
        read: newest.len(),
        asked: Vec::new(),
        models: Vec::new(),
        known_usd: 0.0,
        unmetered: 0,
        scored: Vec::new(),
    };
    let account = |reflection: &mut Reflection, reply: &Reply| {
        if !reflection.models.contains(&reply.model) {
            reflection.models.push(reply.model.clone());
        }
        match reply.usd {
            Some(usd) => reflection.known_usd += usd,
            None => reflection.unmetered += 1,
        }
    };
    let (system, user) = questions_prompt(inputs.agent, &newest);
    let reply = services.writer.write(&system, &user)?;
    account(&mut reflection, &reply);
    let questions = parse_questions(&reply.text)?;

    let journal: BTreeSet<usize> = inputs.journal.iter().map(|(pos, _)| *pos).collect();
    let memory: HashMap<u64, &MemoryEntry> = inputs.memory.iter().map(|e| (e.id, e)).collect();
    let mut known = known.clone();
    for question in questions {
        let (shown, scored) = agent_recall::retrieve(
            &agent_recall::Inputs {
                agent: inputs.agent,
                request: &question,
                workspace: inputs.workspace,
                now: inputs.now,
                journal: inputs.journal,
                memory: inputs.memory,
            },
            &known,
            &mut services.recall,
            SHOWN,
        );
        for row in &scored {
            known.insert(row.record.clone(), row.clone());
        }
        reflection.scored.extend(scored);
        let mut asked = Asked {
            question: question.clone(),
            shown: shown.iter().map(|r| r.reference).collect(),
            insights: Vec::new(),
            failed: None,
        };
        let (system, user) = insights_prompt(inputs.agent, &question, &shown);
        let written = services.writer.write(&system, &user).and_then(|reply| {
            account(&mut reflection, &reply);
            parse_insights(&reply.text)
        });
        match written {
            Ok(written) => {
                let context = Context {
                    agent: inputs.agent,
                    journal: &journal,
                    memory: &memory,
                    shown: &shown,
                    screen,
                };
                asked.insights = written
                    .iter()
                    .map(|w| check(w, &context, services.verify.as_mut()))
                    .collect();
            }
            Err(why) => asked.failed = Some(why),
        }
        reflection.asked.push(asked);
    }
    Ok(reflection)
}

/// What writing a reflection added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// New insight entries.
    pub stored: Vec<u64>,
    /// New preference candidates.
    pub proposed: Vec<u64>,
    pub dropped: usize,
}

fn refs(list: &[String]) -> String {
    if list.is_empty() {
        "nothing".into()
    } else {
        list.join(", ")
    }
}

/// Writes `reflection`: a journal row per question with the references it
/// showed, an entry and a journal row per stored or proposed insight, a
/// journal row per dropped insight with why, and the run record last.
///
/// # Errors
/// When the journal cannot be written.
pub fn apply(memory: &Memory, reflection: &Reflection) -> Result<Applied, String> {
    let store = memory.store();
    let at = reflection.at;
    let mut applied = Applied::default();
    for (n, asked) in reflection.asked.iter().enumerate() {
        let n = n + 1;
        let shown: Vec<String> = asked.shown.iter().map(ToString::to_string).collect();
        store.append(&Entry::new(
            at,
            Kind::Memory,
            &format!(
                "reflection question {n}: {}; shown {}",
                asked.question,
                refs(&shown)
            ),
        ))?;
        if let Some(why) = &asked.failed {
            store.append(&Entry::new(
                at,
                Kind::Memory,
                &format!("reflection question {n} wrote no insights: {why}"),
            ))?;
        }
        for insight in &asked.insights {
            let cites = refs(&insight.because);
            let (kind, depth) = match &insight.verdict {
                Verdict::Stored { depth } => (MemoryKind::Insight, *depth),
                Verdict::Proposed { depth } => (MemoryKind::Preference, *depth),
                Verdict::Dropped(why) => {
                    applied.dropped += 1;
                    store.append(&Entry::new(
                        at,
                        Kind::Memory,
                        &format!(
                            "dropped an unverified insight (question {n}): {why}: {}; cites {cites}",
                            insight.text
                        ),
                    ))?;
                    continue;
                }
            };
            match memory.add(
                kind,
                Author::Agent,
                &insight.text,
                insight.because.clone(),
                at,
            ) {
                Ok(id) => {
                    let word = if kind == MemoryKind::Insight {
                        applied.stored.push(id);
                        format!("insight entry {id}")
                    } else {
                        applied.proposed.push(id);
                        format!(
                            "preference entry {id}, proposed from an insight and waiting for the owner,"
                        )
                    };
                    store.append(&Entry::new(
                        at,
                        Kind::Memory,
                        &format!("{word} (question {n}, depth {depth}) cites {cites}"),
                    ))?;
                }
                Err(why) => {
                    applied.dropped += 1;
                    store.append(&Entry::new(
                        at,
                        Kind::Memory,
                        &format!(
                            "dropped an unverified insight (question {n}): {why}: {}; cites {cites}",
                            insight.text
                        ),
                    ))?;
                }
            }
        }
    }
    store.append(&Entry::new(at, Kind::Memory, &reflection.run_record()))?;
    Ok(applied)
}

/// When the last reflection ran: the newest run record in `journal`.
#[must_use]
pub fn last_reflection(journal: &[(usize, Entry)]) -> Option<u64> {
    journal
        .iter()
        .rev()
        .find(|(_, e)| e.kind == Kind::Memory && e.text.starts_with(RUN_PREFIX))
        .map(|(_, e)| e.at)
}

/// Summed importance of the records written after `since`, as the stream
/// reads it without asking Jev ([`agent_recall::importance_of`]).
#[must_use]
pub fn importance_since(
    journal: &[(usize, Entry)],
    memory: &[MemoryEntry],
    known: &HashMap<String, ScoreRow>,
    since: u64,
) -> f64 {
    agent_recall::candidates(journal, memory)
        .iter()
        .filter(|r| r.at() > since)
        .map(|r| agent_recall::importance_of(r, known))
        .sum()
}

/// Summed importance since the last reflection or `floor`, whichever is
/// later, from the agent's journal, memory, and score sidecar.
///
/// # Errors
/// When the journal or the memory cannot be read.
pub fn pressure(store: &Store, floor: u64) -> Result<f64, String> {
    let journal = store.journal_rows()?;
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes()).entries()?;
    let known = Scores::of(store).load().unwrap_or_default();
    let since = last_reflection(&journal).unwrap_or(0).max(floor);
    Ok(importance_since(&journal, &memory, &known, since))
}

impl Memory {
    /// Runs a reflection over this agent's journal and memory at `now`,
    /// appends the score rows retrieval set, and writes the result
    /// ([`apply`]).
    ///
    /// # Errors
    /// When the files cannot be read or written, or [`reflect`] fails.
    pub fn reflect(
        &self,
        services: &mut Services,
        screen: &secret_screen::Screen,
        trigger: &str,
        now: u64,
    ) -> Result<(Reflection, Applied), String> {
        let store = self.store();
        super::sales::privacy::model_available(store)?;
        super::sales::privacy::check_agent_copy(store, trigger)?;
        let journal = store.journal_rows()?;
        let memory = self.entries()?;
        let sidecar = Scores::of(store);
        let known = sidecar.load().unwrap_or_default();
        let workspace = store
            .load()
            .ok()
            .flatten()
            .map(|record| record.workspace)
            .unwrap_or_default();
        let reflection = reflect(
            &Inputs {
                agent: store.name(),
                workspace: &workspace,
                now,
                trigger,
                journal: &journal,
                memory: &memory,
            },
            &known,
            services,
            screen,
        )?;
        let _ = sidecar.append(&reflection.scored);
        let applied = apply(self, &reflection)?;
        Ok((reflection, applied))
    }
}

/// A recorded reflection: the replies a model gave and the answers Jev
/// gave, replayed by [`Scripted`] and [`Recorded`] with no model. The
/// interview's full arm and the end-to-end test run one over the phase A
/// fixture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub schema: String,
    /// The fixture digest it was written against.
    pub fixture_digest: String,
    /// When it runs, Unix seconds.
    pub at: u64,
    pub model: String,
    /// Dollars per call.
    pub usd: f64,
    pub questions: Vec<ScriptQuestion>,
}

/// One question of a [`Script`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptQuestion {
    pub question: String,
    pub insights: Vec<ScriptInsight>,
}

/// One insight of a [`Script`], with Jev's recorded answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptInsight {
    pub text: String,
    pub because: Vec<String>,
    pub supported: f64,
    pub preference: f64,
    /// What code should decide, for the test: `stored`, `proposed`, or
    /// the start of the drop reason.
    pub expect: String,
}

/// The script's schema.
pub const SCRIPT_SCHEMA: &str = "openagents.agent-reflection-script.v1";

const ALICE_V1_SCRIPT: &str = include_str!("../../fixtures/agent-reflect/alice-interview-v1.json");

impl Script {
    /// The recorded reflection over the phase A fixture.
    ///
    /// # Errors
    /// When it doesn't parse.
    pub fn alice_v1() -> Result<Self, String> {
        let script: Self = serde_json::from_str(ALICE_V1_SCRIPT)
            .map_err(|e| format!("the recorded reflection doesn't read: {e}"))?;
        if script.schema != SCRIPT_SCHEMA {
            return Err(format!("the recorded reflection isn't {SCRIPT_SCHEMA}"));
        }
        Ok(script)
    }

    /// Services that replay this script, with offline retrieval.
    #[must_use]
    pub fn services(&self) -> Services {
        Services {
            writer: Box::new(Scripted::new(self.clone())),
            verify: Box::new(Recorded::new(self)),
            recall: agent_recall::Services::offline(),
        }
    }
}

/// Replays a [`Script`]'s replies: the questions first, then each
/// question's insights, matched by the question in the prompt.
#[derive(Clone, Debug)]
pub struct Scripted {
    script: Script,
    /// Every prompt it was given, for a test or a trace.
    pub prompts: Vec<String>,
}

impl Scripted {
    #[must_use]
    pub fn new(script: Script) -> Self {
        Self {
            script,
            prompts: Vec::new(),
        }
    }
}

impl Writer for Scripted {
    fn write(&mut self, _system: &str, prompt: &str) -> Result<Reply, String> {
        self.prompts.push(prompt.to_string());
        let text = if let Some(question) = self
            .script
            .questions
            .iter()
            .find(|q| prompt.contains(&format!("Question: {}\n", q.question)))
        {
            let insights: Vec<Written> = question
                .insights
                .iter()
                .map(|i| Written {
                    text: i.text.clone(),
                    because: i.because.clone(),
                })
                .collect();
            serde_json::json!({ "insights": insights }).to_string()
        } else {
            let questions: Vec<&str> = self
                .script
                .questions
                .iter()
                .map(|q| q.question.as_str())
                .collect();
            serde_json::json!({ "questions": questions }).to_string()
        };
        Ok(Reply {
            text,
            model: self.script.model.clone(),
            usd: Some(self.script.usd),
        })
    }
}

/// Replays a [`Script`]'s Jev answers by insight text.
#[derive(Clone, Debug)]
pub struct Recorded {
    answers: BTreeMap<String, Support>,
}

impl Recorded {
    #[must_use]
    pub fn new(script: &Script) -> Self {
        Self {
            answers: script
                .questions
                .iter()
                .flat_map(|q| &q.insights)
                .map(|i| {
                    (
                        super::agent::ascii(i.text.trim()),
                        Support {
                            supported: i.supported,
                            preference: i.preference,
                            model: "recorded".into(),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl Verify for Recorded {
    fn verify(&mut self, _: &str, insight: &str, _: &[&Record]) -> Result<Support, String> {
        self.answers
            .get(insight)
            .cloned()
            .ok_or_else(|| "no recorded answer for this insight".into())
    }
}

/// The state Jev reads for one insight.
#[must_use]
pub fn verify_state(agent: &str, insight: &str, cited: &[&Record]) -> serde_json::Value {
    let cited: Vec<serde_json::Value> = cited
        .iter()
        .map(|record| {
            let mut fields = serde_json::json!({
                "reference": record.reference.to_string(),
                "kind": record.kind(),
                "date": agent_recall::day(record.at()),
                "text": record.text(),
            });
            if let Body::Journal(Entry {
                status: Some(status),
                ..
            }) = &record.body
            {
                fields["status"] = (*status).into();
            }
            fields
        })
        .collect();
    serde_json::json!({ "agent": agent, "insight": insight, "cited": cited })
}

/// The decide request for one insight.
///
/// # Errors
/// When the state is larger than the set's policy admits.
pub fn verify_request(
    agent: &str,
    insight: &str,
    cited: &[&Record],
) -> Result<jev::SystemOneRequest, String> {
    let state = verify_state(agent, insight, cited);
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    Ok(jev::SystemOneRequest::new(state, SET.build(&Fill::None)?))
}

#[cfg(test)]
#[path = "agent_reflect_tests.rs"]
mod tests;
