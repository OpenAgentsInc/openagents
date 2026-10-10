//! The rule type: one JSON document per rule at
//! `~/.openagents/background/rules/<id>.json`, versioned and digested.
//!
//! Phase 1 ships one built-in rule, [`disk`]. A rule file that is absent
//! means the built-in defaults; editing it (`openagents background edit
//! disk`) writes the file, which then wins.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The schema every rule document carries.
pub const SCHEMA: &str = "openagents.background.rule.v1";

/// One gigabyte, as the policy counts it (10^9 bytes).
pub const GB: u64 = 1_000_000_000;

/// A start level this high means "clean whatever qualifies, whatever the
/// free space": the pruning rules use it.
pub const ALWAYS: u64 = 1_000_000 * GB;

/// A durable, user-defined rule the host runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub schema: String,
    pub id: String,
    pub name: String,
    /// Bumped by every edit.
    pub version: u64,
    pub origin: Origin,
    pub enabled: bool,
    /// While set and in the future, the rule is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    pub triggers: Vec<Trigger>,
    /// Typed predicates that must all hold for a triggered evaluation to
    /// act (phase 2). Empty for the disk rule, whose condition is its
    /// goal's start level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    pub goal: Goal,
    /// Run in order until the goal is met.
    pub actions: Vec<Action>,
    pub classes: Classes,
    pub safety: Safety,
    pub cooldown_secs: u64,
    /// When the actions fall short of the goal, start a Coder run that
    /// proposes rule changes (phase 3). At most one a day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalate: Option<Escalate>,
    /// What a plugin's rule asks the host for. Empty for the built-in
    /// rule, which the host trusts; a plugin's rule is admitted only
    /// within what it names ([`crate::plugins::admit`]).
    #[serde(default, skip_serializing_if = "Needs::is_empty")]
    pub needs: Needs,
}

/// What a plugin's background rule needs from the host. The host grants
/// no more: an action whose class is not in `delete` refuses the rule, and
/// every safety check stays the host's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Needs {
    /// The candidate classes the rule may delete (the `fs.delete`
    /// capability, limited to the host's classes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delete: Vec<Class>,
    /// Read the Coder task store (which tasks ended).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tasks: bool,
    /// Send notifications.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub notify: bool,
    /// Start Coder runs (escalation, or a `StartCoderRun` action).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub coder: bool,
}

impl Needs {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.delete.is_empty() && !self.tasks && !self.notify && !self.coder
    }
}

/// Escalation to a Coder run when a rule falls short.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escalate {
    /// The checkout the run works in; the host's default when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// What a health watch probes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Watched {
    /// The relay this computer publishes to.
    Relay,
    /// This computer's host.
    Host,
}

impl Watched {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Watched::Relay => "relay",
            Watched::Host => "host",
        }
    }
}

/// A folder the person confirmed as a disposable cache after Jev judged
/// it one (or a plugin proposed it). Later runs need no model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Judged {
    pub path: String,
    /// `build_output`, `package_cache`, or `app_cache`.
    pub kind: String,
    /// When the person confirmed it.
    pub confirmed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    BuiltIn,
    File {
        path: String,
    },
    /// Contributed by an installed plugin, `KEY:SLUG` at `version`. Set by
    /// the host when it admits the rule, never by the plugin.
    Plugin {
        plugin: String,
        version: String,
    },
    /// Compiled from a message in conversation (phase 2) and confirmed by
    /// the person: the chat thread (or `cli`) and the message's words.
    Conversation {
        thread: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Trigger {
    /// A check every `every_secs`.
    Interval { every_secs: u64 },
    /// Free space falls below the goal's start threshold, checked on the
    /// interval.
    Threshold,
    /// A Coder task ends.
    TaskEnded,
    /// The host starts.
    HostStart,
    /// Every day at a local wall-clock time `HH:MM`. A run missed while
    /// the computer slept runs once when the runner next looks.
    Daily { at: String },
    /// A watched path changed: created, removed, or modified (its size or
    /// modification time moved). Paths are absolute or under `~`.
    FsEvent { paths: Vec<String> },
}

/// How a Coder task ended, for [`Condition::TaskOutcome`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutcome {
    /// It finished and nothing failed.
    Succeeded,
    /// Its run or its checks failed.
    Failed,
    /// It was cancelled.
    Cancelled,
}

