//! Gym interviews of a workshop agent (`docs/verse/generative-agents.md`,
//! item 7): `coder interview`.
//!
//! A run asks each item of a pinned suite (`gym::interview`) about a frozen
//! fixture of the agent's journal and memory. An [`Arm`] builds the
//! briefing the agent would carry, an [`Answerer`] answers from it, and
//! each answer becomes a code-graded, receipt-chained row in a Gym store.
//!
//! Five arms exist, the paper's ablations plus today's code:
//! [`ArmName::Full`], the scored memory stream with the insights a recorded
//! reflection stored and the day plan; [`ArmName::NoReflection`], the
//! stream and the plan; [`ArmName::NoReflectionOrPlan`], the stream alone;
//! [`ArmName::NoMemory`], an empty briefing; and [`ArmName::WordOverlap`],
//! the baseline [`Memory::briefing`]. The day plan is a [`Planner`]:
//! [`StandingJobs`] reads the standing jobs and waiting tasks from the
//! fixture with no model, and [`DayPlans`] adds the blocks a recorded
//! morning draft makes through `agent_plan`'s checks.
//!
//! The [`FromBriefing`] and [`Canned`] answerers run with no model; [`Live`]
//! asks the agent's model through the capacity book, under a cost cap, and
//! only the command uses it. Items code doesn't check go to a
//! [`judge::Judge`]. [`round`] runs every arm on disjoint seed blocks,
//! compares the full arm with word overlap through `gym::ab`, and judges
//! the result with the digested `interview-v1` gate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gym::interview::{self as gi, ItemState};
use gym::row::DoorIdentity;
use gym::suite::{Item, LockedLedger, Partition, Spend, Suite};

use super::agent::{Entry, Kind, LiveModel, Model, Store};
use super::agent_memory::{Memory, MemoryEntry};

#[path = "agent_interview_judge.rs"]
pub mod judge;
pub use judge::{JevJudge, Judge, ScriptedJudge};

/// A fixture, parsed into the agent's own types and screened.
#[derive(Clone, Debug)]
pub struct Fixture {
    pub gym: gi::Fixture,
    /// The journal in file order; `journal:POS` is index `POS - 1`.
    pub journal: Vec<Entry>,
    pub memory: Vec<MemoryEntry>,
}

impl Fixture {
    /// Parses every line strictly and runs the secret screen on every text.
    ///
    /// # Errors
    /// When a line isn't a journal row or memory entry, or the screen
    /// refuses a text.
    pub fn parse(gym: gi::Fixture) -> Result<Self, String> {
        let screen = secret_screen::Screen::shapes();
        let mut journal = Vec::new();
        for (index, line) in gym.journal.lines().enumerate() {
            let entry: Entry = serde_json::from_str(line)
                .map_err(|e| format!("journal line {} doesn't read: {e}", index + 1))?;
            screen
                .check(&entry.text)
                .map_err(|why| format!("journal line {} fails the screen: {why}", index + 1))?;
            journal.push(entry);
        }
        let mut memory = Vec::new();
        for (index, line) in gym.memory.lines().enumerate() {
            let entry: MemoryEntry = serde_json::from_str(line)
                .map_err(|e| format!("memory line {} doesn't read: {e}", index + 1))?;
            screen
                .check(&entry.text)
                .map_err(|why| format!("memory entry {} fails the screen: {why}", entry.id))?;
            memory.push(entry);
        }
        Ok(Self {
            gym,
            journal,
            memory,
        })
    }

    /// The committed Alice fixture.
    ///
    /// # Errors
    /// When it doesn't load.
    pub fn alice_v1() -> Result<Self, String> {
        Self::parse(gi::alice_v1_fixture()?)
    }

    #[must_use]
    pub fn agent(&self) -> &str {
        &self.gym.manifest.agent
    }

    #[must_use]
    pub fn workspace(&self) -> &str {
        &self.gym.manifest.workspace
    }
}

/// What an arm hands the answerer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Briefing {
    pub text: String,
    /// The fixture records it carried, as `journal:POS` or `memory:ID`.
    pub carried: Vec<String>,
}

/// One question, as an arm and an answerer see it.
#[derive(Clone, Copy, Debug)]
pub struct Ask<'a> {
    pub item_id: &'a str,
    pub question: &'a str,
    /// When the interview happens, Unix seconds.
    pub as_of: u64,
}

/// Builds the briefing an agent carries into a question.
pub trait Arm {
    /// The arm's name, as rows record it.
    fn name(&self) -> &'static str;

    /// The briefing for `ask`, from what `fixture` held before `ask.as_of`.
    ///
    /// # Errors
    /// When the arm can't build one.
    fn brief(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String>;
}

/// The arms a run can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmName {
    /// An empty briefing: the paper's no-memory condition.
    NoMemory,
    /// The word-overlap baseline: [`Memory::briefing`], words shared with
    /// the question.
    WordOverlap,
    /// The scored memory stream (`agent_recall`) alone: recency,
    /// importance, and relevance over memory entries and journal rows. The
    /// paper's no-reflection-or-planning condition.
    NoReflectionOrPlan,
    /// The scored stream and the day plan, with no insights.
    NoReflection,
    /// The full architecture: the scored stream with the insights a
    /// reflection stored (`agent_reflect`), and the day plan.
    Full,
}

impl ArmName {
    pub const ALL: [Self; 5] = [
        Self::NoMemory,
        Self::WordOverlap,
        Self::NoReflectionOrPlan,
        Self::NoReflection,
        Self::Full,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoMemory => "no-memory",
            Self::WordOverlap => "word-overlap",
            Self::NoReflectionOrPlan => "no-reflection-or-plan",
            Self::NoReflection => "no-reflection",
            Self::Full => "full",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.as_str() == name)
    }

    /// The arm, keeping any files it needs under `scratch`.
    #[must_use]
    pub fn build(self, scratch: &Path) -> Box<dyn Arm> {
        match self {
            Self::NoMemory => Box::new(NoMemory),
            Self::WordOverlap => Box::new(WordOverlap {
                root: scratch.join(self.as_str()),
            }),
            Self::NoReflectionOrPlan => Box::new(NoReflectionOrPlan),
            Self::NoReflection => Box::new(NoReflection::new(Box::new(DayPlans::new(None)))),
            Self::Full => Box::new(Full::new(
                scratch.join(self.as_str()),
                Box::new(DayPlans::new(Some(scratch.join("full-plan")))),
            )),
        }
    }
}

/// The empty briefing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMemory;

impl Arm for NoMemory {
    fn name(&self) -> &'static str {
        ArmName::NoMemory.as_str()
    }

    fn brief(&mut self, _: &Fixture, _: &Ask<'_>) -> Result<Briefing, String> {
        Ok(Briefing::default())
    }
}

/// [`Memory::briefing`] over the fixture's memory as it stood at the
/// interview, in a scratch host root.
#[derive(Clone, Debug)]
pub struct WordOverlap {
    root: PathBuf,
}

