//! What the operator authorizes a program to do.
//!
//! [`crate::runtime`] runs the program a decision model selected, and a
//! selection is a proposal: which of the programs this host would run
//! does the request look like. This module is the other half of the
//! question — whether the operator lets a selected program do what it
//! proposes. Neither half implies the other. A request that reads like a
//! program request grants nothing, and a grant that names a program does
//! not select it.
//!
//! The [`Grant`] is the operator's answer, built from the session's
//! settings — [`PROGRAMS_ENV`], the `--programs` flag, and the
//! [`EFFECTS_ENV`] ceiling — before anything runs. It is held against a
//! program twice: at admission, where [`Runtime::authorize`] refuses a
//! program the grant does not name or whose declared effects exceed the
//! ceiling, and at dispatch, where a work item's own `writes` is read
//! off the item rather than trusted from a declaration, because a `query`
//! step's source answers after admission ran.
//!
//! What a grant is not: the model's. A readonly probability, an
//! independence answer, the bullets a request happened to carry, and the
//! selection itself are all data the program reads, and none of them
//! grants an effect. `CODER_SHELL` is the neighboring boundary — it
//! governs the command loop a reply may run, and says nothing about
//! program delegation; the two are separate questions on purpose.
//!
//! # What the axes mean, honestly
//!
//! [`Effects`] names six axes. They are authorization axes: whether the
//! operator allows the effect, not whether the host confines it. The
//! filesystem boundary confines *writes* only — it confines neither
//! reads nor network access, and a step that names a restriction this
//! host cannot enforce refuses at admission for `bound_unenforceable`
//! rather than running unbounded. Granting `reads` or `network` says the
//! operator permits the disclosure; it never claims a wall exists around
//! it.
//!
//! # Composition narrows, never widens
//!
//! A `program` step is refused at admission — composition is not built —
//! but the grant's shape is already the one it needs: [`Grant::meets`]
//! intersects two grants, and [`Grant::narrowed`] applies a ceiling, so a
//! child scope can ask for less than its parent and never more.

use std::collections::BTreeSet;
use std::env;

use serde_json::{Value, json};

use crate::capability::is_slug;

/// The variable naming which programs this session may run.
///
/// `all` (or `*`) names every program the host resolves; `none`, `off`,
/// `no`, `false`, and `0` name none — which is also what unset means.
/// Anything else is a comma-separated list of program slugs, and a word
/// that is no slug grants nothing and is noted in the record.
pub const PROGRAMS_ENV: &str = "CODER_PROGRAMS";

/// The variable bounding the effects an authorized program may have.
///
/// A comma-separated list of the effect words [`Effects::named`] reads —
/// `reads`, `writes`, `delegation`, `network`, `subprocesses`, `spend` —
/// or `none` to allow none. Unset, the ceiling is every effect: the
/// variable bounds what an authorized program may do, and authorizing a
/// program is [`PROGRAMS_ENV`]'s job, not this one's.
pub const EFFECTS_ENV: &str = "CODER_PROGRAM_EFFECTS";

/// The refusal code a program earns when the grant does not cover it.
pub const UNAUTHORIZED: &str = "unauthorized";

/// The name the grant decision records itself under in a trace.
pub const AUTHORITY_CALL: &str = "program_authority";

/// The effects a program's steps may produce, and the ceiling a grant
/// puts on them.
///
/// One flag per axis. A grant's [`Effects`] is a ceiling — the most a run
/// under it may do — and a step's is what running it will do, derived
/// from the step's kind and the executor the run names rather than
/// stated in the program, because a program's own words are a claim and
/// the grant is held against what the step will do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Reading what a `query` step's source holds, or what a delegated
    /// task reads of its checkout.
    pub reads: bool,
    /// A delegated task that writes. Checked at dispatch off the work
    /// item itself — the one axis a `query` step's answer can decide
    /// after admission ran.
    pub writes: bool,
    /// Handing work to an executor at all.
    pub delegation: bool,
    /// Program state or task work leaving this machine: a `decide`
    /// step's state to the decision door, a task to a relay worker, or
    /// whatever a spawned process chooses to send.
    pub network: bool,
    /// An executor spawned as a supervised subprocess.
    pub subprocesses: bool,
    /// Work that can bill an account: decision calls, and an executor
    /// whose manifest's cost is not `local`.
    pub spend: bool,
}