/// A typed predicate over what code observes before acting. All of a
/// rule's conditions must hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// The fullest watched volume has less free space than `level`.
    FreeBelow { level: Level },
    /// The task whose end triggered the evaluation ended this way. Holds
    /// only for a `TaskEnded` evaluation.
    TaskOutcome { outcomes: Vec<TaskOutcome> },
    /// No Coder task is queued or running.
    NoTaskRunning,
    /// A path exists (absolute or under `~`).
    PathExists { path: String },
    /// The local time is within `[from, to)`, `HH:MM`; `to` before `from`
    /// wraps past midnight.
    TimeBetween { from: String, to: String },
    /// The local day of the week is one of `days`, 0 Sunday to 6 Saturday
    /// (a prompt scheduled for weekdays, #11177).
    Weekdays { days: Vec<u8> },
    /// A bounded Jev judgment: the Noul `question` over the observation,
    /// read against the named setting (`background.judgment` by default)
    /// at `threshold` percent. The only place a model appears in
    /// evaluation; without a judge it does not hold.
    Judgment {
        question: String,
        setting: String,
        threshold: u8,
    },
}

/// A day of the week's name, 0 Sunday to 6 Saturday.
#[must_use]
pub fn day_name(day: u8) -> &'static str {
    match day {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        _ => "Saturday",
    }
}

