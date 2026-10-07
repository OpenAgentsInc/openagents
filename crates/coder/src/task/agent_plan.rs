//! A workshop agent's day plan from real work, with re-planning
//! (`docs/verse/generative-agents.md`, item 4).
//!
//! A plan (`openagents.agent-day-plan.v1`, [`DayPlan`]) lives in
//! `agents/NAME/plan.json`. It is made from real sources only
//! ([`sources`]): standing jobs with a schedule, which code places in their
//! slots with no model call ([`code_blocks`]); the issues `watch-issues`
//! would pick; the owner's queued requests; and her accepted insights.
//! One model call drafts the rest ([`draft`]) from the sources it is given
//! by ID, and code keeps a drafted block only when its source is one of
//! them, its node is a place she knows within her walking bound
//! ([`places`]), and its time fits. With no source there is no call, and
//! the plan has no blocks: she idles at her desk.
//!
//! Only the block under way is decomposed into 5 to 15 minute steps, when
//! it starts ([`begin`], [`decompose`]), with one call through the same
//! [`Writer`]. When something happens, code decides the known cases: the
//! owner's request interrupts ([`Event::Owner`]), and a scheduled job
//! fires in the slot already planned ([`Event::Job`]). Jev decides the
//! rest with the `questions/react-or-continue.json` choice ([`Judge`]).
//! Reacting or deferring re-plans from the current block on ([`replan`]),
//! by code, and the plan keeps the history.
//!
//! A studio seat's plan is its task queue, rendered with no model call
//! ([`seat_plan`]).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

use coder_host::access::day_plan::{
    self as wire, Block, By, DAY, DayPlan, MAX_BLOCKS, MAX_REPLANS, MAX_STEPS, MAX_TITLE, Replan,
    Replanned, Step, clock,
};
use serde::Deserialize;
use world_tree::{Affordance, Kind as NodeKind, Known, Node, Tree};

use super::agent::{Entry, Kind, Store};
use super::agent_jobs::{Job, Trigger};
use super::agent_memory::{MemoryEntry, MemoryKind, MemoryState};
use super::agent_reflect::Writer;
use crate::questions::{Fill, Set};

#[path = "agent_plan_live.rs"]
mod live;
pub use live::JevJudge;

/// The owner's house: her walks stay inside it unless the owner widens the
/// bound (`docs/verse/generative-agents.md`, Decisions).
pub const HOUSE: &str = "everglade/knowledge-district/owners-house";
/// The great room, where she lives and works.
pub const GREAT_ROOM: &str = "everglade/knowledge-district/owners-house/great-room";
/// Her workstation: where an idle day, or a block with no better place,
/// puts her.
pub const DESK: &str = "everglade/knowledge-district/owners-house/great-room/workstation";
/// A scheduled job's block, minutes.
pub const JOB_MINUTES: u32 = 30;
/// An interruption's or a reaction's block, minutes.
pub const REACTION_MINUTES: u32 = 30;
/// The shortest and longest block a draft may hold, minutes.
pub const BLOCK_MIN: u32 = 15;
pub const BLOCK_MAX: u32 = 240;
/// The shortest and longest step, minutes.
pub const STEP_MIN: u32 = 5;
pub const STEP_MAX: u32 = 15;
/// The window one decomposition covers from the block's start, minutes:
/// the paper's hour.
pub const STEP_WINDOW: u32 = 60;
/// The most issues and insights a draft is offered.
pub const ISSUES_MAX: usize = 5;
pub const INSIGHTS_MAX: usize = 5;
/// The template that makes the morning plan.
pub const TEMPLATE: &str = "plan";

const SET_JSON: &str = include_str!("../../../../questions/react-or-continue.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the react-or-continue set parses");
    set.validate()
        .expect("the react-or-continue set is one this host asks");
    set
});

/// The react-or-continue question set.
#[must_use]
pub fn react_set() -> &'static Set {
    &SET
}

/// The local date, `YYYY-MM-DD`, and minute of the day at Unix second
/// `now`, `utc_offset` minutes east of UTC.
#[must_use]
pub fn local(now: u64, utc_offset: i32) -> (String, u32) {
    let local = now as i64 + i64::from(utc_offset) * 60;
    let days = local.div_euclid(86_400);
    let minute = u32::try_from(local.rem_euclid(86_400) / 60).unwrap_or(0);
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (format!("{year:04}-{month:02}-{day:02}"), minute)
}

/// What a source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    /// A standing job with a schedule: code places it.
    Job,
    Issue,
    Request,
    Insight,
}

/// One piece of real work a plan may name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// `job:ID`, `issue:N`, `request:N` or `journal:POS`, or `memory:ID`.
    pub id: String,
    pub kind: SourceKind,
    pub text: String,
    /// A scheduled job's slot, as a local minute.
    pub slot: Option<u32>,
    /// What a job's work needs of a place.
    pub needs: Affordance,
}