impl Arm for WordOverlap {
    fn name(&self) -> &'static str {
        ArmName::WordOverlap.as_str()
    }

    fn brief(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        let store = Store::new(&self.root, fixture.agent())?;
        std::fs::create_dir_all(store.dir())
            .map_err(|e| format!("cannot create {}: {e}", store.dir().display()))?;
        let mut body = String::new();
        for entry in fixture.memory.iter().filter(|e| e.at < ask.as_of) {
            body.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
            body.push('\n');
        }
        let path = store.dir().join("memory.jsonl");
        std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let memory = Memory::new(store, secret_screen::Screen::shapes());
        let (text, ids) = memory.briefing(ask.question, fixture.workspace())?;
        Ok(Briefing {
            text,
            carried: ids.iter().map(|id| format!("memory:{id}")).collect(),
        })
    }
}

/// The scored stream (`agent_recall::recall`) over the fixture as it stood
/// at `ask`, with `insights` beside its memory, and no model: rule and
/// prior importance, BM25 relevance, and recency from the fixture's own
/// selection receipts.
fn stream_brief(fixture: &Fixture, ask: &Ask<'_>, insights: &[MemoryEntry]) -> Briefing {
    let journal: Vec<(usize, Entry)> = fixture
        .journal
        .iter()
        .enumerate()
        .filter(|(_, e)| e.at < ask.as_of)
        .map(|(i, e)| (i + 1, e.clone()))
        .collect();
    let memory: Vec<MemoryEntry> = fixture
        .memory
        .iter()
        .chain(insights)
        .filter(|e| e.at < ask.as_of)
        .cloned()
        .collect();
    let recall = super::agent_recall::recall(
        &super::agent_recall::Inputs {
            agent: fixture.agent(),
            request: ask.question,
            workspace: fixture.workspace(),
            now: ask.as_of,
            journal: &journal,
            memory: &memory,
        },
        &std::collections::HashMap::new(),
        &mut super::agent_recall::Services::offline(),
    );
    Briefing {
        text: recall.text,
        carried: recall.carried.iter().map(ToString::to_string).collect(),
    }
}

/// The day plan an agent carries into a question.
pub trait Planner {
    /// The plan as it stood at `ask.as_of`, from what `fixture` held then.
    ///
    /// # Errors
    /// When the planner can't build one.
    fn plan(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String>;
}

/// A day plan with no model: the standing jobs that are on, each in its
/// slot with what it runs, and the tasks that wait at the Merge station.
/// It reads only real work the journal records, as item 4 requires, and is
/// the planner until phase D's plan replaces it.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandingJobs;

/// The job a journal row about a standing job names, after `prefix`.
fn job_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.strip_prefix(prefix)?.split([' ', ',']).next()
}

impl Planner for StandingJobs {
    fn plan(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        struct Job {
            title: String,
            on: bool,
            slot: Option<u64>,
            runs: Option<(usize, String)>,
            added: usize,
        }
        let mut jobs: BTreeMap<String, Job> = BTreeMap::new();
        let mut waiting: BTreeMap<String, usize> = BTreeMap::new();
        for (index, entry) in fixture.journal.iter().enumerate() {
            if entry.at >= ask.as_of {
                break;
            }
            let pos = index + 1;
            let text = entry.text.as_str();
            if let Some(name) = job_after(text, "the owner added job ") {
                let title = text
                    .split_once('(')
                    .and_then(|(_, rest)| rest.split_once(')'))
                    .map_or_else(String::new, |(title, _)| title.to_string());
                jobs.insert(
                    name.to_string(),
                    Job {
                        title,
                        on: false,
                        slot: None,
                        runs: None,
                        added: pos,
                    },
                );
            } else if let Some(name) = job_after(text, "job ") {
                if let Some(job) = jobs.get_mut(name) {
                    if text.ends_with("turned on") {
                        job.on = true;
                    } else if text.ends_with("turned off") {
                        job.on = false;
                    } else if text.contains(" fired") {
                        job.slot = Some(entry.at % 86_400);
                    }
                }
            } else if entry.kind == Kind::Request
                && let Some(name) = entry.from.as_deref().and_then(|f| f.strip_prefix("job:"))
                && let Some(job) = jobs.get_mut(name)
                && !text.starts_with("A check failed")
            {
                job.runs = Some((pos, text.to_string()));
            } else if entry.kind == Kind::Task
                && let Some(task) = job_after(text, "task ")
            {
                if text.ends_with("waits at the Merge station") {
                    waiting.insert(task.to_string(), pos);
                } else if text.contains(" merged ") || text.contains(" rejected ") {
                    waiting.remove(task);
                }
            }
        }
        let mut slots: Vec<(u64, String, Vec<String>)> = Vec::new();
        for (name, job) in jobs.iter().filter(|(_, job)| job.on) {
            let Some(slot) = job.slot else { continue };
            let mut carried = vec![format!("journal:{}", job.added)];
            let runs = job.runs.as_ref().map_or("", |(pos, text)| {
                carried.push(format!("journal:{pos}"));
                text.as_str()
            });
            slots.push((
                slot,
                format!(
                    "- {:02}:{:02} {name} ({}): {runs}",
                    slot / 3600,
                    slot % 3600 / 60,
                    job.title
                ),
                carried,
            ));
        }
        slots.sort_by_key(|(slot, line, _)| (*slot, line.clone()));
        if slots.is_empty() && waiting.is_empty() {
            return Ok(Briefing::default());
        }
        let mut text = format!(
            "Your plan for {} (standing jobs, by their time of day):\n",
            &gym::eval::utc_from_unix(ask.as_of)[..10]
        );
        let mut carried = Vec::new();
        for (_, line, refs) in slots {
            text.push_str(&line);
            text.push('\n');
            carried.extend(refs);
        }
        for (task, pos) in waiting {
            text.push_str(&format!(
                "- waiting for the owner: task {task} waits at the Merge station\n"
            ));
            carried.push(format!("journal:{pos}"));
        }
        Ok(Briefing { text, carried })
    }
}

/// The recorded morning draft over the fixture.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanScript {
    pub schema: String,
    pub fixture_digest: String,
    /// When she plans, Unix seconds.
    pub at: u64,
    pub model: String,
    /// The draft call's reply.
    pub draft: serde_json::Value,
}

/// The plan script's schema.
pub const PLAN_SCRIPT_SCHEMA: &str = "openagents.agent-plan-script.v1";

const ALICE_V1_PLAN: &str = include_str!("../../fixtures/agent-plan/alice-interview-v1.json");

impl PlanScript {
    /// The recorded morning draft over the phase A fixture.
    ///
    /// # Errors
    /// When it doesn't parse.
    pub fn alice_v1() -> Result<Self, String> {
        let script: Self = serde_json::from_str(ALICE_V1_PLAN)
            .map_err(|e| format!("the recorded plan doesn't read: {e}"))?;
        if script.schema != PLAN_SCRIPT_SCHEMA {
            return Err(format!("the recorded plan isn't {PLAN_SCRIPT_SCHEMA}"));
        }
        Ok(script)
    }
}

