//! Decision settings: thresholds that turn a Jev probability into an
//! action, named, with their defaults. This crate holds [`Setting`] and
//! the settings its own components read; Coder One's `decision` module
//! re-exports them beside the rest and computes the digest a policy
//! manifest records.

use jev::Threshold;

/// One named threshold on a Noul probability, or on the probability of a
/// Choice's pick: a probability at or above it reads as yes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setting {
    /// The setting's name, as a record and a fitted-settings study name it.
    pub name: &'static str,
    /// The value the code used before settings existed.
    pub default: Threshold,
}

impl Setting {
    /// A setting and its default.
    #[must_use]
    pub const fn new(name: &'static str, default: f64) -> Self {
        Self {
            name,
            default: Threshold::at(default),
        }
    }

    /// The threshold in effect. A question built in code has no file to
    /// carry a block, so this is the default.
    #[must_use]
    pub fn threshold(self) -> Threshold {
        self.default
    }

    /// Whether a probability reads as yes under this setting.
    #[must_use]
    pub fn yes(self, p: f64) -> bool {
        self.threshold().yes(p)
    }
}

/// `system.select`: a Noul at or above this selects an optional section.
pub const SYSTEM_SELECT: Setting = Setting::new("system.select", crate::system::SELECT);

/// `evidence.yes`: a Noul at or above this reads as yes in the evidence
/// components.
pub const EVIDENCE_YES: Setting = Setting::new("evidence.yes", crate::component::evidence::YES);

/// `evidence.edit_target`: an edit probability at or above this marks a
/// likely edit target. The delegate's briefing and the replay read the
/// same survey answer against it; both wrote 0.8 inline before settings
/// existed.
pub const EVIDENCE_EDIT_TARGET: Setting = Setting::new(
    "evidence.edit_target",
    crate::component::evidence::EDIT_TARGET,
);

/// `pack.selected`: the relevance at or above which a briefing item counts
/// as selected when a packed briefing is measured. Written as `p >= 0.5`
/// before settings existed; unmeasured.
pub const PACK_SELECTED: Setting = Setting::new("pack.selected", 0.5);

/// `judge.ready`: the probability that the task is done and checked at
/// which the explorer is told to finish. Written as `>= 0.8` before
/// settings existed; unmeasured.
pub const JUDGE_READY: Setting = Setting::new("judge.ready", 0.8);

/// `terminal.asks_only`: the probability at which a terminal request asks
/// only for an answer, not a change. Written as `>= 0.6` before settings
/// existed; unmeasured.
pub const TERMINAL_ASKS_ONLY: Setting = Setting::new("terminal.asks_only", 0.6);

/// `issue_turn.depends`: how sure Jev must be that a place depends on a
/// changed count to flag it.
pub const ISSUE_TURN_DEPENDS: Setting =
    Setting::new("issue_turn.depends", crate::issue::DEPENDS_FLAG);

/// `issue_turn.plain`: how sure Jev must be that a newcomer understands a
/// text for it to pass.
pub const ISSUE_TURN_PLAIN: Setting = Setting::new("issue_turn.plain", crate::issue::PLAIN_FLAG);
