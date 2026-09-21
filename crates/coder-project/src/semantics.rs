//! The semantic-independence harness: a measured answer to *the model
//! said independent*.
//!
//! A fan-out program asks a model whether the listed tasks can run at
//! once, and the deterministic scheduler already answers the same
//! question from declared footprints and `after` edges — the rules it
//! enforces, never an opinion it holds. When a proposal claims a pair
//! the scheduler separates is really independent, someone is right and
//! someone is wrong, and *the model said so* is not a measurement. This
//! module is the deterministic half of that measurement: supplied
//! fixtures and supplied proposals in, a versioned report out. No model
//! is called here — a proposal arrives already answered and is scored
//! like any other evidence, and a label arrives already decided, from a
//! source that owes the model nothing.
//!
//! The report counts direction, not just correctness. A proposal that
//! frees a pair the label says conflicts is the dangerous direction —
//! [`Finding::UnsafeIndependent`] — counted apart from every other
//! outcome, because one accuracy number would let a cautious model and
//! a reckless one score identically. A proposal that separates a pair
//! the label says independent is the safe direction: headroom left on
//! the floor, never a hazard. An ambiguous label scores neither side —
//! where the fixture cannot say who was right, the report says so too.
//!
//! # A case
//!
//! A [`Case`] pins one fixture: a shape, a task set, and the ground
//! truth. Each [`FixtureTask`] declares what the scheduler reads — an
//! id, a [`Footprint`], and the tasks it must run `after` — and each
//! [`LabeledPair`] supplies what a scorer cannot derive: really
//! conflicts, really independent, or ambiguous. A case that cannot pin
//! every pair refuses to score, because a partial fixture is an
//! argument wearing a lab coat.
//!
//! # The verdict
//!
//! Every pair gets a [`Verdict`]: the deterministic rule's answer, the
//! supplied label, the supplied proposal when one exists, and the
//! [`Finding`] the three make together. The rule's answer is computed
//! by the same check the fan-out enforces — `footprints_conflict` over
//! the declared footprints, with a declared `after` edge reached
//! transitively ordering the pair first: an ordered pair never
//! overlaps, so its footprint collision, if any, is already enforced by
//! the ordering.
//!
//! # The report
//!
//! [`Report::score`] returns a versioned table: totals, the same counts
//! broken down per fixture shape, and every verdict it counted. The
//! findings answer *what the model adds* — recovered headroom where
//! model and label agree the rule was over-conservative, unsafe
//! independence alone in the dangerous direction, ambiguous labels
//! counted as decidable neither way, unanimity where all three agree,
//! and the rule's own misses named, caught and uncaught, so a
//! deterministic over-optimism never hides inside the model's score.
//!
//! # The gate
//!
//! [`Gate`] is the host's admission rule over a report, stated in the
//! report it judges. A proposal may inform resource classes or ordering
//! suggestions only while the measured unsafe rate — unsafe claims over
//! decidable independence claims — sits at or under the supplied bound.
//! Over the bound, or with no decidable claims at all, the standing is
//! [`Standing::AdvisoryOnly`]: an advisory proposal can annotate a
//! plan, and it can never relax a constraint the deterministic rule
//! enforces. A wrong answer becomes an annotation, not a concurrency
//! bug.
//!
//! # Determinism
//!
//! The module reads no filesystem, no clock, and no network. The same
//! cases and proposals produce the same report every time; pair order
//! follows each case's task order, and every count derives from the
//! pinned inputs alone.

use std::collections::{BTreeMap, BTreeSet};

use coder_scheduler::catalog::{Footprint, footprints_conflict};
use serde::{Deserialize, Serialize};

/// The schema tag a semantics report carries.
pub const SCHEMA: &str = "openagents.project-semantics.v1";

/// What kind of fixture one case builds.
///
/// The shape is the fixture's own claim about what it was built to
/// probe, and the report breaks every count down by it — a model that
/// is safe on disjoint work but reckless on mixed read/write sets reads
/// as exactly that, never averaged into one number.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shape {
    /// Built so the declared footprints collide on purpose.
    DeliberatelyColliding,
    /// Built so nothing the tasks touch overlaps.
    Disjoint,
    /// Built so the honest ground truth is ambiguous.
    Ambiguous,
    /// Readers against writers and shared reads mixed together.
    MixedReadWrite,
}

/// The independently supplied ground truth for one pair.
///
/// A label is an input, never a computation of this module and never a
/// model's answer: it says what the pair really does, which is what a
/// verdict scores the rule and the proposal against. `Ambiguous` is a
/// real label, not a missing one — it says the honest answer is that
/// nobody can say.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Label {
    /// The pair really collides — running it at once is a bug.
    ReallyConflicts,
    /// The pair really is independent — separating it costs parallelism.
    ReallyIndependent,
    /// The honest answer is that nobody can say; scores neither side.
    Ambiguous,
}