/// Parse `HH:MM` into minutes past midnight.
#[must_use]
pub fn clock(at: &str) -> Option<u32> {
    let (hours, minutes) = at.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let (hours, minutes): (u32, u32) = (hours.parse().ok()?, minutes.parse().ok()?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// `max(bytes, percent of the volume)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub bytes: u64,
    pub percent: u8,
}

impl Level {
    /// The bytes this level means on a volume of `total` bytes.
    #[must_use]
    pub fn of(self, total: u64) -> u64 {
        let share = u128::from(total) * u128::from(self.percent) / 100;
        self.bytes.max(u64::try_from(share).unwrap_or(u64::MAX))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    /// Clean when free space is below this.
    pub start: Level,
    /// Stop when free space reaches this.
    pub stop: Level,
    /// Below this, every class runs and the trash empties.
    pub emergency: Level,
    /// Stop after freeing this much in one run.
    pub max_freed: u64,
    /// While the last check found a volume below the start level, check
    /// this often instead of the rule's interval (seconds; at least 60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_secs: Option<u64>,
}

/// The candidate classes, in the order the spec lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// 1: ended tasks' target directories, and slots past the slot count.
    EndedTargets,
    /// 2: idle slots, Coder One's target, agent target directories, and
    /// checkouts' `target/`.
    StaleTargets,
    /// 3: ended tasks' clean, pushed worktrees.
    Worktrees,
    /// 4: gate pools' build directories.
    GatePools,
    /// 5: `debug/incremental` and `release/incremental` of idle target
    /// directories.
    Incremental,
    /// 6: the background trash, in an emergency only (and anything in
    /// it past its 24-hour window).
    Trash,
    /// 7: folders the person confirmed as caches after a judgment. They
    /// move to the trash, never straight to deletion.
    Judged,
    /// 8: Claude Code worktrees (`<checkout>/.claude/worktrees/*`) that
    /// are clean, pushed, unlocked, and idle, under the class 3 checks.
    ClaudeWorktrees,
    /// 9: the kache compile cache, reclaimed only through kache's own
    /// collector; no file under its store is ever deleted here.
    Kache,
    /// 10: agent scratch (`~/.openagents/scratch/*`) of sessions that
    /// ended, unchanged for `scratch_days` ([`crate::scratch`]).
    Scratch,
}

impl Class {
    pub const ALL: [Class; 6] = [
        Class::EndedTargets,
        Class::StaleTargets,
        Class::Worktrees,
        Class::GatePools,
        Class::Incremental,
        Class::Trash,
    ];

    /// Every class a rule may delete from: [`Class::ALL`] (the classes
    /// conversation names) with Claude Code worktrees, the kache store, and
    /// agent scratch.
    pub const DELETABLE: [Class; 9] = [
        Class::EndedTargets,
        Class::StaleTargets,
        Class::Worktrees,
        Class::GatePools,
        Class::Incremental,
        Class::Trash,
        Class::ClaudeWorktrees,
        Class::Kache,
        Class::Scratch,
    ];

    /// The class's number in the spec.
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Class::EndedTargets => 1,
            Class::StaleTargets => 2,
            Class::Worktrees => 3,
            Class::GatePools => 4,
            Class::Incremental => 5,
            Class::Trash => 6,
            Class::Judged => 7,
            Class::ClaudeWorktrees => 8,
            Class::Kache => 9,
            Class::Scratch => 10,
        }
    }

    /// Plain words for one item and several.
    #[must_use]
    pub fn noun(self, count: usize) -> &'static str {
        let one = count == 1;
        match self {
            Class::EndedTargets if one => "build cache from an ended task",
            Class::EndedTargets => "build caches from ended tasks",
            Class::StaleTargets if one => "old build folder",
            Class::StaleTargets => "old build folders",
            Class::Worktrees if one => "finished worktree",
            Class::Worktrees => "finished worktrees",
            Class::GatePools if one => "gate build",
            Class::GatePools => "gate builds",
            Class::Incremental if one => "incremental cache",
            Class::Incremental => "incremental caches",
            Class::Trash if one => "trash folder",
            Class::Trash => "trash folders",
            Class::Judged if one => "confirmed cache",
            Class::Judged => "confirmed caches",
            Class::ClaudeWorktrees if one => "finished Claude Code worktree",
            Class::ClaudeWorktrees => "finished Claude Code worktrees",
            Class::Kache => "kache collection",
            Class::Scratch if one => "scratch of an ended session",
            Class::Scratch => "scratch of ended sessions",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Delete candidates of these classes, least recently used first.
    DeleteCaches { classes: Vec<Class> },
    /// Remove ended tasks' clean, pushed worktrees.
    PruneWorktrees,
    /// Remove `debug/incremental` and `release/incremental` of idle target
    /// directories.
    CargoCleanPartial,
    /// Empty the background trash, oldest first (emergency only).
    EmptyTrash,
    /// Tell the person: the host's log, the rule's state, and the
    /// terminal's transcript. `text` is shown as written.
    Notify { text: String },
    /// Bring a Git checkout up to date with its upstream: `git fetch`,
    /// then `git merge --ff-only`, only when the checkout is clean and
    /// has no commits its upstream lacks; otherwise it changes nothing
    /// and says why. No other command runs.
    GitFastForward {
        repo: String,
        /// Only on this branch; on another it changes nothing and says so.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
    },
    /// Start a Coder run with `prompt` in `workspace` (the host's default
    /// when unset) and a code-built briefing.
    StartCoderRun {
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
    /// Run an installed plugin's declared background action, read-only.
    /// A plugin's rule may run only its own plugin.
    RunPlugin {
        plugin: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        input: String,
    },
    /// Release Coder issue claims whose task ended, or that have not run
    /// for `idle_hours`, with a comment saying why.
    ReleaseStaleClaims { idle_hours: u64 },
    /// Probe the relay or the host; after `failures` failures in a row,
    /// restart it through the service manager and say so.
    HealthWatch { target: Watched, failures: u32 },
    /// Judge each new test failure against the known flakes (Jev), then
    /// update the flake or report a new failure.
    FlakeWatch,
    /// One short line: what ran, what it cost, what finished. A summary,
    /// never a limit.
    UsageSummary,
    /// Compress traces, gate logs, and run artifacts older than
    /// `compress_days`; remove those older than `keep_days`.
    RotateLogs { compress_days: u64, keep_days: u64 },
    /// Refit the decision thresholds from joined run outcomes and adopt
    /// only those that pass their held-out check (#10387).
    Recalibrate,
}

impl Action {
    /// The classes this action works through.
    #[must_use]
    pub fn classes(&self) -> Vec<Class> {
        match self {
            Action::DeleteCaches { classes } => classes.clone(),
            Action::PruneWorktrees => vec![Class::Worktrees],
            Action::CargoCleanPartial => vec![Class::Incremental],
            Action::EmptyTrash => vec![Class::Trash],
            _ => Vec::new(),
        }
    }

    /// Whether this action deletes from the candidate classes (the disk
    /// cleanup planner runs it), rather than notifying or updating a
    /// checkout.
    #[must_use]
    pub fn cleans(&self) -> bool {
        matches!(
            self,
            Action::DeleteCaches { .. }
                | Action::PruneWorktrees
                | Action::CargoCleanPartial
                | Action::EmptyTrash
        )
    }
}

/// Per-class parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classes {
    /// Agent target directories (`~/work/openagents-target-agent*`). A
    /// pattern may end its last component with `*`.
    pub agent_targets: Vec<String>,
    /// Folders whose Git checkouts' `target/` are candidates
    /// (`~/work/*`).
    pub checkouts: Vec<String>,
    /// Slots and Coder One's target untouched this many days are stale.
    pub idle_days: u64,
    /// Agent target directories untouched this many hours are stale. A
    /// rule file without it gets the default, [`AGENT_IDLE_HOURS`].
    #[serde(default = "agent_idle_hours")]
    pub agent_idle_hours: u64,
    /// A checkout's `target/` untouched this long is stale.
    pub checkout_days: u64,
    /// The most recently used agent target directories always kept.
    pub keep: usize,
    /// A worktree with no task record is a candidate only past this age.
    pub orphan_worktree_days: u64,
    /// A gate build used within this many hours stays.
    pub gate_idle_hours: u64,
    /// Classes measured and named but never deleted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_only: Vec<Class>,
    /// An ended task's worktree is a candidate only this many days after
    /// it was last used (0: at once).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub worktree_days: u64,
    /// Folders confirmed as caches (class 7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub judged: Vec<Judged>,
    /// Checkouts whose Claude Code worktrees (`.claude/worktrees/*`) are
    /// class 8 candidates. A pattern may end its last component with `*`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claude_checkouts: Vec<String>,
    /// A Claude Code worktree whose Git state changed within this many
    /// hours stays. A rule file without it gets the default,
    /// [`CLAUDE_WORKTREE_HOURS`].
    #[serde(default = "claude_worktree_hours")]
    pub claude_worktree_hours: u64,
    /// An ended session's scratch unchanged this many days is a class 10
    /// candidate. A rule file without it gets the default,
    /// [`SCRATCH_DAYS`].
    #[serde(default = "scratch_days")]
    pub scratch_days: u64,
}