impl Effects {
    /// No effects: the ceiling under which a program may only describe
    /// work.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            reads: false,
            writes: false,
            delegation: false,
            network: false,
            subprocesses: false,
            spend: false,
        }
    }

    /// Every effect.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            reads: true,
            writes: true,
            delegation: true,
            network: true,
            subprocesses: true,
            spend: true,
        }
    }

    /// The union of two effect sets.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            reads: self.reads || other.reads,
            writes: self.writes || other.writes,
            delegation: self.delegation || other.delegation,
            network: self.network || other.network,
            subprocesses: self.subprocesses || other.subprocesses,
            spend: self.spend || other.spend,
        }
    }

    /// The intersection of two effect sets: what both allow. A child
    /// scope's ceiling is a meet, so it can only narrow its parent's.
    #[must_use]
    pub fn meet(self, other: Self) -> Self {
        Self {
            reads: self.reads && other.reads,
            writes: self.writes && other.writes,
            delegation: self.delegation && other.delegation,
            network: self.network && other.network,
            subprocesses: self.subprocesses && other.subprocesses,
            spend: self.spend && other.spend,
        }
    }

    /// The effects this declares that `ceiling` does not grant, as words.
    #[must_use]
    pub fn missing(self, ceiling: Self) -> Vec<&'static str> {
        Self {
            reads: self.reads && !ceiling.reads,
            writes: self.writes && !ceiling.writes,
            delegation: self.delegation && !ceiling.delegation,
            network: self.network && !ceiling.network,
            subprocesses: self.subprocesses && !ceiling.subprocesses,
            spend: self.spend && !ceiling.spend,
        }
        .words()
    }

    /// The words this set holds, in the order a record lists them.
    #[must_use]
    pub fn words(self) -> Vec<&'static str> {
        let mut words = Vec::new();
        for (word, set) in [
            ("reads", self.reads),
            ("writes", self.writes),
            ("delegation", self.delegation),
            ("network", self.network),
            ("subprocesses", self.subprocesses),
            ("spend", self.spend),
        ] {
            if set {
                words.push(word);
            }
        }
        words
    }

    /// The effects one spec word grants, or `None` for a word that names
    /// no axis — which grants nothing.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        let mut effects = Self::none();
        match word {
            "reads" => effects.reads = true,
            "writes" => effects.writes = true,
            "delegation" => effects.delegation = true,
            "network" => effects.network = true,
            "subprocesses" => effects.subprocesses = true,
            "spend" => effects.spend = true,
            _ => return None,
        }
        Some(effects)
    }
}

/// Which programs a grant names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Programs {
    /// None: the session runs no program, whatever is selected.
    None,
    /// Every program the host resolves.
    All,
    /// These slugs and no others.
    Named(BTreeSet<String>),
}

impl Programs {
    /// Whether this set names `slug`.
    #[must_use]
    pub fn authorizes(&self, slug: &str) -> bool {
        match self {
            Programs::None => false,
            Programs::All => true,
            Programs::Named(slugs) => slugs.contains(slug),
        }
    }

    /// The union of two program sets: a grant read from two places
    /// authorizes what either named.
    fn union(self, other: Self) -> Self {
        match (self, other) {
            (Programs::All, _) | (_, Programs::All) => Programs::All,
            (Programs::None, other) | (other, Programs::None) => other,
            (Programs::Named(mut mine), Programs::Named(theirs)) => {
                mine.extend(theirs);
                Programs::Named(mine)
            }
        }
    }

