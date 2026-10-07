//! Gym interviews of a workshop agent (`docs/verse/generative-agents.md`,
//! item 7): `coder interview`.
//!
//! A run asks each item of a pinned suite (`gym::interview`) about a frozen
//! fixture of the agent's journal and memory. An [`Arm`] builds the
//! briefing the agent would carry, an [`Answerer`] answers from it, and
//! each answer becomes a code-graded, receipt-chained row in a Gym store.
//!
//! Two arms exist: [`ArmName::NoMemory`], an empty briefing, and
//! [`ArmName::WordOverlap`], today's [`Memory::briefing`]. The scored
//! memory, no-reflection, and no-plan arms plug in as further
//! [`ArmName`] variants with their own [`Arm`]. The [`FromBriefing`] and
//! [`Canned`] answerers run with no model; [`Live`] asks the agent's
//! model through the capacity book, and only the command uses it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gym::interview::{self as gi, ItemState};
use gym::row::DoorIdentity;
use gym::suite::{Item, LockedLedger, Partition, Spend, Suite};

use super::agent::{Entry, LiveModel, Model, Store};
use super::agent_memory::{Memory, MemoryEntry};

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
    /// Today's briefing: [`Memory::briefing`], words shared with the question.
    WordOverlap,
}

impl ArmName {
    pub const ALL: [Self; 2] = [Self::NoMemory, Self::WordOverlap];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoMemory => "no-memory",
            Self::WordOverlap => "word-overlap",
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

/// One model call that returns a reply and the model that wrote it.
pub trait Respond {
    /// # Errors
    /// When no model answered.
    fn respond(&mut self, system: &str, prompt: &str) -> Result<(String, String), String>;
}

impl Respond for LiveModel {
    fn respond(&mut self, system: &str, prompt: &str) -> Result<(String, String), String> {
        let action = self.next(system, prompt)?;
        let reply = if action.reply.trim().is_empty() {
            action.rationale
        } else {
            action.reply
        };
        Ok((reply, self.model.clone().unwrap_or_default()))
    }
}

/// The agent's model answers, through [`prompt`].
pub struct Live<R> {
    model: R,
    last: String,
}

impl<R: Respond> Live<R> {
    #[must_use]
    pub fn new(model: R) -> Self {
        Self {
            model,
            last: String::new(),
        }
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
        let (system, user) = prompt(agent, briefing, ask);
        let (reply, model) = self.model.respond(&system, &user)?;
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
    /// Rows whose briefing carried at least one of the item's sources.
    pub evidence: usize,
    /// Items with no row, and why.
    pub skipped: Vec<String>,
}

/// Asks every item in `items`, appending one row each to `store`.
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
        let row = gi::Row::graded(
            suite,
            item,
            arm.name(),
            answerer.door(),
            answerer.identity(),
            trial,
            briefing.carried,
            briefing.text.len(),
            &answer,
            clock(),
        )?;
        store.append(&row).map_err(|e| e.to_string())?;
        summary.rows += 1;
        if let Some(correct) = row.correct {
            summary.checked += 1;
            summary.correct += usize::from(correct);
        }
        summary.evidence += usize::from(!row.evidence_carried.is_empty());
    }
    Ok(summary)
}

const USAGE: &str = "\
coder interview [--arm NAME|all] [--partition calibration|development|locked]
                [--answerer scripted|live] [--store PATH] [--trial N]
                [--ledger PATH --reason TEXT] [--json]

Interviews the workshop agent's frozen fixture (gym suite alice-interview-v1)
and appends one receipt-chained row per item to a Gym store.

  --arm         no-memory, word-overlap, or all (default all)
  --partition   default development; locked is read once and needs --ledger
                and --reason, and the read is recorded in the ledger
  --answerer    scripted answers from the briefing with no model (default);
                live asks the agent's model through the capacity book
  --store       default ~/.openagents/gym/interviews.jsonl
  --trial       a trial number, so a repeat run is a new trial";

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