/// What a plan is made from.
pub struct Inputs<'a> {
    pub agent: &'a str,
    /// Unix seconds.
    pub now: u64,
    pub utc_offset: i32,
    /// Her standing jobs; only the enabled ones count.
    pub jobs: &'a [Job],
    /// The issues `watch-issues` would pick, best first: number and title.
    pub issues: &'a [(u64, String)],
    /// The owner's queued requests: a source ID and the text.
    pub queued: &'a [(String, String)],
    /// Her memory; only active insights count.
    pub memory: &'a [MemoryEntry],
    pub tree: &'a Tree,
    pub known: &'a Known,
    /// The node her walks stay within: [`HOUSE`] unless the owner widened
    /// it.
    pub bound: &'a str,
}

/// The objects she may work at: ones she knows, inside `bound`, that offer
/// something. Her great room is always known to her.
#[must_use]
pub fn places<'t>(tree: &'t Tree, known: &Known, bound: &str) -> Vec<&'t Node> {
    let within = |id: &str| id == bound || id.starts_with(&format!("{bound}/"));
    tree.walk()
        .into_iter()
        .filter(|n| n.kind == NodeKind::Object && !n.affordances.is_empty())
        .filter(|n| within(&n.id))
        .filter(|n| known.knows(&n.id) || n.id.starts_with(&format!("{GREAT_ROOM}/")))
        .collect()
}

/// The most specific place offering `needs` (the one offering least
/// else), else her desk when it is a place, else the first place, else her
/// desk's ID.
#[must_use]
pub fn place_for(places: &[&Node], needs: Affordance) -> String {
    places
        .iter()
        .filter(|n| n.offers(needs))
        .min_by_key(|n| n.affordances.len())
        .or_else(|| places.iter().find(|n| n.id == DESK))
        .or_else(|| places.first())
        .map_or_else(|| DESK.to_string(), |n| n.id.clone())
}

fn job_slot(job: &Job, now: u64) -> Option<u32> {
    let (at, weekday, offset) = match &job.trigger {
        Trigger::Schedule {
            at,
            weekday,
            utc_offset,
        } => (at, *weekday, *utc_offset),
        Trigger::Reflect { at, utc_offset, .. } | Trigger::Plan { at, utc_offset, .. } => {
            (at, None, *utc_offset)
        }
        Trigger::Issues { .. } | Trigger::Checks {} => return None,
    };
    let minute = wire::minute(at)?;
    if let Some(weekday) = weekday {
        let local = now as i64 + i64::from(offset) * 60;
        // 1970-01-01 was a Thursday: weekday 3 counting from Monday.
        let today = (local.div_euclid(86_400) + 3).rem_euclid(7);
        if i64::from(weekday) != today {
            return None;
        }
    }
    Some(minute)
}

/// Every real source for the day `inputs` describes.
#[must_use]
pub fn sources(inputs: &Inputs<'_>) -> Vec<Source> {
    let mut out = Vec::new();
    for job in inputs.jobs.iter().filter(|j| j.enabled) {
        // The morning plan doesn't plan itself.
        if matches!(job.trigger, Trigger::Plan { .. }) {
            continue;
        }
        let Some(slot) = job_slot(job, inputs.now) else {
            continue;
        };
        let needs = if matches!(job.trigger, Trigger::Reflect { .. }) {
            Affordance::Work
        } else if job.mode == coder_host::access::agent::Mode::Terminal {
            Affordance::RunCommands
        } else {
            Affordance::Work
        };
        out.push(Source {
            id: format!("job:{}", job.job),
            kind: SourceKind::Job,
            text: job.title.clone(),
            slot: Some(slot),
            needs,
        });
    }
    for (number, title) in inputs.issues.iter().take(ISSUES_MAX) {
        out.push(Source {
            id: format!("issue:{number}"),
            kind: SourceKind::Issue,
            text: title.clone(),
            slot: None,
            needs: Affordance::Work,
        });
    }
    for (id, text) in inputs.queued {
        out.push(Source {
            id: id.clone(),
            kind: SourceKind::Request,
            text: one_line(text, 200),
            slot: None,
            needs: Affordance::Work,
        });
    }
    let mut insights: Vec<&MemoryEntry> = inputs
        .memory
        .iter()
        .filter(|e| e.kind == MemoryKind::Insight && e.state == MemoryState::Active)
        .collect();
    insights.sort_by_key(|e| std::cmp::Reverse(e.at));
    for entry in insights.into_iter().take(INSIGHTS_MAX) {
        out.push(Source {
            id: format!("memory:{}", entry.id),
            kind: SourceKind::Insight,
            text: one_line(&entry.text, 200),
            slot: None,
            needs: Affordance::Work,
        });
    }
    out
}