/// Phase D's day plan (`agent_plan`): [`StandingJobs`]' jobs in their
/// slots, by code, and the blocks the recorded morning draft makes from her
/// accepted insights, kept only when `agent_plan::draft`'s checks pass
/// them. With insights (the full arm), they are the ones the recorded
/// reflection stores; without (the no-reflection arm), the draft has no
/// source to work and makes no call.
pub struct DayPlans {
    /// Where the recorded reflection runs, for the full arm.
    reflect: Option<PathBuf>,
    insights: Option<Vec<MemoryEntry>>,
}

impl DayPlans {
    /// A day plan over the recorded reflection's insights, run under
    /// `reflect`, or over none.
    #[must_use]
    pub fn new(reflect: Option<PathBuf>) -> Self {
        Self {
            reflect,
            insights: None,
        }
    }

    /// The plan the recorded draft makes over `fixture` at its morning.
    ///
    /// # Errors
    /// When the scripts don't pin this fixture, or the draft fails.
    pub fn made(&mut self, fixture: &Fixture) -> Result<super::agent_plan::Made, String> {
        use super::agent_plan as plan;
        let script = PlanScript::alice_v1()?;
        if script.fixture_digest != fixture.gym.manifest.digest {
            return Err(format!(
                "the recorded plan pins fixture {}, and this fixture is {}",
                script.fixture_digest, fixture.gym.manifest.digest
            ));
        }
        if self.insights.is_none() {
            self.insights = Some(match &self.reflect {
                Some(root) => reflect_fixture(fixture, root)?.1,
                None => Vec::new(),
            });
        }
        let memory: Vec<MemoryEntry> = self
            .insights
            .iter()
            .flatten()
            .filter(|e| e.at <= script.at)
            .cloned()
            .collect();
        let tree = world_tree::everglade();
        let known = world_tree::Known::new(fixture.agent(), tree);
        let inputs = plan::Inputs {
            agent: fixture.agent(),
            now: script.at,
            utc_offset: 0,
            jobs: &[],
            issues: &[],
            queued: &[],
            memory: &memory,
            tree,
            known: &known,
            bound: plan::HOUSE,
        };
        let mut writer = plan::Scripted::new([script.draft.to_string()]);
        plan::draft(&inputs, &mut writer, &secret_screen::Screen::shapes())
    }
}

impl Planner for DayPlans {
    fn plan(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        let jobs = StandingJobs.plan(fixture, ask)?;
        let made = self.made(fixture)?;
        let today = &gym::eval::utc_from_unix(ask.as_of)[..10];
        if made.plan.made_at > ask.as_of || made.plan.date != today || made.plan.idle() {
            return Ok(jobs);
        }
        let mut text = format!(
            "Your other blocks for {today} (drafted at {} from real work):\n",
            coder_host::access::day_plan::clock(super::agent_plan::local(made.plan.made_at, 0).1)
        );
        let mut carried = Vec::new();
        for block in &made.plan.blocks {
            text.push_str(&format!(
                "- {}-{} {} [{}]\n",
                coder_host::access::day_plan::clock(block.start),
                coder_host::access::day_plan::clock(block.end),
                block.title,
                block.source
            ));
            if block.source.starts_with("memory:") || block.source.starts_with("journal:") {
                carried.push(block.source.clone());
            }
        }
        Ok(joined(jobs, Briefing { text, carried }))
    }
}

/// `first` followed by `second`, with the records both carried.
fn joined(mut first: Briefing, second: Briefing) -> Briefing {
    if !second.text.is_empty() {
        if !first.text.is_empty() && !first.text.ends_with('\n') {
            first.text.push('\n');
        }
        first.text.push_str(&second.text);
    }
    for reference in second.carried {
        if !first.carried.contains(&reference) {
            first.carried.push(reference);
        }
    }
    first
}

/// The scored stream over the fixture alone: the paper's no-reflection,
/// no-planning condition.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoReflectionOrPlan;

impl Arm for NoReflectionOrPlan {
    fn name(&self) -> &'static str {
        ArmName::NoReflectionOrPlan.as_str()
    }

    fn brief(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        Ok(stream_brief(fixture, ask, &[]))
    }
}

/// The scored stream and the day plan: the paper's no-reflection
/// condition.
pub struct NoReflection {
    planner: Box<dyn Planner>,
}

impl NoReflection {
    #[must_use]
    pub fn new(planner: Box<dyn Planner>) -> Self {
        Self { planner }
    }
}

impl Arm for NoReflection {
    fn name(&self) -> &'static str {
        ArmName::NoReflection.as_str()
    }

    fn brief(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        let plan = self.planner.plan(fixture, ask)?;
        Ok(joined(stream_brief(fixture, ask, &[]), plan))
    }
}

/// The scored stream with the insights the recorded reflection
/// (`agent_reflect::Script::alice_v1`) stores when it runs over the
/// fixture in a scratch host root, through the same checks a live one
/// passes, and the day plan.
pub struct Full {
    root: PathBuf,
    insights: Option<Vec<MemoryEntry>>,
    planner: Box<dyn Planner>,
}

impl Full {
    #[must_use]
    pub fn new(root: PathBuf, planner: Box<dyn Planner>) -> Self {
        Self {
            root,
            insights: None,
            planner,
        }
    }

    /// The insight entries the recorded reflection stored, run once.
    ///
    /// # Errors
    /// When the script doesn't match the fixture or the reflection fails.
    pub fn insights(&mut self, fixture: &Fixture) -> Result<&[MemoryEntry], String> {
        if self.insights.is_none() {
            self.insights = Some(reflect_fixture(fixture, &self.root)?.1);
        }
        Ok(self.insights.as_deref().unwrap_or_default())
    }
}

