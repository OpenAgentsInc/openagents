//! The decision settings: every threshold that turns a Jev probability into
//! an action, named, with its default in one place.
//!
//! A setting is how an answer is read, not what was asked. It is never sent
//! and is outside every recorded-answer key ([`crate::component::jev::key`]),
//! so a setting can change, or be fitted on recorded answers, without asking
//! Jev again. Each default is the value the code used before settings
//! existed, so reading through a setting changes no decision.
//!
//! Most settings belong to a question Coder One builds in code, and their
//! default is the whole story: the constant each one names keeps the
//! comment that says how its value was chosen. A question read from a file
//! in `questions/` may carry a `decision` block beside its wording
//! ([`jev::decision`]), and the block then wins over the default: the
//! departure sources' thresholds and the method-conformance Choice's
//! weights are read that way.
//!
//! [`digest`] names the settings in effect that differ from their defaults,
//! and a policy manifest records it as `policy.jev.decision`. It is `None`
//! while every setting is at its default, so no existing manifest's digest
//! moves.

use std::collections::BTreeMap;

use jev::{Decision, Threshold};
use serde_json::json;

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

/// `stall.next`: the lowest probability of the pick for a next-step
/// suggestion to be put in the brief.
pub const STALL_NEXT: Setting = Setting::new("stall.next", crate::stall::NEXT_P);

/// `grade.faithful`: the probability at or above which a line's
/// expectation follows.
pub const GRADE_FAITHFUL: Setting = Setting::new("grade.faithful", crate::grade::THRESHOLD);

/// `issue_turn.depends`: how sure Jev must be that a place depends on a
/// changed count to flag it.
pub const ISSUE_TURN_DEPENDS: Setting =
    Setting::new("issue_turn.depends", crate::issue_turn::DEPENDS_FLAG);

/// `issue_turn.plain`: how sure Jev must be that a newcomer understands a
/// text for it to pass.
pub const ISSUE_TURN_PLAIN: Setting =
    Setting::new("issue_turn.plain", crate::issue_turn::PLAIN_FLAG);

/// `micro.parallel.shared`: the probability at which two units share a
/// file, for planning.
pub const MICRO_SHARED: Setting =
    Setting::new("micro.parallel.shared", crate::micro::parallel::SHARED_MIN);

/// `micro.lean.suspect`: the probability at which a comment is a likely
/// defect.
pub const MICRO_SUSPECT: Setting =
    Setting::new("micro.lean.suspect", crate::micro::lean::SUSPECT_P);

/// `micro.lean.hardcode`: the probability at which Jev's answer flags
/// hard-coding.
pub const MICRO_HARDCODE: Setting =
    Setting::new("micro.lean.hardcode", crate::micro::lean::HARDCODE_P);

/// `micro.part_met`: how sure Jev must be that a part is met for it to
/// count.
pub const MICRO_PART_MET: Setting = Setting::new("micro.part_met", crate::micro::PART_MET);

/// `micro.close`: the probability that the joined evidence shows the task
/// done, below which a green suite gets an audit session.
pub const MICRO_CLOSE: Setting = Setting::new("micro.close", crate::micro::CLOSE_MIN);

/// `micro.close.requirement`: the probability under which the closing
/// audit names a requirement as weakly shown. Written as `p < 0.5` before
/// settings existed; unmeasured.
pub const MICRO_CLOSE_REQUIREMENT: Setting = Setting::new("micro.close.requirement", 0.5);

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

/// `accept.verify.fails_on_current`: the probability that a test fails on
/// a module's current behavior under which Jev is said to doubt it.
/// Written as `p < 0.3` before settings existed; unmeasured.
pub const VERIFY_FAILS_ON_CURRENT: Setting = Setting::new("accept.verify.fails_on_current", 0.3);

/// `checks.truth.report`: the probability at which a question about the
/// final report reads as yes, as a truth signal. Written as `p >= 0.5`
/// before settings existed.
pub const TRUTH_REPORT: Setting = Setting::new("checks.truth.report", 0.5);