    /// The intersection of two program sets: what both name. A child
    /// scope's set is a meet, so it can only narrow its parent's.
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Programs::None, _) | (_, Programs::None) => Programs::None,
            (Programs::All, other) => other.clone(),
            (mine, Programs::All) => mine.clone(),
            (Programs::Named(mine), Programs::Named(theirs)) => {
                let slugs: BTreeSet<String> = mine.intersection(theirs).cloned().collect();
                match slugs.is_empty() {
                    true => Programs::None,
                    false => Programs::Named(slugs),
                }
            }
        }
    }
}

/// The operator's grant: which programs may run this session, and the
/// effects a run may have.
///
/// Build it with [`Grant::operator`] — the environment merged with the
/// command line — or [`Grant::selected`] when a caller holds the words
/// itself. A `Grant` is passed to [`Runtime::run`] for each run rather
/// than remembered, so a grant the operator withdrew between runs is not
/// handed out anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    programs: Programs,
    /// The ceiling a run under this grant runs beneath.
    effects: Effects,
    /// What a spec said that granted nothing, for the record.
    notes: Vec<String>,
}

impl Grant {
    /// A grant of nothing: no program is authorized, and no effect is
    /// allowed. The default a session carries until the operator says
    /// otherwise — ordinary chat is not a program grant, whatever it
    /// reads like.
    #[must_use]
    pub fn none() -> Self {
        Self {
            programs: Programs::None,
            effects: Effects::none(),
            notes: Vec::new(),
        }
    }

    /// A grant of every program at every effect: what `CODER_PROGRAMS=all`
    /// spells, and what a test that means to run a program uses. A
    /// deliberate word, never a default.
    #[must_use]
    pub fn all() -> Self {
        Self {
            programs: Programs::All,
            effects: Effects::all(),
            notes: Vec::new(),
        }
    }

    /// The operator's grant: [`PROGRAMS_ENV`] merged with `spec` — the
    /// command line's `--programs` — under the [`EFFECTS_ENV`] ceiling.
    ///
    /// The two program sources union: a slug the operator named through
    /// either channel is granted. The effects ceiling is the
    /// environment's alone — a ceiling narrows, it does not merge upward.
    #[must_use]
    pub fn operator(spec: Option<&str>) -> Self {
        let mut grant = Self::selected(
            env::var(PROGRAMS_ENV).ok().as_deref(),
            env::var(EFFECTS_ENV).ok().as_deref(),
        );
        if let Some(spec) = spec {
            let given = Self::selected(Some(spec), None);
            grant.programs = grant.programs.union(given.programs);
            grant.notes.extend(given.notes);
        }
        grant
    }

    /// A grant from explicit words, as [`Grant::operator`] reads them:
    /// `programs` is the list `CODER_PROGRAMS` or `--programs` spells,
    /// `effects` the `CODER_PROGRAM_EFFECTS` ceiling. Deterministic, for
    /// the caller that holds the words rather than the environment.
    #[must_use]
    pub fn selected(programs: Option<&str>, effects: Option<&str>) -> Self {
        let (programs, mut notes) = match programs {
            Some(spec) => parse_programs(spec),
            None => (Programs::None, Vec::new()),
        };
        let (effects, mut more) = parse_effects(effects);
        notes.append(&mut more);
        Self {
            programs,
            effects,
            notes,
        }
    }

    /// Whether this grant names the program.
    #[must_use]
    pub fn authorizes(&self, slug: &str) -> bool {
        self.programs.authorizes(slug)
    }

    /// The ceiling a run under this grant runs beneath.
    #[must_use]
    pub fn effects(&self) -> Effects {
        self.effects
    }