fn one_line(text: &str, max: usize) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= max {
        return line;
    }
    let mut cut: String = line.chars().take(max.saturating_sub(3)).collect();
    cut.push_str("...");
    cut
}

/// The scheduled jobs' blocks, in their slots, by code. Two jobs in one
/// slot follow each other.
#[must_use]
pub fn code_blocks(sources: &[Source], places: &[&Node]) -> Vec<Block> {
    let mut jobs: Vec<&Source> = sources.iter().filter(|s| s.slot.is_some()).collect();
    jobs.sort_by_key(|s| s.slot);
    let mut blocks: Vec<Block> = Vec::new();
    for source in jobs {
        let slot = source.slot.unwrap_or(0);
        let start = blocks.last().map_or(slot, |b| slot.max(b.end));
        let end = (start + JOB_MINUTES).min(DAY);
        if end <= start || blocks.len() == MAX_BLOCKS {
            continue;
        }
        blocks.push(Block {
            start,
            end,
            title: source.text.clone(),
            source: source.id.clone(),
            node: place_for(places, source.needs),
            by: By::Code,
        });
    }
    blocks
}

/// A block the model drafted, before code checks it.
#[derive(Clone, Debug, Deserialize)]
pub struct Drafted {
    pub source: String,
    pub node: String,
    pub start: String,
    pub minutes: u32,
    pub title: String,
}

/// What [`draft`] made.
#[derive(Clone, Debug, PartialEq)]
pub struct Made {
    pub plan: DayPlan,
    /// Drafted blocks code refused, with why.
    pub rejected: Vec<(String, String)>,
    /// Whether the model was asked.
    pub called: bool,
    /// What the call cost, when reported.
    pub usd: Option<f64>,
    pub model: String,
}

