//! A workshop agent's day plan (`openagents.agent-day-plan.v1`,
//! `docs/verse/generative-agents.md`, item 4), as the host keeps it in
//! `agents/NAME/plan.json` and a device reads it in
//! [`crate::agent::AgentView::plan`].
//!
//! A plan is the local date, at most [`MAX_BLOCKS`] blocks that each name
//! the real work they come from (`job:ID`, `issue:N`, `request:N`,
//! `journal:POS`, or `memory:ID`) and the world-tree node the agent works
//! at, the current block's steps, and the re-plan history. A day with no
//! work has no blocks: the agent idles at her desk. The host makes and
//! changes plans (`coder::task::agent_plan`); a device only draws them.

use serde::{Deserialize, Serialize};

use crate::{Code, Result, fail};

/// The plan's schema.
pub const SCHEMA: &str = "openagents.agent-day-plan.v1";
/// The most blocks in a day.
pub const MAX_BLOCKS: usize = 8;
/// The most steps the current block holds.
pub const MAX_STEPS: usize = 12;
/// The most re-plans the history keeps; the oldest go first.
pub const MAX_REPLANS: usize = 16;
/// The longest block title or step text.
pub const MAX_TITLE: usize = 160;
/// Minutes in a day.
pub const DAY: u32 = 24 * 60;

/// Who placed a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// Code placed it: a scheduled standing job in its slot.
    Code,
    /// The morning plan's model call drafted it from a listed source.
    Model,
    /// The owner's request interrupted the plan.
    Owner,
    /// A reaction to an event, by Jev's choice.
    Reaction,
    /// A studio seat's task queue.
    Studio,
}

/// One block of the day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    /// Minutes after local midnight.
    pub start: u32,
    pub end: u32,
    pub title: String,
    /// The real work it comes from.
    pub source: String,
    /// The world-tree node she works at.
    pub node: String,
    pub by: By,
}

/// One step of the current block, 5 to 15 minutes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

/// How a re-plan came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Replanned {
    /// The owner's request interrupted.
    Interrupt,
    /// Jev chose to react now.
    React,
    /// Jev chose to take the event up as the next block.
    Defer,
}

/// One re-plan, from the current block on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replan {
    /// Unix seconds.
    pub at: u64,
    /// The local minute it re-planned from.
    pub minute: u32,
    pub kind: Replanned,
    /// The event's source.
    pub source: String,
    pub why: String,
    /// The blocks it moved past the end of the day and dropped, by title.
    #[serde(default)]
    pub dropped: Vec<String>,
}

/// A day plan (`openagents.agent-day-plan.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DayPlan {
    pub schema: String,
    pub agent: String,
    /// The local date, `YYYY-MM-DD`.
    pub date: String,
    /// The owner's time zone, minutes east of UTC.
    #[serde(default)]
    pub utc_offset: i32,
    /// Unix seconds.
    pub made_at: u64,
    /// The world-tree node her walks stay within.
    pub bound: String,
    /// In time order, without overlaps.
    pub blocks: Vec<Block>,
    /// The block under way, by index.
    #[serde(default)]
    pub current: Option<u32>,
    /// The current block's steps, made when it started.
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub replans: Vec<Replan>,
}

/// `HH:MM` for a minute of the day.
#[must_use]
pub fn clock(minute: u32) -> String {
    format!("{:02}:{:02}", (minute / 60) % 24, minute % 60)
}