/// One task in a fixture set: an id, a declared footprint, and the
/// declared after-dependencies the scheduler orders it under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureTask {
    /// The stable id pairs and labels name.
    pub id: String,
    /// What the task reads and writes, declared the way the scheduler
    /// reads it. [`Footprint::Unknown`] declares a task that cannot
    /// enumerate its paths, and it collides with everything.
    pub footprint: Footprint,
    /// Ids this task must run after — the `depends_on` half of the
    /// declared rules, read transitively when a pair is scored. An id
    /// the set does not carry dangles, exactly as a catalog dependency
    /// on an absent task is no defect.
    #[serde(default)]
    pub after: Vec<String>,
}

/// One pair's supplied ground truth.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledPair {
    /// One task id; either endpoint order names the same pair.
    pub a: String,
    /// The other task id.
    pub b: String,
    /// What the pair really does.
    pub label: Label,
}

/// One labeled fixture: a task set and the ground truth for every pair.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// The fixture's id — proposals name it to score against it.
    pub id: String,
    /// What the fixture was built to probe.
    pub shape: Shape,
    /// The task set, in fixture order.
    pub tasks: Vec<FixtureTask>,
    /// The ground truth: one label per unordered pair, no more and no
    /// fewer. A pair without a label is an incomplete fixture, and the
    /// case refuses to score.
    pub labels: Vec<LabeledPair>,
}

impl Case {
    /// The structural checks: an id, at least two tasks, unique task
    /// ids, a label for every pair the set makes and only for tasks the
    /// set carries, and no cyclic `after` edge.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the fixture is not one a report
    /// reads: no id, too few tasks, a duplicated or unlabeled pair, a
    /// label naming a task the set does not carry, a pair that names
    /// the same task twice, or an `after` edge that loops.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("a case needs an id".into());
        }
        if self.tasks.len() < 2 {
            return Err(format!(
                "case `{}` holds fewer than two tasks — no pair to score",
                self.id
            ));
        }
        let mut ids = BTreeSet::new();
        for task in &self.tasks {
            if task.id.is_empty() {
                return Err(format!("case `{}` holds a task with no id", self.id));
            }
            if !ids.insert(task.id.as_str()) {
                return Err(format!("case `{}` names task `{}` twice", self.id, task.id));
            }
            if task.after.iter().any(|after| after.is_empty()) {
                return Err(format!(
                    "case `{}` task `{}` waits on an empty id",
                    self.id, task.id
                ));
            }
        }
        let mut labeled = BTreeSet::new();
        for pair in &self.labels {
            if pair.a == pair.b {
                return Err(format!(
                    "case `{}` labels task `{}` against itself",
                    self.id, pair.a
                ));
            }
            let key = pair_key(&pair.a, &pair.b);
            if !ids.contains(key.0.as_str()) || !ids.contains(key.1.as_str()) {
                return Err(format!(
                    "case `{}` labels pair {}, which names a task the set does not carry",
                    self.id,
                    pair_name(&key)
                ));
            }
            if !labeled.insert(key.clone()) {
                return Err(format!(
                    "case `{}` labels pair {} twice",
                    self.id,
                    pair_name(&key)
                ));
            }
        }
        for key in self.pairs() {
            if !labeled.contains(&key) {
                return Err(format!(
                    "case `{}` carries no label for pair {} — an incomplete fixture, not a guessed one",
                    self.id,
                    pair_name(&key)
                ));
            }
        }
        for (id, afters) in self.after_sets() {
            if afters.contains(id) {
                return Err(format!(
                    "case `{}` declares a cyclic after-dependency through `{id}`",
                    self.id
                ));
            }
        }
        Ok(())
    }

    /// Every unordered pair, canonicalized, in task order.
    ///
    /// The canonical key sorts the endpoints, so a label or a proposal
    /// names the pair in either order and lands on the same verdict.
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for (index, first) in self.tasks.iter().enumerate() {
            for second in &self.tasks[index + 1..] {
                pairs.push(pair_key(&first.id, &second.id));
            }
        }
        pairs
    }

    /// The task under one id.
    fn task(&self, id: &str) -> Option<&FixtureTask> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// The transitive after-closure: for each task, the in-set tasks it
    /// must run after. A task that reaches its own id is the cycle
    /// [`Case::validate`] refuses.
    fn after_sets(&self) -> BTreeMap<&str, BTreeSet<&str>> {
        let mut sets = BTreeMap::new();
        for task in &self.tasks {
            let mut reached = BTreeSet::new();
            let mut stack: Vec<&str> = task.after.iter().map(String::as_str).collect();
            while let Some(id) = stack.pop() {
                if !reached.insert(id) {
                    continue;
                }
                if let Some(next) = self.task(id) {
                    stack.extend(next.after.iter().map(String::as_str));
                }
            }
            sets.insert(task.id.as_str(), reached);
        }
        sets
    }
}