/// Runs the recorded reflection over `fixture`, as it stood when the
/// script runs, in a scratch host root under `root`: the reflection and
/// the active insight entries it stored.
///
/// # Errors
/// When the script doesn't pin this fixture, or the reflection fails.
pub fn reflect_fixture(
    fixture: &Fixture,
    root: &Path,
) -> Result<(super::agent_reflect::Reflection, Vec<MemoryEntry>), String> {
    let script = super::agent_reflect::Script::alice_v1()?;
    if script.fixture_digest != fixture.gym.manifest.digest {
        return Err(format!(
            "the recorded reflection pins fixture {}, and this fixture is {}",
            script.fixture_digest, fixture.gym.manifest.digest
        ));
    }
    let store = Store::new(root, fixture.agent())?;
    std::fs::create_dir_all(store.dir())
        .map_err(|e| format!("cannot create {}: {e}", store.dir().display()))?;
    let lines = |rows: Vec<String>| rows.into_iter().map(|row| row + "\n").collect::<String>();
    let journal = fixture
        .journal
        .iter()
        .filter(|e| e.at <= script.at)
        .map(|e| serde_json::to_string(e).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let memory = fixture
        .memory
        .iter()
        .filter(|e| e.at <= script.at)
        .map(|e| serde_json::to_string(e).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    for (name, body) in [
        ("journal.jsonl", lines(journal)),
        ("memory.jsonl", lines(memory)),
    ] {
        let path = store.dir().join(name);
        std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    let screen = secret_screen::Screen::shapes();
    let memory = Memory::new(store, screen.clone());
    let mut services = script.services();
    let (reflection, _) = memory.reflect(&mut services, &screen, "recorded", script.at)?;
    let insights = memory
        .entries()?
        .into_iter()
        .filter(|e| {
            e.kind == super::agent_memory::MemoryKind::Insight
                && e.state == super::agent_memory::MemoryState::Active
        })
        .collect();
    Ok((reflection, insights))
}

impl Arm for Full {
    fn name(&self) -> &'static str {
        ArmName::Full.as_str()
    }

    fn brief(&mut self, fixture: &Fixture, ask: &Ask<'_>) -> Result<Briefing, String> {
        let insights = self.insights(fixture)?.to_vec();
        let plan = self.planner.plan(fixture, ask)?;
        Ok(joined(stream_brief(fixture, ask, &insights), plan))
    }
}

/// The system text and prompt an answerer reads.
#[must_use]
pub fn prompt(agent: &str, briefing: &Briefing, ask: &Ask<'_>) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, answering an interview question about your \
         own work. Answer from your memory briefing alone, in one or two plain sentences. If the \
         briefing doesn't hold the answer, say you don't remember. Run no commands: finish on \
         this step and put the answer in your reply."
    );
    let memory = if briefing.text.trim().is_empty() {
        "(empty)".to_string()
    } else {
        briefing.text.trim_end().to_string()
    };
    let user = format!(
        "Now: {}\n\nYour memory briefing:\n{memory}\n\nQuestion: {}",
        gym::eval::utc_from_unix(ask.as_of),
        ask.question
    );
    (system, user)
}

/// Answers an interview question.
pub trait Answerer {
    /// The answerer's kind, as rows record it in `door`.
    fn door(&self) -> &'static str;

    /// What answered last, as rows record it.
    fn identity(&self) -> DoorIdentity;

    /// The answer to `ask` from `briefing`.
    ///
    /// # Errors
    /// When nothing answered; the run records no row for the item.
    fn answer(&mut self, agent: &str, briefing: &Briefing, ask: &Ask<'_>)
    -> Result<String, String>;
}

/// The answer for an empty or unhelpful briefing.
pub const DONT_REMEMBER: &str = "I don't remember that.";

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

/// A deterministic answerer with no model: the briefing line that shares
/// the most words with the question, first on a tie.
#[derive(Clone, Copy, Debug, Default)]
pub struct FromBriefing;

impl Answerer for FromBriefing {
    fn door(&self) -> &'static str {
        "scripted"
    }

    fn identity(&self) -> DoorIdentity {
        DoorIdentity::hosted("from-briefing-v1")
    }

    fn answer(&mut self, _: &str, briefing: &Briefing, ask: &Ask<'_>) -> Result<String, String> {
        let asked = words(ask.question);
        let best = briefing
            .text
            .lines()
            .map(|line| (words(line).intersection(&asked).count(), line))
            .filter(|(shared, _)| *shared > 0)
            .fold(None::<(usize, &str)>, |best, candidate| match best {
                Some(best) if best.0 >= candidate.0 => Some(best),
                _ => Some(candidate),
            });
        Ok(best.map_or_else(
            || DONT_REMEMBER.to_string(),
            |(_, line)| line.trim_start_matches("- ").to_string(),
        ))
    }
}

/// Answers from a fixed map of item ID to answer, for tests.
#[derive(Clone, Debug, Default)]
pub struct Canned(pub BTreeMap<String, String>);

impl Answerer for Canned {
    fn door(&self) -> &'static str {
        "canned"
    }

    fn identity(&self) -> DoorIdentity {
        DoorIdentity::hosted("canned")
    }

    fn answer(&mut self, _: &str, _: &Briefing, ask: &Ask<'_>) -> Result<String, String> {
        Ok(self
            .0
            .get(ask.item_id)
            .cloned()
            .unwrap_or_else(|| DONT_REMEMBER.into()))
    }
}

/// One model call's reply: the text, the model that wrote it, and what
/// it cost in dollars, when the provider reported a cost.
pub type Response = (String, String, Option<f64>);

/// One model call that returns a reply and the model that wrote it.
pub trait Respond {
    /// # Errors
    /// When no model answered.
    fn respond(&mut self, system: &str, prompt: &str) -> Result<Response, String>;
}

impl Respond for LiveModel {
    fn respond(&mut self, system: &str, prompt: &str) -> Result<Response, String> {
        let action = self.next(system, prompt)?;
        let reply = if action.reply.trim().is_empty() {
            action.rationale
        } else {
            action.reply
        };
        Ok((reply, self.model.clone().unwrap_or_default(), self.usd))
    }
}

/// The agent's model answers, through [`prompt`], until what it has spent
/// reaches the cap.
pub struct Live<R> {
    model: R,
    last: String,
    /// Dollars the reported costs sum to.
    pub spent: f64,
    /// Calls whose provider reported no cost.
    pub unpriced: usize,
    cap: Option<f64>,
}

impl<R: Respond> Live<R> {
    #[must_use]
    pub fn new(model: R) -> Self {
        Self {
            model,
            last: String::new(),
            spent: 0.0,
            unpriced: 0,
            cap: None,
        }
    }

    /// Refuses further calls once the reported costs reach `usd`. A call
    /// with no reported cost counts as nothing, so the summary reports how
    /// many there were.
    #[must_use]
    pub fn capped(mut self, usd: f64) -> Self {
        self.cap = Some(usd);
        self
    }
}

impl<R: Respond> Answerer for Live<R> {
    fn door(&self) -> &'static str {
        "live"
    }

    fn identity(&self) -> DoorIdentity {
        DoorIdentity::hosted(self.last.clone())
    }

    fn answer(
        &mut self,
        agent: &str,
        briefing: &Briefing,
        ask: &Ask<'_>,
    ) -> Result<String, String> {
        if let Some(cap) = self.cap
            && self.spent >= cap
        {
            return Err(format!(
                "the cost cap of ${cap:.2} is spent (${:.4})",
                self.spent
            ));
        }
        let (system, user) = prompt(agent, briefing, ask);
        let (reply, model, usd) = self.model.respond(&system, &user)?;
        match usd {
            Some(usd) => self.spent += usd,
            None => self.unpriced += 1,
        }
        self.last = model;
        Ok(super::agent::plain(&reply))
    }
}