/// The draft call's system text and prompt.
#[must_use]
pub fn draft_prompt(
    agent: &str,
    date: &str,
    minute: u32,
    sources: &[&Source],
    fixed: &[Block],
    places: &[&Node],
    room: usize,
) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, planning your working day. Run no commands: \
         finish on this step and put only the JSON asked for in your reply."
    );
    let listed: String = sources
        .iter()
        .map(|s| format!("[{}] {}\n", s.id, s.text))
        .collect();
    let fixed: String = if fixed.is_empty() {
        "none\n".into()
    } else {
        fixed
            .iter()
            .map(|b| {
                format!(
                    "{}-{} {} [{}]\n",
                    clock(b.start),
                    clock(b.end),
                    b.title,
                    b.source
                )
            })
            .collect()
    };
    let nodes: String = places
        .iter()
        .map(|n| {
            let offers: Vec<&str> = n.affordances.iter().map(|a| a.as_str()).collect();
            format!("{} ({}): offers {}\n", n.id, n.name, offers.join(", "))
        })
        .collect();
    let user = format!(
        "Today is {date}; it is {} now. Plan the rest of the day as at most {room} blocks of real \
         work. Every block works exactly one of these sources, named by its ID, and nothing else; \
         leave a source out rather than invent work:\n{listed}\nThese blocks are fixed already; \
         don't overlap them:\n{fixed}\nWork at one of these places, by ID:\n{nodes}\nEach block \
         starts at or after now, lasts {BLOCK_MIN} to {BLOCK_MAX} minutes, and ends by midnight. \
         Code drops any block whose source or place isn't listed. Reply with JSON: \
         {{\"blocks\": [{{\"source\": \"issue:7\", \"node\": \"...\", \"start\": \"09:00\", \
         \"minutes\": 60, \"title\": \"...\"}}]}}",
        clock(minute)
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

/// The blocks in a draft call's reply.
///
/// # Errors
/// When the reply isn't the JSON asked for.
pub fn parse_draft(text: &str) -> Result<Vec<Drafted>, String> {
    #[derive(Deserialize)]
    struct Reply {
        blocks: Vec<Drafted>,
    }
    let reply: Reply = serde_json::from_value(json_object(text)?)
        .map_err(|e| format!("the reply isn't a list of blocks: {e}"))?;
    Ok(reply.blocks)
}

fn overlaps(blocks: &[Block], start: u32, end: u32) -> bool {
    blocks.iter().any(|b| start < b.end && b.start < end)
}

/// Checks one drafted block against what code knows.
///
/// # Errors
/// Why it is refused.
pub fn check(
    drafted: &Drafted,
    offered: &[&Source],
    places: &[&Node],
    taken: &[Block],
    minute: u32,
    screen: &secret_screen::Screen,
) -> Result<Block, String> {
    if !offered.iter().any(|s| s.id == drafted.source) {
        return Err(format!("source {} wasn't offered", drafted.source));
    }
    if taken.iter().any(|b| b.source == drafted.source) {
        return Err(format!("source {} is planned already", drafted.source));
    }
    if !places.iter().any(|n| n.id == drafted.node) {
        return Err(format!(
            "node {} isn't a place she knows within her bound",
            drafted.node
        ));
    }
    let start = wire::minute(&drafted.start).ok_or("the start isn't HH:MM")?;
    if start < minute {
        return Err(format!("it starts at {}, before now", clock(start)));
    }
    if !(BLOCK_MIN..=BLOCK_MAX).contains(&drafted.minutes) {
        return Err(format!(
            "it lasts {} minutes, not {BLOCK_MIN} to {BLOCK_MAX}",
            drafted.minutes
        ));
    }
    let end = start + drafted.minutes;
    if end > DAY {
        return Err("it runs past midnight".into());
    }
    if overlaps(taken, start, end) {
        return Err(format!(
            "{}-{} overlaps a planned block",
            clock(start),
            clock(end)
        ));
    }
    let title = one_line(&drafted.title, MAX_TITLE);
    if title.is_empty() {
        return Err("it has no title".into());
    }
    screen
        .check(&title)
        .map_err(|why| format!("its title fails the screen: {why}"))?;
    Ok(Block {
        start,
        end,
        title,
        source: drafted.source.clone(),
        node: drafted.node.clone(),
        by: By::Model,
    })
}

/// A new plan for `inputs`' day: code places the scheduled jobs, and one
/// call to `writer` drafts the rest from the other sources, which code
/// checks ([`check`]). No source but the jobs means no call; no source at
/// all means an idle plan.
///
/// # Errors
/// When the model call fails or its reply doesn't read; the caller keeps
/// no plan rather than an invented one.
pub fn draft(
    inputs: &Inputs<'_>,
    writer: &mut dyn Writer,
    screen: &secret_screen::Screen,
) -> Result<Made, String> {
    let (date, minute) = local(inputs.now, inputs.utc_offset);
    let all = sources(inputs);
    let places = places(inputs.tree, inputs.known, inputs.bound);
    let mut blocks = code_blocks(&all, &places);
    let offered: Vec<&Source> = all.iter().filter(|s| s.slot.is_none()).collect();
    let mut made = Made {
        plan: DayPlan {
            schema: wire::SCHEMA.into(),
            agent: inputs.agent.into(),
            date: date.clone(),
            utc_offset: inputs.utc_offset,
            made_at: inputs.now,
            bound: inputs.bound.into(),
            blocks: Vec::new(),
            current: None,
            steps: Vec::new(),
            replans: Vec::new(),
        },
        rejected: Vec::new(),
        called: false,
        usd: None,
        model: String::new(),
    };
    let room = MAX_BLOCKS.saturating_sub(blocks.len());
    if !offered.is_empty() && room > 0 {
        let (system, prompt) = draft_prompt(
            inputs.agent,
            &date,
            minute,
            &offered,
            &blocks,
            &places,
            room,
        );
        let reply = writer.write(&system, &prompt)?;
        made.called = true;
        made.usd = reply.usd;
        made.model = reply.model;
        for drafted in parse_draft(&reply.text)? {
            if blocks.len() == MAX_BLOCKS {
                made.rejected
                    .push((drafted.source, format!("the day holds {MAX_BLOCKS} blocks")));
                continue;
            }
            match check(&drafted, &offered, &places, &blocks, minute, screen) {
                Ok(block) => blocks.push(block),
                Err(why) => made.rejected.push((drafted.source, why)),
            }
        }
    }
    blocks.sort_by_key(|b| b.start);
    made.plan.blocks = blocks;
    Ok(made)
}

/// Moves `plan` to the block under way at local `minute`. Returns the
/// block's index when one started since the last call, so the caller
/// decomposes it; the steps of a block that ended go with it.
pub fn begin(plan: &mut DayPlan, minute: u32) -> Option<usize> {
    let at = plan.block_at(minute);
    let current = plan.current.and_then(|i| usize::try_from(i).ok());
    if at == current {
        return None;
    }
    plan.current = at.and_then(|i| u32::try_from(i).ok());
    plan.steps.clear();
    at
}

/// The decomposition call's system text and prompt for `block` from
/// `from`.
#[must_use]
pub fn steps_prompt(agent: &str, block: &Block, source: &str, from: u32) -> (String, String) {
    let system = format!(
        "You are {agent}, the owner's workshop agent, planning the next hour of your work. Run no \
         commands: finish on this step and put only the JSON asked for in your reply."
    );
    let until = block.end.min(from + STEP_WINDOW);
    let user = format!(
        "You are starting the block \"{}\" ({}-{}), which works {} ({source}). Break {}-{} into \
         steps of {STEP_MIN} to {STEP_MAX} minutes each, in order, without gaps or overlaps, each \
         a concrete action on that work and nothing else. Reply with JSON: {{\"steps\": \
         [{{\"start\": \"{}\", \"minutes\": 10, \"text\": \"...\"}}]}}",
        block.title,
        clock(block.start),
        clock(block.end),
        block.source,
        clock(from),
        clock(until),
        clock(from),
    );
    (system, user)
}

/// The steps for the current block, from `from` (its start, or now when it
/// started earlier), through one call to `writer`. Code keeps the steps in
/// order that fit the window and last [`STEP_MIN`] to [`STEP_MAX`]
/// minutes, at most [`MAX_STEPS`].
///
/// # Errors
/// When there is no current block, or the call or its reply fails.
pub fn decompose(
    plan: &DayPlan,
    source_text: &str,
    from: u32,
    writer: &mut dyn Writer,
    screen: &secret_screen::Screen,
) -> Result<(Vec<Step>, Option<f64>), String> {
    #[derive(Deserialize)]
    struct Drafted {
        start: String,
        minutes: u32,
        text: String,
    }
    #[derive(Deserialize)]
    struct Reply {
        steps: Vec<Drafted>,
    }
    let block = plan.current_block().ok_or("no block is under way")?;
    let from = from.clamp(block.start, block.end);
    let until = block.end.min(from + STEP_WINDOW);
    let (system, prompt) = steps_prompt(&plan.agent, block, source_text, from);
    let reply = writer.write(&system, &prompt)?;
    let drafted: Reply = serde_json::from_value(json_object(&reply.text)?)
        .map_err(|e| format!("the reply isn't a list of steps: {e}"))?;
    let mut steps: Vec<Step> = Vec::new();
    let mut at = from;
    for step in drafted.steps {
        let Some(start) = wire::minute(&step.start) else {
            continue;
        };
        let end = start + step.minutes;
        let text = one_line(&step.text, MAX_TITLE);
        if start < at
            || end > until
            || !(STEP_MIN..=STEP_MAX).contains(&step.minutes)
            || text.is_empty()
            || screen.check(&text).is_err()
        {
            continue;
        }
        steps.push(Step { start, end, text });
        at = end;
        if steps.len() == MAX_STEPS {
            break;
        }
    }
    Ok((steps, reply.usd))
}

/// Something that happened while she works her plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The owner asked her for something: always an interruption.
    Owner { source: String, text: String },
    /// A standing job with a schedule fired.
    Job { job: String, text: String },
    /// Anything else, such as a watched issue or the default branch
    /// moving: Jev decides.
    Observed { source: String, text: String },
}