fn cli_inner(args: &[String]) -> Result<(), String> {
    let mut arms = ArmName::ALL.to_vec();
    let mut partition = Partition::Development;
    let mut live = false;
    let mut store = None;
    let mut trial = None;
    let mut ledger = None;
    let mut reason = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--arm" => {
                let name = value()?;
                arms = if name == "all" {
                    ArmName::ALL.to_vec()
                } else {
                    vec![ArmName::parse(&name).ok_or_else(|| format!("no arm {name}"))?]
                };
            }
            "--partition" => {
                partition = match value()?.as_str() {
                    "calibration" => Partition::Calibration,
                    "development" => Partition::Development,
                    "locked" => Partition::Locked,
                    other => return Err(format!("no partition {other}")),
                };
            }
            "--answerer" => {
                live = match value()?.as_str() {
                    "scripted" => false,
                    "live" => true,
                    other => return Err(format!("no answerer {other}")),
                };
            }
            "--store" => store = Some(PathBuf::from(value()?)),
            "--trial" => {
                trial = Some(
                    value()?
                        .parse::<u64>()
                        .map_err(|_| "--trial takes a number".to_string())?,
                );
            }
            "--ledger" => ledger = Some(PathBuf::from(value()?)),
            "--reason" => reason = Some(value()?),
            "--json" => json = true,
            other => return Err(format!("unexpected argument {other}\n\n{USAGE}")),
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    let store = match store {
        Some(path) => path,
        None => home
            .as_ref()
            .ok_or("HOME is unset; give --store")?
            .join(".openagents/gym/interviews.jsonl"),
    };
    let suite = gi::alice_v1_suite().map_err(|e| e.to_string())?;
    let fixture = Fixture::alice_v1()?;
    let ledger = ledger.map(LockedLedger::at);
    let now = gym::eval::now_utc();
    let subject = format!(
        "{} arms {}",
        if live { "live" } else { "scripted" },
        arms.iter()
            .map(|a| a.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let spend = Spend {
        subject: &subject,
        reason: reason.as_deref().unwrap_or_default(),
        at: &now,
        adapter: "",
    };
    let items = items(&suite, partition, ledger.as_ref().map(|l| (l, &spend)))?;
    let scratch = std::env::temp_dir().join(format!("coder-interview-{}", std::process::id()));
    let mut answerer: Box<dyn Answerer> = if live {
        Box::new(Live::new(LiveModel::new()?))
    } else {
        Box::new(FromBriefing)
    };
    let gym_store = gym::store::Store::at(&store);
    let mut summaries = Vec::new();
    for name in arms {
        let mut arm = name.build(&scratch);
        let result = run(
            &suite,
            &fixture,
            &items,
            arm.as_mut(),
            answerer.as_mut(),
            trial,
            &gym_store,
            &gym::eval::now_utc,
        );
        match result {
            Ok(summary) => summaries.push(summary),
            Err(why) => {
                let _ = std::fs::remove_dir_all(&scratch);
                return Err(why);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    if json {
        let out: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "schema": "openagents.coder-interview-summary.v1",
                    "suite": suite.name, "suite_digest": suite.digest,
                    "partition": partition, "arm": s.arm, "answerer": s.door,
                    "rows": s.rows, "checked": s.checked, "correct": s.correct,
                    "evidence": s.evidence, "skipped": s.skipped,
                    "store": store.display().to_string(),
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(out));
    } else {
        println!(
            "{} {} partition, {} answerer, rows in {}",
            suite.name,
            partition,
            if live { "live" } else { "scripted" },
            store.display()
        );
        for s in &summaries {
            println!(
                "  {:<13} {:>2} of {:>2} right, evidence carried for {:>2}, {} skipped",
                s.arm,
                s.correct,
                s.checked,
                s.evidence,
                s.skipped.len()
            );
            for why in &s.skipped {
                println!("    skipped {why}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "agent_interview_tests.rs"]
mod tests;