/// The items of one partition. The locked partition takes a ledger and a
/// reason, and the read is recorded before any item comes back.
///
/// # Errors
/// When the partition can't be read.
pub fn items<'a>(
    suite: &'a Suite,
    partition: Partition,
    locked: Option<(&LockedLedger, &Spend<'_>)>,
) -> Result<Vec<&'a Item>, String> {
    match (partition, locked) {
        (Partition::Locked, Some((ledger, spend))) => {
            ledger.read_locked(suite, spend).map_err(|e| e.to_string())
        }
        (Partition::Locked, None) => {
            Err("the locked partition is read once, with --ledger PATH and --reason TEXT".into())
        }
        (open, _) => suite.partition(open).map_err(|e| e.to_string()),
    }
}

/// What a run did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub arm: String,
    pub door: String,
    pub rows: usize,
    /// Rows code graded, and how many were right.
    pub checked: usize,
    pub correct: usize,
    /// Rows a judge read, and how many it found supported and embellished.
    pub judged: usize,
    pub supported: usize,
    pub embellished: usize,
    /// Rows whose briefing carried at least one of the item's sources.
    pub evidence: usize,
    /// Items with no row, and why.
    pub skipped: Vec<String>,
    /// Rows recorded without a judgment because the judge failed, and why.
    pub unjudged: Vec<String>,
}

/// Asks every item in `items`, appending one row each to `store`. An item
/// code doesn't check goes to `judge`, when there is one.
///
/// # Errors
/// When an arm fails or the store refuses a row; rows already appended stay.
#[allow(clippy::too_many_arguments)]
pub fn run(
    suite: &Suite,
    fixture: &Fixture,
    items: &[&Item],
    arm: &mut dyn Arm,
    answerer: &mut dyn Answerer,
    mut judge: Option<&mut (dyn Judge + '_)>,
    trial: Option<u64>,
    store: &gym::store::Store,
    clock: &dyn Fn() -> String,
) -> Result<Summary, String> {
    let mut summary = Summary {
        arm: arm.name().into(),
        door: answerer.door().into(),
        ..Summary::default()
    };
    for item in items {
        let state = ItemState::of(item)?;
        if state.fixture_digest != fixture.gym.manifest.digest {
            return Err(format!(
                "item {} pins fixture {}, and this fixture is {}",
                item.id, state.fixture_digest, fixture.gym.manifest.digest
            ));
        }
        let ask = Ask {
            item_id: &item.id,
            question: gi::question(item),
            as_of: state.as_of,
        };
        let briefing = arm.brief(fixture, &ask)?;
        let answer = match answerer.answer(fixture.agent(), &briefing, &ask) {
            Ok(answer) => answer,
            Err(why) => {
                summary.skipped.push(format!("{}: {why}", item.id));
                continue;
            }
        };
        let mut row = gi::Row::graded(
            suite,
            item,
            arm.name(),
            answerer.door(),
            answerer.identity(),
            trial,
            briefing.carried.clone(),
            briefing.text.len(),
            &answer,
            clock(),
        )?;
        let judged = !gi::Category::parse(&item.family).is_some_and(gi::Category::code_checked);
        if judged && let Some(judge) = judge.as_deref_mut() {
            let evidence = judge::evidence(&fixture.gym, item)?;
            let case = judge::Case {
                agent: fixture.agent(),
                item,
                evidence: &evidence,
                briefing: &briefing,
                answer: &row.answer,
            };
            match judge.judge(&case) {
                Ok(judgment) => row = row.judged(judgment),
                Err(why) => summary.unjudged.push(format!("{}: {why}", item.id)),
            }
        }
        store.append(&row).map_err(|e| e.to_string())?;
        summary.rows += 1;
        if let Some(correct) = row.correct {
            summary.checked += 1;
            summary.correct += usize::from(correct);
        }
        if let Some(judgment) = &row.judgment {
            summary.judged += 1;
            summary.supported += usize::from(judgment.is_supported());
            summary.embellished += usize::from(judgment.is_embellished());
        }
        summary.evidence += usize::from(!row.evidence_carried.is_empty());
    }
    Ok(summary)
}

/// The schema of a round's report.
pub const ROUND_SCHEMA: &str = "openagents.gym.interview_round.v1";

/// How a round runs.
pub struct Round<'a> {
    /// What the items are, for the report: the suite and partition.
    pub group: String,
    /// The first seed block; blocks are trial numbers, and a round draws
    /// `blocks` consecutive ones.
    pub seed_base: u64,
    pub blocks: u64,
    /// Where arms keep their scratch host roots.
    pub scratch: &'a Path,
    pub clock: &'a dyn Fn() -> String,
    /// The owner's marks, when the judge's agreement should count.
    pub marks: Option<&'a gym::store::Store>,
}

/// What a round recorded: every arm's results, the `gym::ab` comparison of
/// the full arm with word overlap, and the gate's verdict.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RoundReport {
    pub schema: String,
    pub recorded_at: String,
    pub suite: String,
    pub suite_digest: String,
    pub fixture_digest: String,
    pub group: String,
    pub answerer: String,
    pub answerer_identity: DoorIdentity,
    pub blocks: Vec<u64>,
    /// Rows this round appended, and the store's head receipt after them.
    pub rows: usize,
    pub head: Option<String>,
    pub arms: Vec<gi::ArmSummary>,
    pub ab: gym::ab::Evidence,
    pub comparison: gym::gate::InterviewComparison,
    pub agreement: Option<gi::Agreement>,
    pub gate: gym::gate::Outcome,
}