/// What she does about an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Continue,
    ReactNow,
    Defer,
}

impl Reaction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::ReactNow => "react_now",
            Self::Defer => "defer",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        [Self::Continue, Self::ReactNow, Self::Defer]
            .into_iter()
            .find(|r| r.as_str() == word)
    }
}

/// Decides an observed event's reaction.
pub trait Judge {
    /// # Errors
    /// When nothing answered; she continues.
    fn react(&mut self, state: &serde_json::Value) -> Result<Reaction, String>;
}

/// The state the react-or-continue question reads.
#[must_use]
pub fn react_state(plan: &DayPlan, minute: u32, source: &str, text: &str) -> serde_json::Value {
    let current = plan.current_block().map(|b| {
        serde_json::json!({
            "title": b.title, "source": b.source, "until": clock(b.end),
        })
    });
    let next = plan
        .blocks
        .iter()
        .find(|b| b.start >= minute && Some(*b) != plan.current_block())
        .map(|b| serde_json::json!({"title": b.title, "starts": clock(b.start)}));
    serde_json::json!({
        "agent": plan.agent,
        "now": clock(minute),
        "current": current,
        "next": next,
        "event": {"what": one_line(text, 400), "source": source},
    })
}

/// The Jev request for `state`.
///
/// # Errors
/// When the state is over the set's bound.
pub fn react_request(state: serde_json::Value) -> Result<jev::SystemOneRequest, String> {
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    Ok(jev::SystemOneRequest::new(state, SET.build(&Fill::None)?))
}

/// What [`react`] decided and did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reacted {
    pub reaction: Reaction,
    /// `code` or `jev`, or `code` with Jev's failure when it fell back.
    pub by: String,
    pub replanned: bool,
}