/// `checks.truthful.report`: the report audit's score at which the
/// truthful check calls a failure.
pub const TRUTHFUL_REPORT: Setting =
    Setting::new("checks.truthful.report", crate::checks::truthful::REPORT_AT);

/// `checks.verdict.admission`: the corroboration threshold on the report's
/// `admits_unmet` answer.
pub const VERDICT_ADMISSION: Setting = Setting::new(
    "checks.verdict.admission",
    crate::checks::verdict::ADMISSION_AT,
);

/// `checks.contract.extract`: a Noul at or above this says yes while a
/// contract plan is extracted.
pub const CONTRACT_EXTRACT: Setting = Setting::new(
    "checks.contract.extract",
    crate::checks::contract::extract::THRESHOLD,
);

/// `checks.metric_target.goal`: the Noul at which a number is a goal's
/// threshold.
pub const METRIC_TARGET_GOAL: Setting = Setting::new(
    "checks.metric_target.goal",
    crate::checks::metric_target::GOAL_P,
);

/// `checks.oracle.define`: the bound a definition Noul must reach.
pub const ORACLE_DEFINE: Setting = Setting::new(
    "checks.oracle.define",
    crate::checks::oracle::define::THRESHOLD,
);

/// `ask.cite.reason`: the threshold a cited judgment must meet.
pub const ASK_CITE_REASON: Setting = Setting::new("ask.cite.reason", crate::ask::cite::REASON_AT);

/// `ask.gather.yes`: a yes at or above this probability selects an item.
pub const ASK_GATHER_YES: Setting = Setting::new("ask.gather.yes", crate::ask::gather::YES);

/// Every setting on a question built in code, in name order. The departure
/// sources and the method-conformance Choice read their settings from their
/// question-set files, through [`in_effect`].
pub const CODE: &[Setting] = &[
    VERIFY_FAILS_ON_CURRENT,
    ASK_CITE_REASON,
    ASK_GATHER_YES,
    CONTRACT_EXTRACT,
    METRIC_TARGET_GOAL,
    ORACLE_DEFINE,
    TRUTH_REPORT,
    TRUTHFUL_REPORT,
    VERDICT_ADMISSION,
    EVIDENCE_EDIT_TARGET,
    EVIDENCE_YES,
    GRADE_FAITHFUL,
    ISSUE_TURN_DEPENDS,
    ISSUE_TURN_PLAIN,
    JUDGE_READY,
    MICRO_CLOSE,
    MICRO_CLOSE_REQUIREMENT,
    MICRO_HARDCODE,
    MICRO_SUSPECT,
    MICRO_SHARED,
    MICRO_PART_MET,
    PACK_SELECTED,
    STALL_NEXT,
    SYSTEM_SELECT,
    TERMINAL_ASKS_ONLY,
];

/// One setting read from a question-set file: its name, the block the file
/// writes, and the default the block overrides.
struct FromFile {
    name: String,
    decision: Decision,
    default: Decision,
}

/// The settings read from question-set files.
fn from_files() -> Vec<FromFile> {
    let mut read: Vec<FromFile> = crate::departures::Source::ALL
        .iter()
        .map(|source| FromFile {
            name: format!("departures.{}", source.word()),
            decision: crate::departures::question_set(*source).decision.clone(),
            default: Decision {
                threshold: Some(source.default_threshold()),
                ..Decision::default()
            },
        })
        .collect();
    read.push(FromFile {
        name: "checks.conformance.method".to_string(),
        decision: crate::checks::conformance::question_set().decision.clone(),
        default: Decision::default(),
    });
    read
}