/// Runs every arm over `items` on disjoint seed blocks and judges the
/// result.
///
/// The code-checked categories run first, as a `gym::ab` round of the
/// word-overlap arm (control) against the full arm (candidate), which
/// interleaves the two per block and category. Then every arm answers the
/// rest of the items in each block. Every answer is a receipt-chained row
/// in `store`; the gate `interview-v1` reads them.
///
/// # Errors
/// When an arm, the store, or the gate fails; rows already appended stay.
pub fn round(
    suite: &Suite,
    fixture: &Fixture,
    items: &[&Item],
    answerer: &mut dyn Answerer,
    judge: &mut dyn Judge,
    spec: &Round<'_>,
    store: &gym::store::Store,
) -> Result<RoundReport, String> {
    use gym::ab::{Attempt, Experiment, Phase, Plan, Side};
    let before = store.rows().map_err(|e| e.to_string())?.len();
    let blocks: Vec<u64> = (spec.seed_base..spec.seed_base + spec.blocks).collect();
    if blocks.is_empty() {
        return Err("a round needs at least one block".into());
    }
    let mut arms: BTreeMap<&'static str, Box<dyn Arm>> = ArmName::ALL
        .into_iter()
        .map(|name| (name.as_str(), name.build(spec.scratch)))
        .collect();
    let checked: Vec<String> = gi::Category::ALL
        .into_iter()
        .filter(|c| c.code_checked())
        .map(|c| c.as_str().to_string())
        .filter(|family| items.iter().any(|item| &item.family == family))
        .collect();
    let experiment = Experiment {
        suite: suite.name.clone(),
        suite_digest: suite.digest.clone(),
        control: format!("arm:{}", gi::BASELINE_ARM),
        candidate: format!("arm:{}", gi::SUBJECT_ARM),
        recorded_at: (spec.clock)(),
        rule: gi::ab_rule(),
    };
    let screening = Plan::new(Phase::Screening, checked.clone(), blocks.clone());
    let confirmation = screening.confirmation();
    let mut failure: Option<String> = None;
    let ab = experiment.run(&screening, &confirmation, |cell| {
        if let Some(why) = &failure {
            return Attempt::HarnessFailure {
                detail: format!("an earlier cell failed: {why}"),
            };
        }
        let name = match cell.side {
            Side::Control => gi::BASELINE_ARM,
            Side::Candidate => gi::SUBJECT_ARM,
        };
        let Some(arm) = arms.get_mut(name) else {
            return Attempt::HarnessFailure {
                detail: format!("no arm {name}"),
            };
        };
        let family: Vec<&Item> = items
            .iter()
            .copied()
            .filter(|item| item.family == cell.family)
            .collect();
        match run(
            suite,
            fixture,
            &family,
            arm.as_mut(),
            answerer,
            Some(&mut *judge),
            Some(cell.block),
            store,
            spec.clock,
        ) {
            Ok(summary) => Attempt::Scored {
                scores: gi::Tally {
                    rows: summary.rows,
                    checked: summary.checked,
                    correct: summary.correct,
                    ..gi::Tally::default()
                }
                .scores(),
                refusals: summary.skipped.len(),
            },
            Err(why) => {
                failure = Some(why.clone());
                Attempt::HarnessFailure { detail: why }
            }
        }
    });
    if let Some(why) = failure {
        return Err(why);
    }
    for &block in &blocks {
        for name in ArmName::ALL {
            let compared = [gi::BASELINE_ARM, gi::SUBJECT_ARM].contains(&name.as_str());
            let rest: Vec<&Item> = items
                .iter()
                .copied()
                .filter(|item| !compared || !checked.contains(&item.family))
                .collect();
            let arm = arms
                .get_mut(name.as_str())
                .ok_or_else(|| format!("no arm {}", name.as_str()))?;
            run(
                suite,
                fixture,
                &rest,
                arm.as_mut(),
                answerer,
                Some(&mut *judge),
                Some(block),
                store,
                spec.clock,
            )?;
        }
    }
    let all = gi::rows_of(&store.verified_rows().map_err(|e| e.to_string())?);
    let stored = store.rows().map_err(|e| e.to_string())?;
    let mine = gi::rows_of(&stored[before.min(stored.len())..]);
    let agreement = match spec.marks {
        Some(marks) => Some(gi::agreement(
            &all,
            &gi::marks_of(&marks.rows().map_err(|e| e.to_string())?),
        )),
        None => None,
    };
    let comparison = gi::comparison(&spec.group, &mine, agreement.as_ref());
    let gate = gi::gate().map_err(|e| e.to_string())?;
    Ok(RoundReport {
        schema: ROUND_SCHEMA.into(),
        recorded_at: (spec.clock)(),
        suite: suite.name.clone(),
        suite_digest: suite.digest.clone(),
        fixture_digest: fixture.gym.manifest.digest.clone(),
        group: spec.group.clone(),
        answerer: answerer.door().into(),
        answerer_identity: answerer.identity(),
        blocks,
        rows: mine.len(),
        head: store.head().map_err(|e| e.to_string())?,
        arms: gi::summarize(&mine),
        ab,
        agreement,
        gate: gate.judge_interview(&comparison),
        comparison,
    })
}

const USAGE: &str = "\
coder interview [--arm NAME|all] [--partition calibration|development|locked]
                [--answerer scripted|live] [--judge scripted|jev|none]
                [--store PATH] [--trial N] [--max-usd N]
                [--ledger PATH --reason TEXT] [--json]
coder interview round [--partition ...] [--blocks N] [--seed-base N]
                [--answerer scripted|live] [--judge scripted|jev]
                [--store PATH] [--marks PATH] [--out PATH] [--max-usd N]
                [--ledger PATH --reason TEXT] [--json]
coder interview sample [--store PATH] [--n N] [--json]
coder interview mark --item ID --arm NAME [--trial N]
                --supported yes|no --embellished yes|no [--note TEXT]
                [--store PATH] [--marks PATH]
coder interview agreement [--store PATH] [--marks PATH] [--json]

Interviews the workshop agent's frozen fixture (gym suite alice-interview-v2:
self-knowledge, memory, plans, reactions, and reflections) and appends one
receipt-chained row per item to a Gym store. Code checks memory and plan
answers; a judge reads the rest for support and embellishment.

  --arm         no-memory, word-overlap, no-reflection-or-plan,
                no-reflection, full, or all (default all)
  --partition   default development; locked is read once and needs --ledger
                and --reason, and the read is recorded in the ledger
  --answerer    scripted answers from the briefing with no model (default);
                live asks the agent's model through the capacity book
  --judge       scripted reads answers with no model (default); jev asks Jev
                questions/interview-answer.json; none leaves them unjudged
  --max-usd     stop asking the live model once its reported costs reach this
                (default 1.00)
  --store       default ~/.openagents/gym/interviews.jsonl
  --marks       default ~/.openagents/gym/interview-marks.jsonl
  --trial       a trial number, so a repeat run is a new trial

round runs every arm on --blocks disjoint seed blocks (default 3) from
--seed-base (default 0), compares full with word-overlap through gym::ab,
and judges the result with the gate interview-v1. --out writes the report.
sample lists judged answers for the owner to mark; mark records one mark;
agreement reports how the judge's readings agree with the marks.";

/// `coder interview ARGS`; returns the exit code.
#[must_use]
pub fn cli(args: &[String]) -> u8 {
    // The live model blocks on its own runtime, so the run leaves the
    // caller's.
    let args = args.to_vec();
    let result = std::thread::spawn(move || cli_inner(&args))
        .join()
        .unwrap_or_else(|_| Err("the run panicked".into()));
    match result {
        Ok(()) => 0,
        Err(why) => {
            eprintln!("coder interview: {why}");
            2
        }
    }
}

/// The flags every subcommand reads.
struct Flags {
    command: String,
    arms: Vec<ArmName>,
    partition: Partition,
    live: bool,
    judge: String,
    store: Option<PathBuf>,
    marks: Option<PathBuf>,
    out: Option<PathBuf>,
    trial: Option<u64>,
    blocks: u64,
    seed_base: u64,
    max_usd: f64,
    n: usize,
    item: Option<String>,
    arm: Option<String>,
    supported: Option<bool>,
    embellished: Option<bool>,
    note: String,
    ledger: Option<PathBuf>,
    reason: Option<String>,
    json: bool,
}

fn yes_no(flag: &str, value: &str) -> Result<bool, String> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => Err(format!("{flag} takes yes or no, not {other}")),
    }
}