/// Decides `event` at Unix second `now` and changes `plan` to match: the
/// owner's request interrupts and re-plans; a scheduled job moves the plan
/// to its slot; Jev decides the rest, and on no answer she continues.
pub fn react(
    plan: &mut DayPlan,
    event: &Event,
    now: u64,
    places: &[&Node],
    judge: &mut dyn Judge,
) -> Reacted {
    let (_, minute) = local(now, plan.utc_offset);
    let node = place_for(places, Affordance::Work);
    match event {
        Event::Owner { source, text } => {
            let block = Block {
                start: minute,
                end: (minute + REACTION_MINUTES).min(DAY),
                title: one_line(text, MAX_TITLE),
                source: source.clone(),
                node,
                by: By::Owner,
            };
            replan(
                plan,
                now,
                minute,
                block,
                Replanned::Interrupt,
                "the owner asked",
            );
            Reacted {
                reaction: Reaction::ReactNow,
                by: "code".into(),
                replanned: true,
            }
        }
        Event::Job { job, .. } => {
            let source = format!("job:{job}");
            if let Some(index) = plan.blocks.iter().position(|b| b.source == source) {
                if plan.current.and_then(|i| usize::try_from(i).ok()) != Some(index) {
                    plan.current = u32::try_from(index).ok();
                    plan.steps.clear();
                }
            }
            Reacted {
                reaction: Reaction::Continue,
                by: "code".into(),
                replanned: false,
            }
        }
        Event::Observed { source, text } => {
            let state = react_state(plan, minute, source, text);
            let (reaction, by) = match judge.react(&state) {
                Ok(reaction) => (reaction, "jev".to_string()),
                Err(why) => (Reaction::Continue, format!("code ({why})")),
            };
            let title = one_line(text, MAX_TITLE);
            let replanned = match reaction {
                Reaction::Continue => false,
                Reaction::ReactNow => {
                    let block = Block {
                        start: minute,
                        end: (minute + REACTION_MINUTES).min(DAY),
                        title,
                        source: source.clone(),
                        node,
                        by: By::Reaction,
                    };
                    replan(plan, now, minute, block, Replanned::React, "Jev: react now");
                    true
                }
                Reaction::Defer => {
                    let from = plan.current_block().map_or(minute, |b| b.end.max(minute));
                    let block = Block {
                        start: from,
                        end: (from + REACTION_MINUTES).min(DAY),
                        title,
                        source: source.clone(),
                        node,
                        by: By::Reaction,
                    };
                    replan(plan, now, from, block, Replanned::Defer, "Jev: defer");
                    true
                }
            };
            Reacted {
                reaction,
                by,
                replanned,
            }
        }
    }
}

/// Re-plans `plan` from local `minute` on, by code: the blocks before stay,
/// the block under way ends at `minute`, `inserted` starts there, and the
/// blocks after it follow in order, each as soon as the one before ends.
/// A scheduled job keeps its slot, and a block that no longer fits before
/// midnight is dropped and named in the history. Past [`MAX_BLOCKS`], the
/// oldest finished blocks go first, then the last work still to come.
/// The inserted block is current when it starts at `minute` now.
pub fn replan(
    plan: &mut DayPlan,
    now: u64,
    minute: u32,
    inserted: Block,
    kind: Replanned,
    why: &str,
) {
    let mut kept: Vec<Block> = Vec::new();
    let mut later: Vec<Block> = Vec::new();
    for mut block in std::mem::take(&mut plan.blocks) {
        if block.end <= minute {
            kept.push(block);
        } else if block.start < minute {
            // The block under way stops here; what's left of it follows.
            let left = block.end - minute;
            let mut rest = block.clone();
            block.end = minute;
            kept.push(block);
            rest.start = minute;
            rest.end = minute + left;
            later.push(rest);
        } else {
            later.push(block);
        }
    }
    let mut dropped = Vec::new();
    let fixed: Vec<Block> = later.iter().filter(|b| b.by == By::Code).cloned().collect();
    let mut moving: Vec<Block> = vec![inserted.clone()];
    moving.extend(later.into_iter().filter(|b| b.by != By::Code));
    let mut placed: Vec<Block> = fixed;
    let mut at = minute;
    for mut block in moving {
        let length = block.end - block.start;
        let mut start = at.max(block.start);
        // Skip past any fixed block it would overlap.
        while let Some(clash) = placed
            .iter()
            .filter(|b| start < b.end && b.start < start + length)
            .map(|b| b.end)
            .max()
        {
            start = clash;
        }
        if start + length > DAY {
            dropped.push(block.title);
            continue;
        }
        block.start = start;
        block.end = start + length;
        at = block.end;
        placed.push(block);
    }
    kept.extend(placed);
    kept.sort_by_key(|b| b.start);
    // Over the bound, the oldest finished blocks go first, then the last
    // block of work still to come.
    while kept.len() > MAX_BLOCKS {
        if kept.first().is_some_and(|b| b.end <= minute) {
            kept.remove(0);
        } else if let Some(last) = kept
            .iter()
            .rposition(|b| b.by != By::Code && *b != inserted)
        {
            dropped.push(kept.remove(last).title);
        } else {
            break;
        }
    }
    plan.blocks = kept;
    let current = plan
        .blocks
        .iter()
        .position(|b| b.start == inserted.start && b.source == inserted.source)
        .filter(|_| inserted.start == minute && kind != Replanned::Defer);
    if let Some(index) = current {
        plan.current = u32::try_from(index).ok();
        plan.steps.clear();
    } else {
        let (_, now_minute) = local(now, plan.utc_offset);
        plan.current = plan
            .block_at(now_minute)
            .and_then(|i| u32::try_from(i).ok());
    }
    plan.replans.push(Replan {
        at: now,
        minute,
        kind,
        source: inserted.source,
        why: why.into(),
        dropped,
    });
    let over = plan.replans.len().saturating_sub(MAX_REPLANS);
    plan.replans.drain(..over);
}

