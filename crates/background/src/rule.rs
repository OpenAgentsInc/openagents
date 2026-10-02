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
    pub goal: Goal,
    /// Run in order until the goal is met.
    pub actions: Vec<Action>,
    pub classes: Classes,
    pub safety: Safety,
    pub cooldown_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    BuiltIn,
    File { path: String },
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
    /// 6: the background trash, in an emergency only.
    Trash,
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
        }
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
    /// Slots, Coder One's target, and agent directories untouched this
    /// long are stale.
    pub idle_days: u64,
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
        enabled: true,
        paused_until: None,
        triggers: vec![
            Trigger::Interval { every_secs: 300 },
            Trigger::Threshold,
            Trigger::TaskEnded,
            Trigger::HostStart,
        ],
        goal: Goal {
            start: Level {
                bytes: 30 * GB,
                percent: 5,
            },
            stop: Level {
                bytes: 60 * GB,
                percent: 15,
            },
            emergency: Level {
                bytes: 10 * GB,
                percent: 1,
            },
            max_freed: 100 * GB,
        },
        actions: vec![
            Action::DeleteCaches {
                classes: vec![Class::EndedTargets, Class::StaleTargets],
            },
            Action::PruneWorktrees,
            Action::DeleteCaches {
                classes: vec![Class::GatePools],
            },
            Action::CargoCleanPartial,
            Action::EmptyTrash,
        ],
        classes: Classes {
            agent_targets: vec!["~/work/openagents-target-agent*".into()],
            checkouts: vec!["~/work/*".into()],
            idle_days: 3,
            checkout_days: 7,
            keep: 0,
            orphan_worktree_days: 7,
            gate_idle_hours: 1,
            report_only: Vec::new(),
        },
        safety: Safety {
            allow: vec![
                "~/.openagents/targets".into(),
                "~/.openagents/coder-one/target".into(),
                "~/.openagents/worktrees".into(),
                "~/.openagents/gate".into(),
                "~/.openagents/background/trash".into(),
                "~/work".into(),
            ],
            deny: Vec::new(),
            report: vec![
                "~/.openagents/pylon".into(),
                "~/Library/Caches".into(),
                "/private/var/folders".into(),
            ],
        },
        cooldown_secs: 600,
    }
}

/// The built-in rules, by id.
#[must_use]
pub fn built_in(id: &str) -> Option<Rule> {
    (id == "disk").then(disk)
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

    #[must_use]
    pub fn has(&self, wanted: &Trigger) -> bool {
        self.triggers.iter().any(|trigger| trigger == wanted)
    }

    /// Check a rule before it is saved.
    ///
    /// # Errors
    /// A plain sentence naming the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema must be {SCHEMA}"));
        }
        if built_in(&self.id).is_none() {
            return Err(format!(
                "`{}` is not a rule this host runs; phase 1 runs `disk` only",
                self.id
            ));
        }
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
        if self.interval().is_some_and(|every| every < 60) {
            return Err("checks must be at least a minute apart".into());
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
        // 1.8 TB: the percentages win.
        let big = 1_800 * GB;
        assert_eq!(goal.start.of(big), 90 * GB);
        assert_eq!(goal.stop.of(big), 270 * GB);
        assert_eq!(goal.emergency.of(big), 18 * GB);
        // 256 GB: the floors win.
        let small = 256 * GB;
        assert_eq!(goal.start.of(small), 30 * GB);
        assert_eq!(goal.stop.of(small), 60 * GB);
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