/// Every setting in effect, by name: each code setting at its default, and
/// each file-read setting as its file writes it over its default.
#[must_use]
pub fn in_effect() -> BTreeMap<String, Decision> {
    let mut settings: BTreeMap<String, Decision> = CODE
        .iter()
        .map(|setting| {
            (
                setting.name.to_string(),
                Decision {
                    threshold: Some(setting.threshold()),
                    ..Decision::default()
                },
            )
        })
        .collect();
    for file in from_files() {
        let mut decision = file.default.clone();
        if file.decision.threshold.is_some() {
            decision.threshold = file.decision.threshold;
        }
        if file.decision.cuts.is_some() {
            decision.cuts.clone_from(&file.decision.cuts);
        }
        if file.decision.weights.is_some() {
            decision.weights.clone_from(&file.decision.weights);
        }
        settings.insert(file.name, decision);
    }
    settings
}

/// The settings in effect that differ from their defaults, by name.
#[must_use]
pub fn changed() -> BTreeMap<String, Decision> {
    let defaults: BTreeMap<String, Decision> = from_files()
        .into_iter()
        .map(|file| (file.name, file.default))
        .collect();
    in_effect()
        .into_iter()
        .filter(|(name, decision)| {
            defaults
                .get(name)
                .is_some_and(|default| default != decision)
        })
        .collect()
}

/// The digest of the settings that differ from their defaults, which a
/// policy manifest records as `policy.jev.decision`. `None` while every
/// setting is at its default.
#[must_use]
pub fn digest() -> Option<String> {
    let changed = changed();
    (!changed.is_empty()).then(|| atif::digest(&json!(changed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_the_value_the_code_used_before_settings_existed() {
        let expected = [
            ("accept.verify.fails_on_current", 0.3),
            ("ask.cite.reason", 0.5),
            ("ask.gather.yes", 0.5),
            ("checks.contract.extract", 0.5),
            ("checks.metric_target.goal", 0.5),
            ("checks.oracle.define", 0.5),
            ("checks.truth.report", 0.5),
            ("checks.truthful.report", 0.5),
            ("checks.verdict.admission", 0.8),
            ("evidence.edit_target", 0.8),
            ("evidence.yes", 0.5),
            ("grade.faithful", 0.5),
            ("issue_turn.depends", 0.5),
            ("issue_turn.plain", 0.2),
            ("judge.ready", 0.8),
            ("micro.close", 0.7),
            ("micro.close.requirement", 0.5),
            ("micro.lean.hardcode", 0.6),
            ("micro.lean.suspect", 0.5),
            ("micro.parallel.shared", 0.4),
            ("micro.part_met", 0.75),
            ("pack.selected", 0.5),
            ("stall.next", 0.5),
            ("system.select", 0.5),
            ("terminal.asks_only", 0.6),
        ];
        let named: Vec<(&str, f64)> = CODE
            .iter()
            .map(|setting| (setting.name, setting.default.value()))
            .collect();
        assert_eq!(named, expected);
    }

    #[test]
    fn a_setting_reads_yes_at_or_above_its_default_as_the_code_did() {
        for setting in CODE {
            let at = setting.default.value();
            assert!(setting.yes(at), "{}", setting.name);
            assert!(setting.yes(at + 0.01), "{}", setting.name);
            assert!(!setting.yes(at - 0.01), "{}", setting.name);
        }
        // The fixtures the code's comments measure against.
        assert!(!MICRO_PART_MET.yes(0.64));
        assert!(ISSUE_TURN_DEPENDS.yes(0.55));
        assert!(!ISSUE_TURN_PLAIN.yes(0.06));
        assert!(ISSUE_TURN_PLAIN.yes(0.38));
    }

    #[test]
    fn every_setting_is_at_its_default_so_manifests_record_none() {
        assert!(changed().is_empty(), "{:?}", changed());
        assert_eq!(digest(), None);
        let in_effect = in_effect();
        assert_eq!(in_effect.len(), CODE.len() + 4);
        for source in crate::departures::Source::ALL {
            assert_eq!(
                in_effect[&format!("departures.{}", source.word())].threshold,
                Some(Threshold::at(0.5))
            );
        }
        assert!(in_effect["checks.conformance.method"].is_empty());
    }
}