/// A studio seat's plan: its tasks from the coordinator's queue, in queue
/// order, each a block at the seat's desk. No model call writes it.
/// Running tasks come first, then queued and held ones, each
/// [`JOB_MINUTES`] from `now`.
#[must_use]
pub fn seat_plan(
    view: &coder_host::access::studio::View,
    seat: &str,
    node: &str,
    now: u64,
    utc_offset: i32,
) -> DayPlan {
    use coder_host::access::studio::TaskStatus;
    let (date, minute) = local(now, utc_offset);
    let order = |status: TaskStatus| match status {
        TaskStatus::Running => Some(0),
        TaskStatus::Waiting => Some(1),
        TaskStatus::Queued | TaskStatus::Held => Some(2),
        _ => None,
    };
    let mut tasks: Vec<_> = view
        .tasks
        .iter()
        .filter(|t| t.seat == seat)
        .filter_map(|t| order(t.status).map(|o| (o, t.position, t)))
        .collect();
    tasks.sort_by_key(|(o, p, _)| (*o, *p));
    let mut blocks = Vec::new();
    let mut at = minute;
    for (_, _, task) in tasks.into_iter().take(MAX_BLOCKS) {
        let end = (at + JOB_MINUTES).min(DAY);
        if end <= at {
            break;
        }
        blocks.push(Block {
            start: at,
            end,
            title: one_line(&task.title, MAX_TITLE),
            source: format!("task:{}", task.task),
            node: node.into(),
            by: By::Studio,
        });
        at = end;
    }
    DayPlan {
        schema: wire::SCHEMA.into(),
        agent: seat.into(),
        date,
        utc_offset,
        made_at: now,
        bound: node.into(),
        current: (!blocks.is_empty()).then_some(0),
        blocks,
        steps: Vec::new(),
        replans: Vec::new(),
    }
}

/// The plan as text for a person: its date and bound, every block with
/// who placed it and where, the current block's steps, and every re-plan.
#[must_use]
pub fn text(plan: &DayPlan) -> String {
    let mut out = format!("{} for {}, within {}\n", plan.agent, plan.date, plan.bound);
    if plan.idle() {
        out.push_str("no work today: idle at her desk\n");
    }
    let current = plan.current.and_then(|i| usize::try_from(i).ok());
    for (index, block) in plan.blocks.iter().enumerate() {
        let by = match block.by {
            By::Code => "code",
            By::Model => "drafted",
            By::Owner => "owner",
            By::Reaction => "reaction",
            By::Studio => "studio",
        };
        out.push_str(&format!(
            "{} {}-{} {} [{}] by {by} at {}\n",
            if current == Some(index) { ">" } else { " " },
            clock(block.start),
            clock(block.end),
            block.title,
            block.source,
            block.node
        ));
        if current == Some(index) {
            for step in &plan.steps {
                out.push_str(&format!(
                    "      {}-{} {}\n",
                    clock(step.start),
                    clock(step.end),
                    step.text
                ));
            }
        }
    }
    for replan in &plan.replans {
        out.push_str(&format!(
            "re-planned from {} ({:?}, {}): {}{}\n",
            clock(replan.minute),
            replan.kind,
            replan.source,
            replan.why,
            if replan.dropped.is_empty() {
                String::new()
            } else {
                format!("; dropped {}", replan.dropped.join(", "))
            }
        ));
    }
    out
}

/// `agents/NAME/plan.json`.
#[must_use]
pub fn path(store: &Store) -> PathBuf {
    store.dir().join("plan.json")
}

/// The stored plan.
///
/// # Errors
/// When the file exists and doesn't read as a plan.
pub fn load(store: &Store) -> Result<Option<DayPlan>, String> {
    let path = path(store);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let plan: DayPlan = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a day plan: {e}", path.display()))?;
    plan.validate()
        .map_err(|e| format!("{} is out of bounds: {e}", path.display()))?;
    Ok(Some(plan))
}

/// Writes `plan` in place of the stored one.
///
/// # Errors
/// When it is out of bounds or the file cannot be written.
pub fn save(store: &Store, plan: &DayPlan) -> Result<(), String> {
    plan.validate().map_err(|e| e.to_string())?;
    let body = serde_json::to_vec_pretty(plan).map_err(|e| e.to_string())?;
    let temp = store.dir().join(".plan.json.tmp");
    std::fs::write(&temp, body).map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&temp, path(store))
        .map_err(|e| format!("cannot write {}: {e}", path(store).display()))
}