/// The default age, in days, of an ended session's scratch before it is a
/// candidate.
pub const SCRATCH_DAYS: u64 = 7;

fn scratch_days() -> u64 {
    SCRATCH_DAYS
}

/// The default staleness of an agent target directory, in hours.
pub const AGENT_IDLE_HOURS: u64 = 6;

/// The default idle time, in hours, before a Claude Code worktree is a
/// candidate.
pub const CLAUDE_WORKTREE_HOURS: u64 = 2;

fn agent_idle_hours() -> u64 {
    AGENT_IDLE_HOURS
}

fn claude_worktree_hours() -> u64 {
    CLAUDE_WORKTREE_HOURS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// Allow and deny lists. The built-in deny list ([`crate::paths`]) always
/// applies on top of these.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    /// Measured and named in the report when the goal is not met; never
    /// deleted.
    pub report: Vec<String>,
}

/// The built-in disk cleanup rule with the spec's default policy.
#[must_use]
pub fn disk() -> Rule {
    Rule {
        schema: SCHEMA.into(),
        id: "disk".into(),
        name: "Disk cleanup".into(),
        version: 1,
        origin: Origin::BuiltIn,
        enabled: false,
        paused_until: None,
        triggers: vec![
            Trigger::Interval { every_secs: 300 },
            Trigger::Threshold,
            Trigger::TaskEnded,
            Trigger::HostStart,
        ],
        conditions: Vec::new(),
        goal: Goal {
            start: Level {
                bytes: 200 * GB,
                percent: 15,
            },
            stop: Level {
                bytes: 300 * GB,
                percent: 20,
            },
            emergency: Level {
                bytes: 10 * GB,
                percent: 1,
            },
            max_freed: 100 * GB,
            pressure_secs: Some(60),
        },
        actions: vec![
            Action::DeleteCaches {
                classes: vec![Class::EndedTargets, Class::StaleTargets],
            },
            Action::PruneWorktrees,
            Action::DeleteCaches {
                classes: vec![Class::ClaudeWorktrees],
            },
            Action::DeleteCaches {
                classes: vec![Class::Scratch],
            },
            Action::DeleteCaches {
                classes: vec![Class::GatePools, Class::Kache],
            },
            Action::CargoCleanPartial,
            Action::EmptyTrash,
        ],
        classes: Classes {
            agent_targets: vec!["~/work/openagents-target-agent*".into()],
            checkouts: vec!["~/work/*".into()],
            idle_days: 3,
            agent_idle_hours: AGENT_IDLE_HOURS,
            checkout_days: 7,
            keep: 0,
            orphan_worktree_days: 7,
            gate_idle_hours: 1,
            report_only: Vec::new(),
            worktree_days: 0,
            judged: Vec::new(),
            claude_checkouts: vec!["~/code/*".into(), "~/work/*".into()],
            claude_worktree_hours: CLAUDE_WORKTREE_HOURS,
            scratch_days: SCRATCH_DAYS,
        },
        safety: Safety {
            allow: vec![
                "~/.openagents/targets".into(),
                "~/.openagents/coder-one/target".into(),
                "~/.openagents/worktrees".into(),
                "~/.openagents/gate".into(),
                "~/.openagents/background/trash".into(),
                "~/.openagents/scratch".into(),
                "~/work".into(),
                "~/code".into(),
            ],
            deny: Vec::new(),
            report: vec![
                "~/.openagents/pylon".into(),
                "~/Library/Caches".into(),
                "/private/var/folders".into(),
            ],
        },
        cooldown_secs: 600,
        escalate: None,
        needs: Needs::default(),
    }
}