/// The deterministic scheduler's answer for one pair — the same answer
/// the declared rules give the fan-out.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ruling {
    /// The declared footprints collide: a write reaches a path the
    /// other side reads or writes.
    Conflict,
    /// A declared `after` edge, direct or transitive, already orders
    /// the pair — they can never overlap, however their footprints read.
    Ordered,
    /// Nothing declared separates the pair; the scheduler may admit
    /// them at once, capacity allowing.
    Free,
}

/// What a supplied proposal claims about one pair.
///
/// A proposal is evidence under evaluation, not authority: it is an
/// input the report scores, exactly the way a label is an input the
/// report trusts. The harness never produces one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Claim {
    /// The proposal says the pair can run at once.
    Independent,
    /// The proposal says the pair must be kept apart.
    Conflicts,
    /// The proposal declines to say — scored as no claim.
    Uncertain,
}

/// One supplied model proposal, pinned to a case and a pair.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// The case the proposal answers.
    pub case: String,
    /// One task id; either endpoint order names the same pair.
    pub a: String,
    /// The other task id.
    pub b: String,
    /// What the proposal claims.
    pub claim: Claim,
}

/// What the three answers make together — the per-pair outcome the
/// report counts.
///
/// Every finding keeps its direction: the unsafe direction never
/// shares a bucket with caution, and the rule's own errors are named
/// rather than folded into the model's score.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Finding {
    /// Rule, label, and proposal all answer the same way.
    Unanimous,
    /// Label and proposal say independent while the rule held the pair
    /// apart — headroom the model recovers.
    RecoveredHeadroom,
    /// The label says independent while the rule held the pair apart
    /// and no proposal freed it — headroom that exists and the model
    /// did not recover.
    UnrecoveredHeadroom,
    /// The proposal says independent and the label says the pair
    /// conflicts — the dangerous direction, counted apart and alone.
    UnsafeIndependent,
    /// Rule and label would run the pair at once and the proposal
    /// objects — caution that costs parallelism, never safety.
    OvercautiousProposal,
    /// The label says the pair conflicts, the rule would have freed it,
    /// and the proposal objected — safety the declared rules missed and
    /// the model added.
    CaughtRuleMiss,
    /// The label says the pair conflicts and nothing objected — a
    /// deterministic over-optimism the proposal did not catch.
    UncaughtRuleMiss,
    /// The label is ambiguous: the pair scores neither direction.
    AmbiguousLabel,
    /// Rule and label agree and the proposal offered nothing — no
    /// claim to score, not a claim scored safe.
    NoProposal,
}

/// The three-way comparison for one pair: what the deterministic rule
/// said, what the supplied label says, and what a supplied proposal
/// said when one exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Verdict {
    /// The case the pair belongs to.
    pub case: String,
    /// The fixture's shape.
    pub shape: Shape,
    /// The pair's canonical first task id.
    pub a: String,
    /// The pair's canonical second task id.
    pub b: String,
    /// The deterministic rule's answer.
    pub rule: Ruling,
    /// The supplied ground truth.
    pub label: Label,
    /// The supplied proposal's claim, when one was supplied.
    pub proposal: Option<Claim>,
    /// What the three make together.
    pub finding: Finding,
}

/// The counts one report or one shape keeps. Every scored pair lands
/// in exactly one finding, and the unsafe direction stays its own
/// count — there is no accuracy field for it to hide inside.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Tally {
    /// The pairs scored.
    pub pairs: usize,
    /// [`Finding::Unanimous`].
    pub unanimous: usize,
    /// [`Finding::RecoveredHeadroom`].
    pub recovered_headroom: usize,
    /// [`Finding::UnrecoveredHeadroom`].
    pub unrecovered_headroom: usize,
    /// [`Finding::UnsafeIndependent`] — the dangerous direction, never
    /// merged into another count.
    pub unsafe_independent: usize,
    /// [`Finding::OvercautiousProposal`].
    pub overcautious_proposal: usize,
    /// [`Finding::CaughtRuleMiss`].
    pub caught_rule_miss: usize,
    /// [`Finding::UncaughtRuleMiss`].
    pub uncaught_rule_miss: usize,
    /// [`Finding::AmbiguousLabel`].
    pub ambiguous_label: usize,
    /// [`Finding::NoProposal`].
    pub no_proposal: usize,
    /// Proposals that claimed a pair independent, whatever the label.
    pub independent_claims: usize,
    /// The claims a label can judge: `independent_claims` minus the
    /// ones made on ambiguous labels. The unsafe rate's denominator.
    pub decidable_claims: usize,
}

impl Tally {
    /// The unsafe-conflict rate: unsafe claims over decidable
    /// independence claims, or `None` when no independence claim can be
    /// checked. An unmeasured rate is not a safe one.
    #[must_use]
    pub fn unsafe_rate(&self) -> Option<f64> {
        if self.decidable_claims == 0 {
            return None;
        }
        Some(self.unsafe_independent as f64 / self.decidable_claims as f64)
    }