/// Serializes every write of a plan file in this process: the host's
/// sweep moves the current block and records reactions while a planning
/// thread writes a new plan or a block's steps.
static WRITE: Mutex<()> = Mutex::new(());

fn hold() -> MutexGuard<'static, ()> {
    WRITE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Stores `plan`, a new day's, under the plan lock.
///
/// # Errors
/// When it is out of bounds or cannot be written.
pub fn replace(store: &Store, plan: &DayPlan) -> Result<(), String> {
    let _write = hold();
    save(store, plan)
}

/// The stored plan when it is for the local day at `now`.
#[must_use]
pub fn today(store: &Store, now: u64) -> Option<DayPlan> {
    load(store)
        .ok()
        .flatten()
        .filter(|plan| plan.date == local(now, plan.utc_offset).0)
}

/// Changes today's stored plan with `change` under the plan lock and
/// writes it back. `None` when there is no plan for the day at `now`.
///
/// # Errors
/// When the file cannot be read or written.
pub fn update<R>(
    store: &Store,
    now: u64,
    change: impl FnOnce(&mut DayPlan) -> R,
) -> Result<Option<R>, String> {
    let _write = hold();
    let Some(mut plan) = load(store)? else {
        return Ok(None);
    };
    if plan.date != local(now, plan.utc_offset).0 {
        return Ok(None);
    }
    let result = change(&mut plan);
    save(store, &plan)?;
    Ok(Some(result))
}

/// Journals a new plan: its blocks by who placed them, and each refused
/// draft with why.
///
/// # Errors
/// When the journal cannot be written.
pub fn journal_made(store: &Store, made: &Made) -> Result<(), String> {
    let plan = &made.plan;
    let by = |who: By| plan.blocks.iter().filter(|b| b.by == who).count();
    let text = if plan.idle() {
        format!("day plan for {}: no work, so an idle day", plan.date)
    } else {
        format!(
            "day plan for {}: {} blocks ({} by code, {} drafted{}): {}",
            plan.date,
            plan.blocks.len(),
            by(By::Code),
            by(By::Model),
            if made.called { "" } else { ", no model call" },
            plan.blocks
                .iter()
                .map(|b| format!("{} {}", clock(b.start), b.source))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    store.append(&Entry::new(plan.made_at, Kind::Plan, &text))?;
    for (source, why) in &made.rejected {
        store.append(&Entry::new(
            plan.made_at,
            Kind::Plan,
            &format!("day plan refused a drafted block for {source}: {why}"),
        ))?;
    }
    Ok(())
}

/// What a plan runs on: the model that drafts and decomposes, and Jev for
/// reactions.
pub struct Services {
    pub writer: Box<dyn Writer + Send>,
    pub judge: Box<dyn Judge + Send>,
}

/// Makes the [`Services`] for one agent.
pub type ServicesFactory = Arc<dyn Fn(&Store) -> Result<Services, String> + Send + Sync>;

/// The live services, or, in a unit test, a refusal: no model runs there.
#[must_use]
pub fn default_factory() -> ServicesFactory {
    if cfg!(test) {
        Arc::new(|_: &Store| Err("no planning model runs in a unit test".to_string()))
    } else {
        Arc::new(live::services)
    }
}

/// A writer that replays recorded replies in order, for tests, captures,
/// and the interview's recorded plans.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    pub replies: std::collections::VecDeque<String>,
    pub usd: Option<f64>,
    /// Every prompt it was given.
    pub prompts: Vec<String>,
}

impl Scripted {
    #[must_use]
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(replies: I) -> Self {
        Self {
            replies: replies.into_iter().map(Into::into).collect(),
            usd: Some(0.0),
            prompts: Vec::new(),
        }
    }
}

impl Writer for Scripted {
    fn write(
        &mut self,
        _system: &str,
        prompt: &str,
    ) -> Result<super::agent_reflect::Reply, String> {
        self.prompts.push(prompt.to_string());
        let text = self
            .replies
            .pop_front()
            .ok_or("the script has no reply left")?;
        Ok(super::agent_reflect::Reply {
            text,
            model: "scripted".into(),
            usd: self.usd,
        })
    }
}

/// A judge that answers each event from a list, for tests.
#[derive(Clone, Debug, Default)]
pub struct Answers {
    pub answers: std::collections::VecDeque<Result<Reaction, String>>,
    pub asked: Vec<serde_json::Value>,
}

impl Judge for Answers {
    fn react(&mut self, state: &serde_json::Value) -> Result<Reaction, String> {
        self.asked.push(state.clone());
        self.answers
            .pop_front()
            .unwrap_or_else(|| Err("no answer left".into()))
    }
}

/// The sources' IDs, for a check that every block names one.
#[must_use]
pub fn source_ids(sources: &[Source]) -> BTreeSet<String> {
    sources.iter().map(|s| s.id.clone()).collect()
}

#[cfg(test)]
#[path = "agent_plan_tests.rs"]
mod tests;