/// The built-in rules, by id. `disk` first; the rest are the spec's
/// other background processes ([`crate::builtins`]).
pub const BUILT_IN: [&str; 10] = [
    "disk",
    "worktrees",
    "claims",
    "checkout",
    "health",
    "flakes",
    "qa",
    "usage",
    "rotate",
    "calibration",
];

/// The built-in rules, by id.
#[must_use]
pub fn built_in(id: &str) -> Option<Rule> {
    match id {
        "disk" => Some(disk()),
        _ => crate::builtins::rule(id),
    }
}

impl Rule {
    /// The rule's digest: SHA-256 of its JSON.
    #[must_use]
    pub fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let hash = Sha256::digest(&bytes);
        let mut out = String::from("sha256:");
        for byte in hash {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    /// Whether the rule runs on its triggers at `now`.
    #[must_use]
    pub fn active(&self, now: u64) -> bool {
        self.enabled && self.paused_until.is_none_or(|until| until <= now)
    }

    /// The check interval.
    #[must_use]
    pub fn interval(&self) -> Option<u64> {
        self.triggers.iter().find_map(|trigger| match trigger {
            Trigger::Interval { every_secs } => Some(*every_secs),
            _ => None,
        })
    }

    /// The check interval given the last observation of the fullest
    /// volume (`free` of `total`): the goal's pressure interval while free
    /// space is below the start level, else [`Rule::interval`].
    #[must_use]
    pub fn interval_at(&self, free: Option<u64>, total: Option<u64>) -> Option<u64> {
        let every = self.interval()?;
        Some(match (free, total, self.goal.pressure_secs) {
            (Some(free), Some(total), Some(pressure))
                if self.cleans() && free < self.goal.start.of(total) =>
            {
                every.min(pressure)
            }
            _ => every,
        })
    }

    #[must_use]
    pub fn has(&self, wanted: &Trigger) -> bool {
        self.triggers.iter().any(|trigger| trigger == wanted)
    }

    /// Whether the rule cleans the disk: any of its actions deletes from
    /// the candidate classes. Such a rule runs when free space is below
    /// its goal's start level; a rule without one acts whenever its
    /// conditions hold.
    #[must_use]
    pub fn cleans(&self) -> bool {
        self.actions.iter().any(Action::cleans)
    }

    /// Whether the rule was made in conversation.
    #[must_use]
    pub fn conversational(&self) -> bool {
        matches!(self.origin, Origin::Conversation { .. })
    }

    /// Check a rule before it is saved.
    ///
    /// # Errors
    /// A plain sentence naming the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema must be {SCHEMA}"));
        }
        let plugin = matches!(self.origin, Origin::Plugin { .. });
        let conversation = self.conversational();
        if !plugin && !conversation && built_in(&self.id).is_none() {
            return Err(format!(
                "`{}` is not a rule this host runs: the built-in `disk`, a rule an enabled plugin brings, or one made in conversation",
                self.id
            ));
        }
        if (plugin || conversation) && built_in(&self.id).is_some() {
            return Err(format!("only the built-in rule can be `{}`", self.id));
        }
        if !id_like(&self.id) {
            return Err(format!(
                "a rule id is 1 to 40 lowercase letters, digits, and dashes, not `{}`",
                self.id
            ));
        }
        self.validate_phase2()?;
        if self.goal.stop.bytes < self.goal.start.bytes
            || self.goal.stop.percent < self.goal.start.percent
        {
            return Err("the stop level must be at or above the start level".into());
        }
        if self.goal.emergency.bytes > self.goal.start.bytes
            || self.goal.emergency.percent > self.goal.start.percent
        {
            return Err("the emergency level must be at or below the start level".into());
        }
        if [self.goal.start, self.goal.stop, self.goal.emergency]
            .iter()
            .any(|level| level.percent > 90)
        {
            return Err("a percent must be at most 90".into());
        }
        if self.interval().is_some_and(|every| every < 60)
            || self.goal.pressure_secs.is_some_and(|every| every < 60)
        {
            return Err("checks must be at least a minute apart".into());
        }
        if self.classes.agent_idle_hours == 0 {
            return Err("an agent build is stale after at least an hour".into());
        }
        if self.classes.scratch_days == 0 {
            return Err("an ended session's scratch stays at least a day".into());
        }
        for root in &self.safety.allow {
            if !(root.starts_with("~/") || root.starts_with('/')) || root.contains("..") {
                return Err(format!("allow root `{root}` must be absolute or under ~"));
            }
            if root == "~/" || root == "/" || root == "~" {
                return Err("an allow root cannot be the whole home or disk".into());
            }
        }
        Ok(())
    }
}