    /// Count one scored pair.
    fn add(&mut self, label: Label, proposal: Option<Claim>, finding: Finding) {
        self.pairs += 1;
        match finding {
            Finding::Unanimous => self.unanimous += 1,
            Finding::RecoveredHeadroom => self.recovered_headroom += 1,
            Finding::UnrecoveredHeadroom => self.unrecovered_headroom += 1,
            Finding::UnsafeIndependent => self.unsafe_independent += 1,
            Finding::OvercautiousProposal => self.overcautious_proposal += 1,
            Finding::CaughtRuleMiss => self.caught_rule_miss += 1,
            Finding::UncaughtRuleMiss => self.uncaught_rule_miss += 1,
            Finding::AmbiguousLabel => self.ambiguous_label += 1,
            Finding::NoProposal => self.no_proposal += 1,
        }
        if proposal == Some(Claim::Independent) {
            self.independent_claims += 1;
            if label != Label::Ambiguous {
                self.decidable_claims += 1;
            }
        }
    }
}

/// The host's admission rule over a report.
///
/// The bound is the most unsafe-conflict rate the host tolerates from
/// a proposal source. It is the operator's number, not the model's own
/// confidence — a model never sets the bar it is measured against.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    /// The highest tolerated rate of unsafe independence claims over
    /// decidable independence claims, from 0.0 to 1.0.
    pub unsafe_bound: f64,
}

impl Gate {
    /// A gate over one bound.
    ///
    /// # Errors
    ///
    /// A bound outside `0.0..=1.0` is not a rate anything can be held to.
    pub fn new(unsafe_bound: f64) -> Result<Self, String> {
        if !(0.0..=1.0).contains(&unsafe_bound) {
            return Err("a gate's unsafe bound must be a rate from 0.0 to 1.0".into());
        }
        Ok(Self { unsafe_bound })
    }

    /// Judge a report and record the verdict inside it.
    ///
    /// At or under the bound the standing is [`Standing::MayInform`].
    /// Over it — or with no decidable claims to measure, because an
    /// unmeasured rate is not under any bound — the standing is
    /// [`Standing::AdvisoryOnly`], and the report keeps the bound, the
    /// counts, and the standing it was judged under.
    pub fn judge(&self, report: &mut Report) -> Standing {
        let unsafe_rate = report.totals.unsafe_rate();
        let standing = match unsafe_rate {
            Some(rate) if rate <= self.unsafe_bound => Standing::MayInform,
            _ => Standing::AdvisoryOnly,
        };
        report.gate = Some(GateVerdict {
            unsafe_bound: self.unsafe_bound,
            independent_claims: report.totals.independent_claims,
            decidable_claims: report.totals.decidable_claims,
            unsafe_independent: report.totals.unsafe_independent,
            unsafe_rate,
            standing,
        });
        standing
    }
}

/// What a proposal source may do, measured over a report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// The measured unsafe rate sits at or under the bound — proposals
    /// may inform resource classes or ordering suggestions.
    MayInform,
    /// Over the bound, or unmeasured — proposals may annotate, and can
    /// never relax a constraint the deterministic rule enforces.
    AdvisoryOnly,
}

/// The gate's verdict, stated inside the report it judged.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct GateVerdict {
    /// The bound the host supplied.
    pub unsafe_bound: f64,
    /// Proposals claiming independence.
    pub independent_claims: usize,
    /// Independence claims a label could judge.
    pub decidable_claims: usize,
    /// Claims that were unsafe.
    pub unsafe_independent: usize,
    /// The measured rate — `None` when nothing decidable was claimed.
    pub unsafe_rate: Option<f64>,
    /// What the source may do.
    pub standing: Standing,
}

/// The versioned report: totals, per-shape counts, every verdict, and
/// the gate's verdict when one has been judged.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Report {
    /// The schema tag.
    pub schema: String,
    /// How many cases were scored.
    pub cases: usize,
    /// Every pair's counts.
    pub totals: Tally,
    /// The same counts broken down per fixture shape.
    pub shapes: BTreeMap<Shape, Tally>,
    /// Every verdict the counts were drawn from, in case order, then
    /// pair order.
    pub verdicts: Vec<Verdict>,
    /// The gate's stated verdict, when a gate has judged this report.
    pub gate: Option<GateVerdict>,
}