fn parse(args: &[String]) -> Result<Option<Flags>, String> {
    let mut flags = Flags {
        command: "run".into(),
        arms: ArmName::ALL.to_vec(),
        partition: Partition::Development,
        live: false,
        judge: "scripted".into(),
        store: None,
        marks: None,
        out: None,
        trial: None,
        blocks: 3,
        seed_base: 0,
        max_usd: 1.0,
        n: 20,
        item: None,
        arm: None,
        supported: None,
        embellished: None,
        note: String::new(),
        ledger: None,
        reason: None,
        json: false,
    };
    let mut iter = args.iter().peekable();
    if let Some(command) = iter.peek()
        && ["round", "sample", "mark", "agreement"].contains(&command.as_str())
    {
        flags.command = (*command).clone();
        iter.next();
    }
    let number = |flag: &str, value: String| {
        value
            .parse::<u64>()
            .map_err(|_| format!("{flag} takes a number"))
    };
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                println!("{USAGE}");
                return Ok(None);
            }
            "--arm" => {
                let name = value()?;
                flags.arms = if name == "all" {
                    ArmName::ALL.to_vec()
                } else {
                    vec![ArmName::parse(&name).ok_or_else(|| format!("no arm {name}"))?]
                };
                flags.arm = Some(name);
            }
            "--partition" => {
                flags.partition = match value()?.as_str() {
                    "calibration" => Partition::Calibration,
                    "development" => Partition::Development,
                    "locked" => Partition::Locked,
                    other => return Err(format!("no partition {other}")),
                };
            }
            "--answerer" => {
                flags.live = match value()?.as_str() {
                    "scripted" => false,
                    "live" => true,
                    other => return Err(format!("no answerer {other}")),
                };
            }
            "--judge" => {
                let judge = value()?;
                if !["scripted", "jev", "none"].contains(&judge.as_str()) {
                    return Err(format!("no judge {judge}"));
                }
                flags.judge = judge;
            }
            "--store" => flags.store = Some(PathBuf::from(value()?)),
            "--marks" => flags.marks = Some(PathBuf::from(value()?)),
            "--out" => flags.out = Some(PathBuf::from(value()?)),
            "--trial" => flags.trial = Some(number("--trial", value()?)?),
            "--blocks" => flags.blocks = number("--blocks", value()?)?,
            "--seed-base" => flags.seed_base = number("--seed-base", value()?)?,
            "--n" => flags.n = usize::try_from(number("--n", value()?)?).unwrap_or(usize::MAX),
            "--max-usd" => {
                flags.max_usd = value()?
                    .parse::<f64>()
                    .ok()
                    .filter(|usd| usd.is_finite() && *usd >= 0.0)
                    .ok_or("--max-usd takes a number of dollars")?;
            }
            "--item" => flags.item = Some(value()?),
            "--supported" => flags.supported = Some(yes_no("--supported", &value()?)?),
            "--embellished" => flags.embellished = Some(yes_no("--embellished", &value()?)?),
            "--note" => flags.note = value()?,
            "--ledger" => flags.ledger = Some(PathBuf::from(value()?)),
            "--reason" => flags.reason = Some(value()?),
            "--json" => flags.json = true,
            other => return Err(format!("unexpected argument {other}\n\n{USAGE}")),
        }
    }
    Ok(Some(flags))
}