/// Whether `id` is a rule id: 1 to 40 of `a-z`, `0-9`, and `-`.
#[must_use]
pub fn id_like(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn path_like(path: &str) -> bool {
    (path.starts_with("~/") || path.starts_with('/'))
        && !path.contains("..")
        && path.len() <= 512
        && !path.chars().any(char::is_control)
}

impl Rule {
    /// The checks for the phase 2 kinds: times, paths, texts, settings.
    fn validate_phase2(&self) -> Result<(), String> {
        for trigger in &self.triggers {
            match trigger {
                Trigger::Daily { at } if clock(at).is_none() => {
                    return Err(format!("`{at}` is not a time of day (HH:MM)"));
                }
                Trigger::FsEvent { paths } => {
                    if paths.is_empty() || paths.len() > 16 {
                        return Err("a file trigger watches 1 to 16 paths".into());
                    }
                    if let Some(bad) = paths.iter().find(|path| !path_like(path)) {
                        return Err(format!("watched path `{bad}` must be absolute or under ~"));
                    }
                }
                _ => {}
            }
        }
        for condition in &self.conditions {
            match condition {
                Condition::TimeBetween { from, to }
                    if clock(from).is_none() || clock(to).is_none() =>
                {
                    return Err(format!("`{from}`-`{to}` is not a span of the day (HH:MM)"));
                }
                Condition::PathExists { path } if !path_like(path) => {
                    return Err(format!("path `{path}` must be absolute or under ~"));
                }
                Condition::Judgment {
                    question,
                    setting,
                    threshold,
                } => {
                    if question.trim().is_empty() || question.len() > 300 {
                        return Err("a judgment's question is 1 to 300 bytes".into());
                    }
                    if !setting.starts_with("background.") || *threshold > 100 {
                        return Err("a judgment names a `background.` setting and a percent".into());
                    }
                }
                Condition::TaskOutcome { outcomes } if outcomes.is_empty() => {
                    return Err("a task outcome condition names at least one outcome".into());
                }
                Condition::Weekdays { days }
                    if days.is_empty() || days.len() > 7 || days.iter().any(|day| *day > 6) =>
                {
                    return Err("a day-of-week condition names 1 to 7 days, 0 (Sunday) to 6".into());
                }
                _ => {}
            }
        }
        for action in &self.actions {
            match action {
                Action::Notify { text } if text.trim().is_empty() || text.len() > 280 => {
                    return Err("a notification is 1 to 280 bytes".into());
                }
                Action::GitFastForward { repo, branch } => {
                    if !path_like(repo) {
                        return Err(format!("checkout `{repo}` must be absolute or under ~"));
                    }
                    if branch
                        .as_ref()
                        .is_some_and(|b| b.trim().is_empty() || b.len() > 200)
                    {
                        return Err("a branch name is 1 to 200 bytes".into());
                    }
                }
                Action::StartCoderRun { prompt, workspace } => {
                    if prompt.trim().is_empty() || prompt.len() > 4000 {
                        return Err("a Coder run's prompt is 1 to 4000 bytes".into());
                    }
                    if let Some(bad) = workspace.as_ref().filter(|w| !path_like(w)) {
                        return Err(format!("workspace `{bad}` must be absolute or under ~"));
                    }
                }
                Action::RunPlugin { plugin, input } => {
                    if plugin.trim().is_empty() || plugin.len() > 200 || input.len() > 4000 {
                        return Err(
                            "a plugin action names a plugin and at most 4000 bytes of input".into(),
                        );
                    }
                }
                Action::ReleaseStaleClaims { idle_hours } if *idle_hours == 0 => {
                    return Err("a claim is stale after at least an hour".into());
                }
                Action::HealthWatch { failures, .. } if *failures == 0 => {
                    return Err("a health watch restarts after at least one failure".into());
                }
                Action::RotateLogs {
                    compress_days,
                    keep_days,
                } if *keep_days == 0 || compress_days > keep_days => {
                    return Err(
                        "logs are kept at least a day, and compressed before they are removed"
                            .into(),
                    );
                }
                _ => {}
            }
        }
        if let Some(Escalate {
            workspace: Some(bad),
        }) = &self.escalate
            && !path_like(bad)
        {
            return Err(format!("workspace `{bad}` must be absolute or under ~"));
        }
        for judged in &self.classes.judged {
            if !path_like(&judged.path) || judged.path == "~/" || judged.path == "/" {
                return Err(format!("`{}` is not a folder a rule can name", judged.path));
            }
            if !crate::judged::KINDS.contains(&judged.kind.as_str()) {
                return Err(format!("`{}` is not a kind of cache", judged.kind));
            }
        }
        if self.cleans() && matches!(self.origin, Origin::Conversation { .. }) {
            // A rule made in conversation deletes only within the host's
            // own roots, like a plugin's.
            let roots = crate::plugins::host_roots();
            let inside = |path: &String| {
                roots
                    .iter()
                    .any(|root| path == root || path.starts_with(&format!("{root}/")))
            };
            if let Some(bad) = self.safety.allow.iter().find(|path| !inside(path)) {
                return Err(format!("`{bad}` is outside the folders the host cleans"));
            }
        }
        Ok(())
    }
}

/// Expand a leading `~/` against `home`.
#[must_use]
pub fn expand(pattern: &str, home: &Path) -> PathBuf {
    match pattern.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if pattern == "~" => home.to_owned(),
        None => PathBuf::from(pattern),
    }
}