impl Report {
    /// Score supplied proposals against supplied labels over supplied
    /// fixtures.
    ///
    /// Pure: fixtures and proposals in, report out. Every case must
    /// validate — every pair labeled exactly once, endpoints the set
    /// carries, no cyclic `after` edge — and every proposal must name a
    /// pair a carried case makes, once: evidence the fixtures cannot
    /// place is refused, not dropped. A pair no proposal answers is
    /// scored with `proposal: None`, because missing evidence is named,
    /// not assumed.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming the fixture or proposal that is not
    /// evidence this report reads.
    pub fn score(cases: &[Case], proposals: &[Proposal]) -> Result<Self, String> {
        let mut case_ids = BTreeSet::new();
        for case in cases {
            case.validate()?;
            if !case_ids.insert(case.id.as_str()) {
                return Err(format!("two cases share id `{}`", case.id));
            }
        }
        let mut claims: BTreeMap<(String, (String, String)), Claim> = BTreeMap::new();
        for proposal in proposals {
            let case = cases
                .iter()
                .find(|case| case.id == proposal.case)
                .ok_or_else(|| {
                    format!(
                        "a proposal answers case `{}`, which the fixtures do not carry",
                        proposal.case
                    )
                })?;
            if proposal.a == proposal.b {
                return Err(format!(
                    "a proposal pairs task `{}` with itself",
                    proposal.a
                ));
            }
            let key = pair_key(&proposal.a, &proposal.b);
            if case.task(&key.0).is_none() || case.task(&key.1).is_none() {
                return Err(format!(
                    "a proposal names pair {} of case `{}`, which names a task the set does not carry",
                    pair_name(&key),
                    case.id
                ));
            }
            if claims
                .insert((case.id.clone(), key.clone()), proposal.claim)
                .is_some()
            {
                return Err(format!(
                    "a second proposal answers pair {} of case `{}`",
                    pair_name(&key),
                    case.id
                ));
            }
        }
        let mut report = Self {
            schema: SCHEMA.to_string(),
            cases: cases.len(),
            totals: Tally::default(),
            shapes: BTreeMap::new(),
            verdicts: Vec::new(),
            gate: None,
        };
        for case in cases {
            let afters = case.after_sets();
            let labels: BTreeMap<(String, String), Label> = case
                .labels
                .iter()
                .map(|pair| (pair_key(&pair.a, &pair.b), pair.label))
                .collect();
            for (a, b) in case.pairs() {
                let rule = rule(
                    case.task(&a).expect("a scored pair names a task"),
                    case.task(&b).expect("a scored pair names a task"),
                    &afters,
                );
                let label = labels
                    .get(&(a.clone(), b.clone()))
                    .copied()
                    .expect("validation pinned every pair");
                let proposal = claims
                    .get(&(case.id.clone(), (a.clone(), b.clone())))
                    .copied();
                let finding = finding(rule, label, proposal);
                report.totals.add(label, proposal, finding);
                report
                    .shapes
                    .entry(case.shape)
                    .or_default()
                    .add(label, proposal, finding);
                report.verdicts.push(Verdict {
                    case: case.id.clone(),
                    shape: case.shape,
                    a,
                    b,
                    rule,
                    label,
                    proposal,
                    finding,
                });
            }
        }
        Ok(report)
    }
}

/// The deterministic rule's answer for one pair. An `after` edge
/// between them — direct or transitive — already orders them, so the
/// ordering is the verdict even when their footprints also collide.
/// Otherwise a declared footprint collision separates them; otherwise
/// they are free.
fn rule(a: &FixtureTask, b: &FixtureTask, afters: &BTreeMap<&str, BTreeSet<&str>>) -> Ruling {
    let ordered = afters
        .get(a.id.as_str())
        .is_some_and(|set| set.contains(b.id.as_str()))
        || afters
            .get(b.id.as_str())
            .is_some_and(|set| set.contains(a.id.as_str()));
    if ordered {
        Ruling::Ordered
    } else if footprints_conflict(&a.footprint, &b.footprint).is_some() {
        Ruling::Conflict
    } else {
        Ruling::Free
    }
}

/// The per-pair outcome the report counts: the grid of rule, label,
/// and proposal collapsed into named directions, each exactly once.
/// The unsafe claim outranks every other reading of the pair — a
/// proposal that frees a real conflict is dangerous even where the
/// rule happened to keep the pair apart anyway.
fn finding(rule: Ruling, label: Label, proposal: Option<Claim>) -> Finding {
    match (rule == Ruling::Free, label, proposal) {
        (_, Label::Ambiguous, _) => Finding::AmbiguousLabel,
        (_, Label::ReallyConflicts, Some(Claim::Independent)) => Finding::UnsafeIndependent,
        (false, Label::ReallyConflicts, Some(Claim::Conflicts))
        | (true, Label::ReallyIndependent, Some(Claim::Independent)) => Finding::Unanimous,
        (false, Label::ReallyIndependent, Some(Claim::Independent)) => Finding::RecoveredHeadroom,
        (false, Label::ReallyIndependent, _) => Finding::UnrecoveredHeadroom,
        (true, Label::ReallyConflicts, Some(Claim::Conflicts)) => Finding::CaughtRuleMiss,
        (true, Label::ReallyConflicts, _) => Finding::UncaughtRuleMiss,
        (true, Label::ReallyIndependent, Some(Claim::Conflicts)) => Finding::OvercautiousProposal,
        _ => Finding::NoProposal,
    }
}