/// The minute of the day `HH:MM` names.
#[must_use]
pub fn minute(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl DayPlan {
    /// A plan with no blocks: an idle day.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.blocks.is_empty()
    }

    /// The block under way.
    #[must_use]
    pub fn current_block(&self) -> Option<&Block> {
        self.current
            .and_then(|index| self.blocks.get(usize::try_from(index).ok()?))
    }

    /// The block whose time holds local minute `minute`.
    #[must_use]
    pub fn block_at(&self, minute: u32) -> Option<usize> {
        self.blocks
            .iter()
            .position(|b| b.start <= minute && minute < b.end)
    }

    /// Checks the bounds a device relies on.
    ///
    /// # Errors
    /// `bounds` or `malformed`, with which.
    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA {
            return fail(Code::Malformed, "not a day plan this device reads");
        }
        if self.blocks.len() > MAX_BLOCKS
            || self.steps.len() > MAX_STEPS
            || self.replans.len() > MAX_REPLANS
        {
            return fail(Code::Bounds, "the day plan exceeds its bounds");
        }
        let mut end = 0;
        for block in &self.blocks {
            if block.start < end || block.end <= block.start || block.end > DAY {
                return fail(
                    Code::Malformed,
                    "the day plan's blocks overlap or leave the day",
                );
            }
            if block.title.trim().is_empty()
                || block.title.len() > MAX_TITLE
                || block.source.is_empty()
                || block.node.is_empty()
            {
                return fail(
                    Code::Bounds,
                    "a block's title, source, or node is out of bounds",
                );
            }
            end = block.end;
        }
        if self
            .current
            .is_some_and(|i| usize::try_from(i).map_or(true, |i| i >= self.blocks.len()))
        {
            return fail(Code::Malformed, "the current block isn't in the plan");
        }
        if self
            .steps
            .iter()
            .any(|s| s.end <= s.start || s.text.len() > MAX_TITLE)
        {
            return fail(Code::Bounds, "a step is out of bounds");
        }
        Ok(())
    }

    /// The plan as lines a board or a panel draws: one per block, the
    /// current one marked `>`, its steps under it, and the last re-plan.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        if self.idle() {
            return vec![format!("{}: no work today, idle at her desk", self.date)];
        }
        let mut out = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let current = self.current.and_then(|i| usize::try_from(i).ok()) == Some(index);
            out.push(format!(
                "{} {}-{} {} [{}]",
                if current { ">" } else { " " },
                clock(block.start),
                clock(block.end),
                block.title,
                block.source
            ));
            if current {
                for step in &self.steps {
                    out.push(format!("    {} {}", clock(step.start), step.text));
                }
            }
        }
        if let Some(replan) = self.replans.last() {
            out.push(format!(
                "re-planned at {} ({}): {}",
                clock(replan.minute),
                match replan.kind {
                    Replanned::Interrupt => "your request",
                    Replanned::React => "reacted",
                    Replanned::Defer => "deferred",
                },
                replan.why
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(start: u32, end: u32) -> Block {
        Block {
            start,
            end,
            title: "Work issue 7".into(),
            source: "issue:7".into(),
            node: "everglade/x".into(),
            by: By::Model,
        }
    }

    #[test]
    fn a_plan_round_trips_validates_and_draws() {
        let mut plan = DayPlan {
            schema: SCHEMA.into(),
            agent: "alice".into(),
            date: "2026-10-07".into(),
            utc_offset: 0,
            made_at: 1,
            bound: "everglade/house".into(),
            blocks: vec![block(540, 600), block(600, 660)],
            current: Some(0),
            steps: vec![Step {
                start: 540,
                end: 550,
                text: "read the issue".into(),
            }],
            replans: Vec::new(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(serde_json::from_str::<DayPlan>(&json).unwrap(), plan);
        assert!(plan.validate().is_ok());
        let lines = plan.lines();
        assert_eq!(lines[0], "> 09:00-10:00 Work issue 7 [issue:7]");
        assert_eq!(lines[1], "    09:00 read the issue");
        assert_eq!(plan.block_at(605), Some(1));
        plan.blocks[1].start = 590;
        assert!(plan.validate().is_err(), "overlap");
        assert_eq!(minute("07:05"), Some(425));
        assert_eq!(minute("24:00"), None);
        assert_eq!(clock(425), "07:05");
    }
}