/// The paths a pattern names: a trailing `*` in the last component matches
/// any suffix, never across `/`. Symbolic links never match.
#[must_use]
pub fn glob(pattern: &str, home: &Path) -> Vec<PathBuf> {
    let path = expand(pattern, home);
    // A privacy-protected folder is never listed or matched: listing one
    // makes macOS ask the person about this program.
    let protected = |path: &Path| coder_boundary::privacy::is_protected(path, home);
    if protected(&path) {
        return Vec::new();
    }
    let Some(name) = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return Vec::new();
    };
    let Some(prefix) = name.strip_suffix('*') else {
        return if crate::paths::real_dir(&path) {
            vec![path]
        } else {
            Vec::new()
        };
    };
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
                && !protected(path)
                && crate::paths::real_dir(path)
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_on_small_and_large_volumes() {
        let goal = disk().goal;
        // 2 TB: the percentages win.
        let big = 2_000 * GB;
        assert_eq!(goal.start.of(big), 300 * GB);
        assert_eq!(goal.stop.of(big), 400 * GB);
        assert_eq!(goal.emergency.of(big), 20 * GB);
        // 1 TB: the floors win.
        let small = 1_000 * GB;
        assert_eq!(goal.start.of(small), 200 * GB);
        assert_eq!(goal.stop.of(small), 300 * GB);
        assert_eq!(goal.emergency.of(small), 10 * GB);
    }

    #[test]
    fn the_built_in_rule_is_valid_and_digested() {
        let rule = disk();
        rule.validate().unwrap();
        assert!(rule.digest().starts_with("sha256:"));
        let mut edited = rule.clone();
        edited.classes.keep = 2;
        assert_ne!(rule.digest(), edited.digest());
        let mut bad = rule;
        bad.goal.stop.bytes = 1;
        assert!(bad.validate().is_err());
    }
}