/// The canonical pair key: endpoints sorted, so either order names the
/// same pair.
fn pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// How a pair reads in a refusal: `a`/`b`.
fn pair_name(key: &(String, String)) -> String {
    format!("`{}`/`{}`", key.0, key.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, reads: &[&str], writes: &[&str], after: &[&str]) -> FixtureTask {
        FixtureTask {
            id: id.into(),
            footprint: Footprint::Declared {
                reads: reads.iter().map(|path| path.to_string()).collect(),
                writes: writes.iter().map(|path| path.to_string()).collect(),
            },
            after: after.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn pair(a: &str, b: &str, label: Label) -> LabeledPair {
        LabeledPair {
            a: a.into(),
            b: b.into(),
            label,
        }
    }

    fn proposal(case: &str, a: &str, b: &str, claim: Claim) -> Proposal {
        Proposal {
            case: case.into(),
            a: a.into(),
            b: b.into(),
            claim,
        }
    }

    /// Both writers touch the same file, and a third task reads it —
    /// a fixture built so the declared footprints collide on purpose.
    fn colliding() -> Case {
        Case {
            id: "colliding".into(),
            shape: Shape::DeliberatelyColliding,
            tasks: vec![
                task("a", &[], &["src/shared.rs"], &[]),
                task("b", &[], &["src/shared.rs"], &[]),
                task("c", &["src/shared.rs"], &[], &[]),
            ],
            labels: vec![
                pair("a", "b", Label::ReallyConflicts),
                pair("a", "c", Label::ReallyConflicts),
                pair("b", "c", Label::ReallyConflicts),
            ],
        }
    }

    /// Three readers of different files, one of them serialized behind
    /// another by a declared after-edge it does not need — a disjoint
    /// set with headroom a proposal can recover.
    fn disjoint() -> Case {
        Case {
            id: "disjoint".into(),
            shape: Shape::Disjoint,
            tasks: vec![
                task("a", &["src/a.rs"], &[], &[]),
                task("b", &["src/b.rs"], &[], &["a"]),
                task("c", &["src/c.rs"], &[], &[]),
            ],
            labels: vec![
                pair("a", "b", Label::ReallyIndependent),
                pair("a", "c", Label::ReallyIndependent),
                pair("b", "c", Label::ReallyIndependent),
            ],
        }
    }

    /// Readers against writers: declared footprints that disagree with
    /// the labels in both directions — misses only the label sees, and
    /// collisions only the declared paths see.
    fn mixed() -> Case {
        Case {
            id: "mixed".into(),
            shape: Shape::MixedReadWrite,
            tasks: vec![
                task("reader", &["src/missing.rs"], &[], &[]),
                task("silent", &[], &["src/silent.rs"], &[]),
                task("tree", &[], &["src"], &[]),
                task("writer", &[], &["src/other.rs"], &[]),
            ],
            labels: vec![
                pair("reader", "silent", Label::ReallyConflicts),
                pair("reader", "tree", Label::ReallyIndependent),
                pair("reader", "writer", Label::ReallyConflicts),
                pair("silent", "tree", Label::ReallyConflicts),
                pair("silent", "writer", Label::ReallyIndependent),
                pair("tree", "writer", Label::ReallyConflicts),
            ],
        }
    }

    fn finding_of(report: &Report, a: &str, b: &str) -> Finding {
        report
            .verdicts
            .iter()
            .find(|verdict| verdict.a == a && verdict.b == b)
            .unwrap_or_else(|| panic!("no verdict for {a}/{b}"))
            .finding
    }

    #[test]
    fn a_colliding_set_scores_unsafe_claims_apart() {
        let case = colliding();
        let proposals = [
            proposal("colliding", "a", "b", Claim::Independent),
            proposal("colliding", "a", "c", Claim::Conflicts),
        ];
        let report = Report::score(&[case], &proposals).unwrap();
        assert_eq!(report.totals.pairs, 3);
        assert_eq!(finding_of(&report, "a", "b"), Finding::UnsafeIndependent);
        assert_eq!(finding_of(&report, "a", "c"), Finding::Unanimous);
        assert_eq!(finding_of(&report, "b", "c"), Finding::NoProposal);
        assert_eq!(report.totals.unsafe_independent, 1);
        assert_eq!(report.totals.unanimous, 1);
        assert_eq!(report.totals.no_proposal, 1);
        assert_eq!(
            report.shapes[&Shape::DeliberatelyColliding].unsafe_independent,
            1,
            "the unsafe direction stays its own count per shape"
        );
        assert_eq!(report.totals.decidable_claims, 1);
        assert_eq!(report.totals.unsafe_rate(), Some(1.0));
    }

    #[test]
    fn a_disjoint_set_shows_the_headroom_a_model_recovers() {
        let case = disjoint();
        let proposals = [
            proposal("disjoint", "a", "b", Claim::Independent),
            proposal("disjoint", "a", "c", Claim::Independent),
            proposal("disjoint", "b", "c", Claim::Independent),
        ];
        let report = Report::score(&[case], &proposals).unwrap();
        assert_eq!(
            finding_of(&report, "a", "b"),
            Finding::RecoveredHeadroom,
            "the declared after-edge was over-conservative; the proposal recovered it"
        );
        assert_eq!(finding_of(&report, "a", "c"), Finding::Unanimous);
        assert_eq!(finding_of(&report, "b", "c"), Finding::Unanimous);
        assert_eq!(report.totals.recovered_headroom, 1);
        assert_eq!(report.totals.unanimous, 2);
        assert_eq!(report.totals.unsafe_independent, 0);
        assert_eq!(report.totals.unsafe_rate(), Some(0.0));
    }

    #[test]
    fn ambiguous_labels_leave_both_sides_unsure() {
        let case = Case {
            id: "ambiguous".into(),
            shape: Shape::Ambiguous,
            tasks: vec![
                task("a", &[], &["src/a.rs"], &[]),
                task("b", &[], &["src/b.rs"], &[]),
            ],
            labels: vec![pair("a", "b", Label::Ambiguous)],
        };
        let proposals = [proposal("ambiguous", "a", "b", Claim::Independent)];
        let report = Report::score(&[case], &proposals).unwrap();
        assert_eq!(finding_of(&report, "a", "b"), Finding::AmbiguousLabel);
        assert_eq!(report.totals.ambiguous_label, 1);
        assert_eq!(report.totals.unsafe_independent, 0);
        assert_eq!(
            report.totals.independent_claims, 1,
            "the claim is counted as made"
        );
        assert_eq!(
            report.totals.decidable_claims, 0,
            "and an ambiguous label cannot decide it"
        );
        assert_eq!(report.totals.unsafe_rate(), None);
    }

    #[test]
    fn deterministic_versus_label_disagreements_are_named() {
        let proposals = [
            proposal("mixed", "reader", "tree", Claim::Conflicts),
            proposal("mixed", "reader", "writer", Claim::Conflicts),
            proposal("mixed", "silent", "tree", Claim::Independent),
            proposal("mixed", "silent", "writer", Claim::Conflicts),
            proposal("mixed", "tree", "writer", Claim::Conflicts),
        ];
        let report = Report::score(&[mixed()], &proposals).unwrap();
        // The rule frees `reader`/`silent` — disjoint declared paths —
        // but the label says they collide and nothing objected: a
        // deterministic over-optimism, named rather than hidden.
        assert_eq!(
            finding_of(&report, "reader", "silent"),
            Finding::UncaughtRuleMiss
        );
        // The same rule miss, but the proposal objected — the model
        // added safety the declared rules lacked.
        assert_eq!(
            finding_of(&report, "reader", "writer"),
            Finding::CaughtRuleMiss
        );
        // `tree` writes `src/` and `reader` reads under it, so the
        // declared rule collides; the label says independent — the
        // headroom exists and the proposal did not claim it.
        assert_eq!(
            finding_of(&report, "reader", "tree"),
            Finding::UnrecoveredHeadroom
        );
        // Rule and label agree the pair is free; the proposal's caution
        // costs parallelism and never safety.
        assert_eq!(
            finding_of(&report, "silent", "writer"),
            Finding::OvercautiousProposal
        );
        assert_eq!(
            finding_of(&report, "silent", "tree"),
            Finding::UnsafeIndependent
        );
        assert_eq!(finding_of(&report, "tree", "writer"), Finding::Unanimous);
        assert_eq!(report.totals.uncaught_rule_miss, 1);
        assert_eq!(report.totals.caught_rule_miss, 1);
        assert_eq!(report.totals.unrecovered_headroom, 1);
        assert_eq!(report.totals.overcautious_proposal, 1);
        assert_eq!(report.totals.unanimous, 1);
        assert_eq!(report.totals.unsafe_independent, 1);
    }

    #[test]
    fn the_gate_admits_under_the_bound_and_marks_advisory_only_over_it() {
        // A clean report meets a strict bound — at or under admits.
        let proposals = [
            proposal("disjoint", "a", "b", Claim::Independent),
            proposal("disjoint", "a", "c", Claim::Independent),
            proposal("disjoint", "b", "c", Claim::Independent),
        ];
        let mut report = Report::score(&[disjoint()], &proposals).unwrap();
        let gate = Gate::new(0.0).unwrap();
        assert_eq!(gate.judge(&mut report), Standing::MayInform);
        let verdict = report.gate.unwrap();
        assert_eq!(verdict.standing, Standing::MayInform);
        assert_eq!(verdict.unsafe_bound, 0.0);
        assert_eq!(verdict.unsafe_rate, Some(0.0));
        assert_eq!(verdict.decidable_claims, 3);

        // One unsafe claim in three decidable claims is a rate of 1/3:
        // under a half it informs, at a quarter it does not.
        let proposals = [
            proposal("mixed", "reader", "tree", Claim::Independent),
            proposal("mixed", "silent", "tree", Claim::Independent),
            proposal("mixed", "silent", "writer", Claim::Independent),
        ];
        let mut report = Report::score(&[mixed()], &proposals).unwrap();
        assert_eq!(report.totals.unsafe_independent, 1);
        assert_eq!(report.totals.decidable_claims, 3);
        assert_eq!(
            Gate::new(0.5).unwrap().judge(&mut report),
            Standing::MayInform
        );
        let mut report = Report::score(&[mixed()], &proposals).unwrap();
        let standing = Gate::new(0.25).unwrap().judge(&mut report);
        assert_eq!(standing, Standing::AdvisoryOnly);
        assert_eq!(
            report.gate.unwrap().standing,
            Standing::AdvisoryOnly,
            "over the bound, a proposal annotates and never relaxes"
        );

        // Nothing decidable claimed is an unmeasured rate, and an
        // unmeasured rate is under no bound.
        let mut report = Report::score(&[disjoint()], &[]).unwrap();
        assert_eq!(
            Gate::new(1.0).unwrap().judge(&mut report),
            Standing::AdvisoryOnly
        );
        assert!(Gate::new(1.5).is_err());
        assert!(Gate::new(-0.1).is_err());
        assert!(Gate::new(f64::NAN).is_err());
    }

    #[test]
    fn the_same_inputs_write_the_same_report() {
        let cases = [colliding(), disjoint()];
        let proposals = [
            proposal("colliding", "a", "b", Claim::Independent),
            proposal("colliding", "b", "c", Claim::Uncertain),
            proposal("disjoint", "b", "a", Claim::Independent),
        ];
        let first = Report::score(&cases, &proposals).unwrap();
        let second = Report::score(&cases, &proposals).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap(),
            "the same fixtures and proposals produce the same bytes"
        );
    }

    #[test]
    fn the_unsafe_direction_never_folds_into_one_number() {
        // Two unanimous answers and one unsafe claim: the unsafe one is
        // its own count, not a deduction from a shared score.
        let case = colliding();
        let proposals = [
            proposal("colliding", "a", "b", Claim::Independent),
            proposal("colliding", "a", "c", Claim::Conflicts),
            proposal("colliding", "b", "c", Claim::Conflicts),
        ];
        let report = Report::score(&[case], &proposals).unwrap();
        assert_eq!(report.totals.unanimous, 2);
        assert_eq!(report.totals.unsafe_independent, 1);
        let value = serde_json::to_value(&report).unwrap();
        assert!(
            value.get("accuracy").is_none() && value["totals"].get("accuracy").is_none(),
            "the report carries named counts, never a merged accuracy"
        );
    }

    #[test]
    fn incomplete_or_defective_evidence_refuses() {
        // A pair without a label is an incomplete fixture.
        let mut case = disjoint();
        case.labels.pop();
        assert!(Report::score(&[case], &[]).is_err());

        // A label naming a task the set does not carry refuses.
        let mut case = disjoint();
        case.labels[0] = pair("a", "ghost", Label::ReallyIndependent);
        assert!(Report::score(&[case], &[]).is_err());

        // So do a duplicated label, a self-pair, and a cyclic after-edge.
        let mut case = disjoint();
        case.labels.push(pair("b", "a", Label::ReallyIndependent));
        assert!(Report::score(&[case], &[]).is_err());
        let mut case = disjoint();
        case.labels[0] = pair("a", "a", Label::ReallyIndependent);
        case.labels[1] = pair("b", "c", Label::ReallyIndependent);
        assert!(Report::score(&[case], &[]).is_err());
        let mut case = disjoint();
        case.tasks[0].after = vec!["b".into()];
        assert!(Report::score(&[case], &[]).is_err());

        // Proposals the fixtures cannot place refuse rather than drop:
        // an unknown case, an unknown task, and a pair answered twice.
        assert!(
            Report::score(
                &[disjoint()],
                &[proposal("ghost", "a", "b", Claim::Independent)]
            )
            .is_err()
        );
        assert!(
            Report::score(
                &[disjoint()],
                &[proposal("disjoint", "a", "ghost", Claim::Independent)]
            )
            .is_err()
        );
        assert!(
            Report::score(
                &[disjoint()],
                &[
                    proposal("disjoint", "a", "b", Claim::Independent),
                    proposal("disjoint", "b", "a", Claim::Conflicts),
                ]
            )
            .is_err(),
            "either endpoint order lands on the same pair"
        );
    }
}