    /// The effects `declared` needs that this grant does not allow, as
    /// words.
    #[must_use]
    pub fn missing(&self, declared: Effects) -> Vec<&'static str> {
        declared.missing(self.effects)
    }

    /// What the grant specs said that granted nothing — a word that is
    /// no slug, an axis nobody named. Kept so the record can say why a
    /// grant covers less than it looks like it does.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// This grant narrowed by a ceiling: a child scope can ask for less,
    /// never more.
    #[must_use]
    pub fn narrowed(&self, ceiling: Effects) -> Self {
        self.meets(&Self {
            programs: Programs::All,
            effects: ceiling,
            notes: Vec::new(),
        })
    }

    /// The intersection of two grants: the programs both name and the
    /// effects both allow.
    ///
    /// This is the shape program composition needs when it lands — a
    /// child program runs under `parent.meets(child)`, which cannot name
    /// a program the parent did not or allow an effect the parent denied.
    /// The `program` step kind refuses at admission today; this is the
    /// operation it will run under, tested before it is used.
    #[must_use]
    pub fn meets(&self, child: &Self) -> Self {
        let mut notes = self.notes.clone();
        notes.extend(child.notes.iter().cloned());
        Self {
            programs: self.programs.meet(&child.programs),
            effects: self.effects.meet(child.effects),
            notes,
        }
    }

    /// What the grant is, as the trace records it.
    #[must_use]
    pub fn value(&self) -> Value {
        json!({
            "programs": match &self.programs {
                Programs::None => json!("none"),
                Programs::All => json!("all"),
                Programs::Named(slugs) => json!(slugs.iter().collect::<Vec<_>>()),
            },
            "effects": self.effects.words(),
        })
    }
}

/// The program set a spec names, and which words in it named nothing.
///
/// The notes are why a spec grants less than it looks like it does: a
/// caller that takes them as an error can refuse a typo'd `--programs`
/// at the command line rather than discovering it as a session that
/// authorized nothing.
pub fn parse_programs(spec: &str) -> (Programs, Vec<String>) {
    let spec = spec.trim();
    if spec.is_empty() {
        return (Programs::None, Vec::new());
    }
    match spec.to_ascii_lowercase().as_str() {
        "all" | "any" | "*" => return (Programs::All, Vec::new()),
        "0" | "off" | "no" | "false" | "none" => return (Programs::None, Vec::new()),
        _ => {}
    }
    let mut slugs = BTreeSet::new();
    let mut notes = Vec::new();
    for word in spec
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
    {
        match word.to_ascii_lowercase().as_str() {
            "all" | "any" | "*" => return (Programs::All, notes),
            "0" | "off" | "no" | "false" | "none" => {}
            _ if is_slug(word) => {
                slugs.insert(word.to_string());
            }
            _ => notes.push(format!("{word:?} is not a program slug and grants nothing")),
        }
    }
    match slugs.is_empty() {
        true => (Programs::None, notes),
        false => (Programs::Named(slugs), notes),
    }
}