/// `~/.openagents/gym/NAME`, or `given`.
fn gym_path(given: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    if let Some(path) = given {
        return Ok(path);
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym").join(name))
        .ok_or_else(|| format!("HOME is unset; give the path to {name}"))
}

fn cli_inner(args: &[String]) -> Result<(), String> {
    let Some(flags) = parse(args)? else {
        return Ok(());
    };
    let store_path = gym_path(flags.store.clone(), "interviews.jsonl")?;
    let store = gym::store::Store::at(&store_path);
    match flags.command.as_str() {
        "sample" => return sample_command(&store, &flags),
        "mark" => {
            let marks =
                gym::store::Store::at(gym_path(flags.marks.clone(), "interview-marks.jsonl")?);
            return mark_command(&store, &marks, &flags);
        }
        "agreement" => {
            let marks =
                gym::store::Store::at(gym_path(flags.marks.clone(), "interview-marks.jsonl")?);
            return agreement_command(&store, &marks, &flags);
        }
        _ => {}
    }
    let suite = gi::alice_v2_suite().map_err(|e| e.to_string())?;
    let fixture = Fixture::alice_v1()?;
    let ledger = flags.ledger.clone().map(LockedLedger::at);
    let now = gym::eval::now_utc();
    let subject = format!(
        "{} {} judge {} arms {}",
        flags.command,
        if flags.live { "live" } else { "scripted" },
        flags.judge,
        flags
            .arms
            .iter()
            .map(|a| a.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let spend = Spend {
        subject: &subject,
        reason: flags.reason.as_deref().unwrap_or_default(),
        at: &now,
        adapter: "",
    };
    let items = items(
        &suite,
        flags.partition,
        ledger.as_ref().map(|l| (l, &spend)),
    )?;
    let scratch = std::env::temp_dir().join(format!("coder-interview-{}", std::process::id()));
    let mut answerer: Box<dyn Answerer> = if flags.live {
        Box::new(Live::new(LiveModel::new()?).capped(flags.max_usd))
    } else {
        Box::new(FromBriefing)
    };
    let mut judge: Option<Box<dyn Judge>> = match flags.judge.as_str() {
        "jev" => Some(Box::new(JevJudge::from_env()?)),
        "none" => None,
        _ => Some(Box::new(ScriptedJudge)),
    };
    let result = if flags.command == "round" {
        let marks = gym::store::Store::at(gym_path(flags.marks.clone(), "interview-marks.jsonl")?);
        let mut judge = judge.ok_or("a round needs a judge: --judge scripted or jev")?;
        round_command(
            &suite,
            &fixture,
            &items,
            answerer.as_mut(),
            judge.as_mut(),
            &flags,
            &scratch,
            &store,
            &marks,
        )
    } else {
        run_command(
            &suite,
            &fixture,
            &items,
            answerer.as_mut(),
            judge.as_deref_mut(),
            &flags,
            &scratch,
            &store,
        )
    };
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

#[allow(clippy::too_many_arguments)]
fn run_command(
    suite: &Suite,
    fixture: &Fixture,
    items: &[&Item],
    answerer: &mut dyn Answerer,
    mut judge: Option<&mut (dyn Judge + '_)>,
    flags: &Flags,
    scratch: &Path,
    store: &gym::store::Store,
) -> Result<(), String> {
    let mut summaries = Vec::new();
    for name in &flags.arms {
        let mut arm = name.build(scratch);
        summaries.push(run(
            suite,
            fixture,
            items,
            arm.as_mut(),
            answerer,
            judge.as_deref_mut(),
            flags.trial,
            store,
            &gym::eval::now_utc,
        )?);
    }
    if flags.json {
        let out: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "schema": "openagents.coder-interview-summary.v2",
                    "suite": suite.name, "suite_digest": suite.digest,
                    "partition": flags.partition, "arm": s.arm, "answerer": s.door,
                    "judge": flags.judge,
                    "rows": s.rows, "checked": s.checked, "correct": s.correct,
                    "judged": s.judged, "supported": s.supported,
                    "embellished": s.embellished,
                    "evidence": s.evidence, "skipped": s.skipped, "unjudged": s.unjudged,
                    "store": store.path().display().to_string(),
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(out));
        return Ok(());
    }
    println!(
        "{} {} partition, {} answerer, {} judge, rows in {}",
        suite.name,
        flags.partition,
        if flags.live { "live" } else { "scripted" },
        flags.judge,
        store.path().display()
    );
    for s in &summaries {
        println!(
            "  {:<22} {:>2} of {:>2} right; {:>2} judged, {:>2} supported, {:>2} embellished; \
             {} skipped",
            s.arm,
            s.correct,
            s.checked,
            s.judged,
            s.supported,
            s.embellished,
            s.skipped.len()
        );
        for why in s.skipped.iter().chain(&s.unjudged) {
            println!("    {why}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn round_command(
    suite: &Suite,
    fixture: &Fixture,
    items: &[&Item],
    answerer: &mut dyn Answerer,
    judge: &mut dyn Judge,
    flags: &Flags,
    scratch: &Path,
    store: &gym::store::Store,
    marks: &gym::store::Store,
) -> Result<(), String> {
    let spec = Round {
        group: format!("{} {}", suite.name, flags.partition),
        seed_base: flags.seed_base,
        blocks: flags.blocks,
        scratch,
        clock: &gym::eval::now_utc,
        marks: Some(marks),
    };
    let report = round(suite, fixture, items, answerer, judge, &spec, store)?;
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(out) = &flags.out {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(out, format!("{text}\n"))
            .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    }
    if flags.json {
        println!("{text}");
        return Ok(());
    }
    print!("{}", render_round(&report));
    Ok(())
}

/// A round's report as the terminal shows it.
#[must_use]
pub fn render_round(report: &RoundReport) -> String {
    let mut out = format!(
        "{} on blocks {:?}: {} rows, {} answerer\n",
        report.group, report.blocks, report.rows, report.answerer_identity.model
    );
    for arm in &report.arms {
        let cell = |family: &str| {
            arm.categories.get(family).map_or_else(
                || "-".to_string(),
                |t| {
                    if t.checked > 0 {
                        format!("{}/{}", t.correct, t.checked)
                    } else {
                        format!("{}/{} e{}", t.supported, t.judged, t.embellished)
                    }
                },
            )
        };
        out.push_str(&format!(
            "  {:<22} self {:<9} memory {:<6} plan {:<6} reaction {:<9} reflection {}\n",
            arm.arm,
            cell("self"),
            cell("memory"),
            cell("plan"),
            cell("reaction"),
            cell("reflection")
        ));
    }
    out.push_str(&format!(
        "  gym::ab {}: {}\n",
        report.ab.decision, report.ab.reason
    ));
    out.push_str(&format!(
        "  gate {} {}: {}\n",
        report.gate.gate_id, report.gate.gate_digest, report.gate.verdict
    ));
    for criterion in &report.gate.criteria {
        out.push_str(&format!(
            "    {:<12} {}: {}\n",
            criterion.verdict.as_str(),
            criterion.name,
            criterion.detail
        ));
    }
    out
}

fn sample_command(store: &gym::store::Store, flags: &Flags) -> Result<(), String> {
    let rows = gi::rows_of(&store.verified_rows().map_err(|e| e.to_string())?);
    let picked = gi::sample(&rows, flags.n);
    if flags.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&picked).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let suite = gi::alice_v2_suite().map_err(|e| e.to_string())?;
    for row in picked {
        let question = suite
            .items
            .iter()
            .find(|item| item.id == row.item_id)
            .map(gi::question)
            .unwrap_or_default();
        let judgment = row.judgment.as_ref();
        println!(
            "{} ({}, trial {}): {question}\n  answer: {}\n  judge {}: supported {:.2}, \
             embellished {:.2}\n  coder interview mark --item {} --arm {}{} --supported yes|no \
             --embellished yes|no\n",
            row.item_id,
            row.arm,
            row.seed_base
                .map_or_else(|| "none".into(), |t| t.to_string()),
            row.answer,
            judgment.map_or_else(String::new, gi::Judgment::name),
            judgment.map_or(0.0, |j| j.supported),
            judgment.map_or(0.0, |j| j.embellished),
            row.item_id,
            row.arm,
            row.seed_base
                .map_or_else(String::new, |t| format!(" --trial {t}")),
        );
    }
    Ok(())
}

fn mark_command(
    store: &gym::store::Store,
    marks: &gym::store::Store,
    flags: &Flags,
) -> Result<(), String> {
    let item = flags.item.as_deref().ok_or("mark needs --item")?;
    let arm = flags.arm.as_deref().ok_or("mark needs --arm")?;
    let supported = flags.supported.ok_or("mark needs --supported yes|no")?;
    let embellished = flags.embellished.ok_or("mark needs --embellished yes|no")?;
    let rows = gi::rows_of(&store.verified_rows().map_err(|e| e.to_string())?);
    let row = rows
        .iter()
        .rev()
        .find(|row| {
            row.item_id == item
                && row.arm == arm
                && row.seed_base == flags.trial
                && row.judgment.is_some()
        })
        .ok_or_else(|| {
            format!(
                "no judged answer to {item} by {arm} in trial {:?} in {}",
                flags.trial,
                store.path().display()
            )
        })?;
    let mark = gi::Mark::on(
        row,
        supported,
        embellished,
        &flags.note,
        gym::eval::now_utc(),
    );
    marks.append(&mark).map_err(|e| e.to_string())?;
    println!(
        "marked {item} by {arm}: supported {}, embellished {}; marks in {}",
        if supported { "yes" } else { "no" },
        if embellished { "yes" } else { "no" },
        marks.path().display()
    );
    Ok(())
}

fn agreement_command(
    store: &gym::store::Store,
    marks: &gym::store::Store,
    flags: &Flags,
) -> Result<(), String> {
    let rows = gi::rows_of(&store.verified_rows().map_err(|e| e.to_string())?);
    let marks = gi::marks_of(&marks.verified_rows().map_err(|e| e.to_string())?);
    let agreement = gi::agreement(&rows, &marks);
    if flags.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": "openagents.gym.interview_agreement.v1",
                "agreement": agreement,
                "rate": agreement.rate(),
            })
        );
        return Ok(());
    }
    println!(
        "{} marks, {} on judged answers in the store: supported agrees on {}, embellished on {}, \
         {} embellishments the judge missed; agreement {}",
        agreement.marks,
        agreement.matched,
        agreement.supported_agree,
        agreement.embellished_agree,
        agreement.missed_embellishments,
        agreement
            .rate()
            .map_or_else(|| "unknown".into(), |rate| format!("{rate:.3}"))
    );
    Ok(())
}

#[cfg(test)]
#[path = "agent_interview_tests.rs"]
mod tests;