/// The effects ceiling a spec names, and which words in it named none.
///
/// Unset means every effect — this is a ceiling on programs that are
/// already authorized, not a grant, so the default is wide and the
/// narrowing is the operator's word.
fn parse_effects(spec: Option<&str>) -> (Effects, Vec<String>) {
    let Some(spec) = spec.map(str::trim).filter(|spec| !spec.is_empty()) else {
        return (Effects::all(), Vec::new());
    };
    match spec.to_ascii_lowercase().as_str() {
        "all" | "any" | "*" => return (Effects::all(), Vec::new()),
        "0" | "off" | "no" | "false" | "none" => return (Effects::none(), Vec::new()),
        _ => {}
    }
    let mut effects = Effects::none();
    let mut notes = Vec::new();
    for word in spec
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
    {
        match Effects::named(word) {
            Some(one) => effects = effects.union(one),
            None => notes.push(format!("{word:?} is not an effect and grants nothing")),
        }
    }
    (effects, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unset and `none` grant no program: ordinary chat is not a program
    /// grant, whatever it reads like.
    #[test]
    fn the_default_grant_authorizes_nothing() {
        for spec in [None, Some(""), Some("none"), Some("off"), Some("0")] {
            let grant = Grant::selected(spec, None);
            assert!(!grant.authorizes("burn-down"), "{spec:?}");
            assert!(!grant.authorizes("delegate-fan-out"), "{spec:?}");
        }
    }

    /// `all` names every program; a slug list names only its own.
    #[test]
    fn a_grant_names_what_it_names() {
        assert!(Grant::selected(Some("all"), None).authorizes("burn-down"));
        let grant = Grant::selected(Some("burn-down, review-changes"), None);
        assert!(grant.authorizes("burn-down"));
        assert!(grant.authorizes("review-changes"));
        assert!(!grant.authorizes("delegate-fan-out"));
        assert_eq!(grant.effects(), Effects::all(), "the ceiling is unset");
    }

    /// A word that is no slug grants nothing and says so, rather than
    /// quietly widening or quietly failing.
    #[test]
    fn a_spec_word_that_is_no_slug_grants_nothing() {
        let (programs, notes) = parse_programs("burn down; rm -rf /");
        assert_eq!(programs, Programs::None);
        assert_eq!(notes.len(), 1);
        let (programs, notes) = parse_programs("burn-down,Not A Slug");
        assert!(programs.authorizes("burn-down"));
        assert_eq!(notes.len(), 1, "{notes:?}");
    }

    /// The effects ceiling is a ceiling: unset allows everything, a list
    /// allows what it names, and a word that is no axis grants nothing.
    #[test]
    fn the_effects_ceiling_allows_what_it_names() {
        assert_eq!(Grant::selected(None, None).effects(), Effects::all());
        assert_eq!(
            Grant::selected(None, Some("none")).effects(),
            Effects::none()
        );
        let grant = Grant::selected(None, Some("reads,writes,bogus"));
        let allowed = grant.effects();
        assert!(allowed.reads && allowed.writes);
        assert!(!allowed.delegation && !allowed.network && !allowed.spend);
        assert_eq!(grant.notes().len(), 1, "bogus named nothing");
    }

    /// A meet can only narrow: no arrangement of the two grants produces
    /// a program or an effect either one denied.
    #[test]
    fn a_child_scope_narrows_and_never_widens() {
        let parent = Grant::selected(
            Some("burn-down"),
            Some("reads,writes,delegation,network,subprocesses,spend"),
        );
        for child in [
            Grant::all(),
            Grant::selected(Some("burn-down,delegate-fan-out"), None),
            parent.narrowed(Effects::all()),
        ] {
            let grant = parent.meets(&child);
            assert!(grant.authorizes("burn-down"), "{grant:?}");
            assert!(!grant.authorizes("delegate-fan-out"), "{grant:?}");
            assert_eq!(grant.effects(), parent.effects());
        }
        // And asking for less gives less.
        let narrow = parent.narrowed(Effects {
            reads: true,
            delegation: true,
            network: true,
            subprocesses: true,
            spend: true,
            writes: false,
        });
        assert!(narrow.authorizes("burn-down"));
        assert!(!narrow.effects().writes);
        assert!(
            narrow
                .missing(Effects {
                    writes: true,
                    ..Effects::none()
                })
                .contains(&"writes")
        );
        // A parent that grants nothing meets everything as nothing.
        assert_eq!(Grant::none().meets(&Grant::all()), Grant::none());
    }

    /// `missing` names what the ceiling denies, for the refusal a reader
    /// has to act on.
    #[test]
    fn a_missing_effect_is_named() {
        let ceiling = Effects::named("delegation").unwrap();
        let declared = Effects {
            delegation: true,
            network: true,
            ..Effects::none()
        };
        assert_eq!(declared.missing(ceiling), ["network"]);
        assert!(declared.missing(Effects::all()).is_empty());
    }
}
