//! Child programs: the contract a `program` step makes before anything
//! runs.
//!
//! A `program` step names another program the way a package names a
//! dependency: pinned like one, bounded like a guest, and its outcome
//! enters the parent only through a stated rule. Composition is a
//! document property — the whole graph a program can reach is knowable
//! before the first step runs, and a composition a host cannot fully
//! account for is refused rather than discovered mid-run.
//!
//! # Pinned, or reported
//!
//! A [`ChildRef`] is the address a `program` step carries: `sha256:` and
//! a digest, a `name@release`, or a bare name. The first two pin. The
//! third is allowed in a document, and [`ChildRef::resolve`] reports it
//! as [`Pin::Unpinned`] rather than silently trusting whatever the name
//! happens to resolve to today.
//!
//! # Bounds narrow, never widen
//!
//! [`Composition::check`] walks the whole graph before any execution: a
//! program that reaches itself through any chain of children is refused
//! with the cycle's path named, nesting past [`MAX_DEPTH`] refuses, more
//! steps or `program` calls than [`MAX_STEPS`] or [`MAX_CALLS`] refuses,
//! and a child whose declared bounds exceed what its parent has left is
//! a widening — a typed [`Problem`], never a clamp. A child gets only
//! what its parent still holds.
//!
//! # Nothing here runs
//!
//! This module is validation and planning. It runs no step, calls no
//! model, and spawns no process. A document that tries to carry
//! executable content — a `command`, `code`, or `wasm` field on a step —
//! refuses with the field named, because a program names sources,
//! questions, and capabilities, and carries no code.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::capability::is_slug;
use crate::program::{Kind, Program, Registry, Step};

/// The deepest a composition may nest, counting the program itself as
/// the first level — NIP-PRG's common depth limit. A host may hold a
/// lower bound; it may not hold a higher one.
pub const MAX_DEPTH: u64 = 8;

/// The most steps the whole expanded composition may hold.
pub const MAX_STEPS: u64 = 256;

/// The most `program` steps the whole expanded composition may invoke.
pub const MAX_CALLS: u64 = 64;

/// The step fields that would make a program carry executable content.
///
/// A program names sources, questions, and capabilities; it never
/// carries an argv, an expression, or a module's bytes.
const EXECUTABLE_FIELDS: &[&str] = &["command", "code", "wasm"];

/// The bounds a `program` step may declare for its child, checked
/// against what the parent has left.
const CHILD_BOUNDS: &[&str] = &["depth", "steps", "calls"];

/// Where one named input comes from.
///
/// The document tags each input with `from`: `{"from": "state",
/// "field": "request"}`, `{"from": "step", "step": "select"}`, or
/// `{"from": "literal", "value": ...}`. There is no expression and no
/// lookup — an input is one of three stated origins.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum Input {
    /// A field of the state the run began with.
    State { field: String },
    /// The output an earlier step produced, named by that step. `field`
    /// addresses one of its exposed fields when stated; unstated, the
    /// input is the step's whole output.
    Step {
        step: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
    /// A value the document states.
    Literal { value: Value },
}

/// A step's declared input/output contract: what it consumes, what it
/// must produce, and what a later step may reference.
///
/// The contract is a document property. A step that consumes an output
/// no earlier step produces is a structural problem — reported, never
/// assumed — and a step may offer a later step only the fields it
/// produces.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Binding {
    /// The named inputs the step consumes, each typed by its origin.
    #[serde(default)]
    pub inputs: BTreeMap<String, Input>,
    /// The fields the step's output must carry, each with the shape
    /// word the document gives it.
    #[serde(default)]
    pub produces: BTreeMap<String, String>,
    /// The fields a later step may reference — a subset of `produces`.
    #[serde(default)]
    pub exposes: Vec<String>,
}

impl Binding {
    /// The contract a step declares under `binding`, when it declares
    /// one.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the field is present and does not read
    /// as an input/output contract.
    pub fn of(step: &Step) -> Result<Option<Self>, String> {
        match step.rest.get("binding") {
            None => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|e| format!("binding does not read as an input/output contract: {e}")),
        }
    }
}

/// What a child's run can end as — the three endings a propagation
/// table must answer for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The child finished its steps.
    Completed,
    /// The child ran and failed.
    Failed,
    /// The child was refused — a bound it asked for, a step that
    /// declined, or the host.
    Refused,
}

impl Outcome {
    /// Where this ending lands in the parent under `table`.
    ///
    /// The answer is always the row the document stated; a mapping that
    /// was never written down is a [`Problem::Propagation`], not a
    /// default.
    #[must_use]
    pub fn propagation(self, table: &Propagation) -> Effect {
        match self {
            Outcome::Completed => table.completed,
            Outcome::Failed => table.failed,
            Outcome::Refused => table.refused,
        }
    }
}

/// Where a child's ending lands on the parent's step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// The parent's step succeeded.
    Success,
    /// The parent's step failed.
    Failure,
    /// The parent's step refused.
    Refusal,
}

/// The stated mapping from a child's terminal state to the parent
/// step's result: three rows, all stated, none defaulted.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Propagation {
    /// Where a completed child lands.
    pub completed: Effect,
    /// Where a failed child lands.
    pub failed: Effect,
    /// Where a refused child lands.
    pub refused: Effect,
}

impl Propagation {
    /// The table a `program` step states under `propagation`, when it
    /// states one.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the field is present and does not read
    /// as a table of the three outcomes.
    pub fn of(step: &Step) -> Result<Option<Self>, String> {
        match step.rest.get("propagation") {
            None => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|e| format!("propagation does not read as the three outcomes: {e}")),
        }
    }
}

/// The address a `program` step carries for its child.
///
/// Parse with [`ChildRef::parse`]; resolve with [`ChildRef::resolve`].
/// A bare name is allowed in a document — and reported as
/// [`Pin::Unpinned`] rather than trusted silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChildRef {
    /// `sha256:` and the digest of the program's canonical form.
    Digest { digest: String },
    /// A name and a release label: `review-changes@1.2.0`.
    Named { name: String, version: String },
    /// A bare name: resolves to whatever the registry holds today.
    Bare { name: String },
}

impl ChildRef {
    /// The reference an address spells.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the address is not a child
    /// reference: a digest that is not one, a malformed `name@release`,
    /// or a bare name that is not a program slug.
    pub fn parse(address: &str) -> Result<Self, String> {
        if let Some(hex) = address.strip_prefix("sha256:") {
            if is_digest(hex) {
                return Ok(ChildRef::Digest {
                    digest: hex.to_string(),
                });
            }
            return Err(format!(
                "{hex:?} is not a digest of 64 lowercase hexadecimal characters"
            ));
        }
        if let Some((name, version)) = address.split_once('@') {
            if !is_slug(name) {
                return Err(format!("{name:?} is not a program slug"));
            }
            if version.is_empty() || version.contains('@') {
                return Err(format!("{address:?} states no release"));
            }
            return Ok(ChildRef::Named {
                name: name.to_string(),
                version: version.to_string(),
            });
        }
        if is_slug(address) {
            return Ok(ChildRef::Bare {
                name: address.to_string(),
            });
        }
        Err(format!(
            "{address:?} is not a child address: a digest, a name@release, or a program slug"
        ))
    }

    /// What the reference resolves to in `registry`, and how firmly it
    /// held what it found.
    ///
    /// `None` when nothing in the registry answers. A bare name that
    /// resolves comes back as [`Pin::Unpinned`] — reported, never
    /// trusted silently.
    #[must_use]
    pub fn resolve(&self, registry: &Registry) -> Option<Resolution> {
        match self {
            ChildRef::Digest { digest } => registry
                .programs()
                .iter()
                .find(|program| &crate::child::digest(program) == digest)
                .map(|program| Resolution {
                    slug: program.slug.clone(),
                    pin: Pin::Digest {
                        digest: digest.clone(),
                    },
                }),
            ChildRef::Named { name, version } => registry.get(name).map(|_| Resolution {
                slug: name.clone(),
                pin: Pin::Release {
                    name: name.clone(),
                    version: version.clone(),
                },
            }),
            ChildRef::Bare { name } => registry.get(name).map(|_| Resolution {
                slug: name.clone(),
                pin: Pin::Unpinned { name: name.clone() },
            }),
        }
    }
}

/// How firmly a reference held the program it resolved to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pin {
    /// The reference carried the program's own digest — the strongest
    /// hold a reference can take.
    Digest { digest: String },
    /// The reference carried a name and a release label. The label is
    /// verified where release labels live; the address itself does not
    /// float.
    Release { name: String, version: String },
    /// The reference was a bare name, resolved to whatever the registry
    /// holds today — reported, never trusted silently.
    Unpinned { name: String },
}

/// What a [`ChildRef`] resolved to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// The slug of the program the reference resolved to.
    pub slug: String,
    /// How firmly the reference held it.
    pub pin: Pin,
}

/// The canonical digest of a resolved program — the identity a `sha256:`
/// reference pins.
///
/// [`atif::digest`] sorts every key before hashing, so two reads of the
/// same program produce the same identity regardless of field order.
#[must_use]
pub fn digest(program: &Program) -> String {
    atif::digest(&json!(program))
}

/// Whether `stated` reads as a digest: 64 lowercase hexadecimal
/// characters.
fn is_digest(stated: &str) -> bool {
    stated.len() == 64
        && stated
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// One way a composition is not one this host runs.
///
/// Every variant names what it found, because a composition that checks
/// partway is a wrong graph, not a partial one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// A step carries a field that is executable content — `command`,
    /// `code`, or `wasm` — and a program holds none.
    Executable { step: String, field: String },
    /// A declared contract — a binding, a propagation table — is
    /// present and does not read as the shape it claims.
    Contract { step: String, reason: String },
    /// A step consumes an output no earlier step produces.
    MissingProducer {
        step: String,
        input: String,
        producer: String,
    },
    /// A step offers a later step a field it does not produce.
    Unproduced { step: String, field: String },
    /// A step addresses a field its producer does not expose.
    Unexposed {
        step: String,
        input: String,
        producer: String,
        field: String,
    },
    /// A `program` step's address does not read as a child reference.
    Address {
        step: String,
        address: String,
        reason: String,
    },
    /// A `program` step states no propagation table. The mapping is
    /// stated in the document or the step is refused; it is never
    /// defaulted.
    Propagation { step: String },
    /// A `program` step's reference is a bare name, which resolves to
    /// whatever the name means today.
    Unpinned { step: String, name: String },
    /// A `program` step's reference resolves to nothing.
    Unresolved { step: String, reference: String },
    /// A program reaches itself through a chain of children. The path
    /// names the whole chain, first revisitation included.
    Cyclic { path: String },
    /// The composition would nest deeper than the bound allows.
    Depth {
        step: String,
        depth: u64,
        bound: u64,
    },
    /// The composition's steps or `program` calls total more than the
    /// bound.
    Budget {
        what: &'static str,
        total: u64,
        bound: u64,
    },
    /// A child declares a bound wider than its parent has left —
    /// refused, never clamped.
    Widening {
        step: String,
        bound: &'static str,
        declared: u64,
        remaining: u64,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Executable { step, field } => write!(
                f,
                "step {step:?} carries {field}, which is executable content a program never holds"
            ),
            Problem::Contract { step, reason } => write!(f, "step {step:?}: {reason}"),
            Problem::MissingProducer {
                step,
                input,
                producer,
            } => write!(
                f,
                "step {step:?} consumes {input:?} from step {producer:?}, which no earlier step produces"
            ),
            Problem::Unproduced { step, field } => write!(
                f,
                "step {step:?} exposes {field:?}, which it does not produce"
            ),
            Problem::Unexposed {
                step,
                input,
                producer,
                field,
            } => write!(
                f,
                "step {step:?} reads {field:?} for {input:?} from step {producer:?}, which does not expose it"
            ),
            Problem::Address {
                step,
                address,
                reason,
            } => write!(f, "program step {step:?} names {address:?}: {reason}"),
            Problem::Propagation { step } => write!(
                f,
                "program step {step:?} states no propagation table, and the mapping is never defaulted"
            ),
            Problem::Unpinned { step, name } => write!(
                f,
                "program step {step:?} names {name:?} with no pin, which resolves to whatever the name means today"
            ),
            Problem::Unresolved { step, reference } => write!(
                f,
                "program step {step:?} names {reference:?}, which nothing in the registry answers"
            ),
            Problem::Cyclic { path } => write!(f, "composition cycle: {path}"),
            Problem::Depth { step, depth, bound } => write!(
                f,
                "program step {step:?} would nest to depth {depth}, past the bound {bound}"
            ),
            Problem::Budget { what, total, bound } => write!(
                f,
                "the composition totals {total} {what}, past the bound {bound}"
            ),
            Problem::Widening {
                step,
                bound,
                declared,
                remaining,
            } => write!(
                f,
                "program step {step:?} declares {bound} {declared}, and its parent has {remaining} left — a child narrows, never widens"
            ),
        }
    }
}

impl std::error::Error for Problem {}

/// The shape a program and the programs it reaches make together.
///
/// `Composition` is a namespace for the one question this module
/// answers: is this graph, stated as documents, one a host can account
/// for before anything runs.
pub struct Composition;

impl Composition {
    /// Every structural problem the composition carries, in a
    /// deterministic order: each program's own steps first, then the
    /// children its `program` steps reach, in document order.
    ///
    /// An empty answer means the whole graph is one the host can
    /// account for: every input has a producer, every child is pinned,
    /// every propagation is stated, no chain returns to itself, and no
    /// bound is exceeded or widened. This is a check over documents —
    /// nothing runs, and nothing is clamped into place.
    #[must_use]
    pub fn check(program: &Program, registry: &Registry) -> Vec<Problem> {
        let mut walk = Walk {
            registry,
            problems: Vec::new(),
            checked: BTreeSet::new(),
            visiting: Vec::new(),
        };
        walk.visit(
            program,
            &Budget {
                depth: MAX_DEPTH,
                steps: MAX_STEPS,
                calls: MAX_CALLS,
            },
        );
        walk.problems
    }
}

/// What a subtree may still spend: the levels it may nest and the steps
/// and calls it may total, reservations shared rather than copied per
/// child.
#[derive(Clone, Copy)]
struct Budget {
    depth: u64,
    steps: u64,
    calls: u64,
}

impl Budget {
    /// What one bound still holds.
    fn of(&self, bound: &str) -> u64 {
        match bound {
            "depth" => self.depth,
            "steps" => self.steps,
            _ => self.calls,
        }
    }

    /// Narrow one bound to what a child declared.
    fn narrow(&mut self, bound: &str, declared: u64) {
        match bound {
            "depth" => self.depth = declared,
            "steps" => self.steps = declared,
            _ => self.calls = declared,
        }
    }
}

/// What a subtree actually spent: the deepest it nested and the steps
/// and calls it totaled.
struct Usage {
    depth: u64,
    steps: u64,
    calls: u64,
}

/// The walk [`Composition::check`] runs, carrying the registry it reads,
/// the problems it has found, the slugs whose own steps were checked,
/// and the chain of programs being expanded — the only place a cycle
/// can hide.
struct Walk<'a> {
    registry: &'a Registry,
    problems: Vec<Problem>,
    checked: BTreeSet<String>,
    visiting: Vec<String>,
}

impl<'a> Walk<'a> {
    /// The contract one program's own steps carry: executable content,
    /// bindings and their producers, and each `program` step's address,
    /// propagation table, and pin.
    ///
    /// Runs once per program — a child three steps name is checked as
    /// one program, not three times.
    fn contract(&mut self, program: &Program) {
        let mut produced: BTreeMap<String, Binding> = BTreeMap::new();
        for step in &program.steps {
            for field in EXECUTABLE_FIELDS {
                if step.rest.contains_key(*field) {
                    self.problems.push(Problem::Executable {
                        step: step.name.clone(),
                        field: (*field).to_string(),
                    });
                }
            }
            match Binding::of(step) {
                Err(reason) => self.problems.push(Problem::Contract {
                    step: step.name.clone(),
                    reason,
                }),
                Ok(Some(binding)) => {
                    self.binding(step, &binding, &produced);
                    produced.insert(step.name.clone(), binding);
                }
                Ok(None) => {}
            }
            if step.kind == Kind::Program {
                self.child_ref(step);
            }
        }
    }

    /// What one binding's inputs and exposes must hold: every step
    /// output consumed has an earlier producer that declared it, every
    /// field addressed is one the producer exposes, and every field
    /// exposed is one the step produces.
    fn binding(&mut self, step: &Step, binding: &Binding, produced: &BTreeMap<String, Binding>) {
        for field in &binding.exposes {
            if !binding.produces.contains_key(field) {
                self.problems.push(Problem::Unproduced {
                    step: step.name.clone(),
                    field: field.clone(),
                });
            }
        }
        for (name, input) in &binding.inputs {
            let Input::Step {
                step: producer,
                field,
            } = input
            else {
                continue;
            };
            match produced.get(producer) {
                None => self.problems.push(Problem::MissingProducer {
                    step: step.name.clone(),
                    input: name.clone(),
                    producer: producer.clone(),
                }),
                Some(makes) => {
                    if let Some(field) = field
                        && !makes.exposes.contains(field)
                    {
                        self.problems.push(Problem::Unexposed {
                            step: step.name.clone(),
                            input: name.clone(),
                            producer: producer.clone(),
                            field: field.clone(),
                        });
                    }
                }
            }
        }
    }

    /// What a `program` step declares about its child: an address that
    /// reads as a reference, a propagation table stated rather than
    /// defaulted, and a pin reported for what it is.
    fn child_ref(&mut self, step: &Step) {
        let Some(address) = step.program.as_deref() else {
            self.problems.push(Problem::Address {
                step: step.name.clone(),
                address: String::new(),
                reason: "names no program".to_string(),
            });
            return;
        };
        let reference = match ChildRef::parse(address) {
            Ok(reference) => Some(reference),
            Err(reason) => {
                self.problems.push(Problem::Address {
                    step: step.name.clone(),
                    address: address.to_string(),
                    reason,
                });
                None
            }
        };
        match Propagation::of(step) {
            Err(reason) => self.problems.push(Problem::Contract {
                step: step.name.clone(),
                reason,
            }),
            Ok(None) => self.problems.push(Problem::Propagation {
                step: step.name.clone(),
            }),
            Ok(Some(_)) => {}
        }
        let Some(reference) = reference else {
            return;
        };
        match reference.resolve(self.registry) {
            None => self.problems.push(Problem::Unresolved {
                step: step.name.clone(),
                reference: address.to_string(),
            }),
            Some(resolution) => {
                if let Pin::Unpinned { name } = resolution.pin {
                    self.problems.push(Problem::Unpinned {
                        step: step.name.clone(),
                        name,
                    });
                }
            }
        }
    }

    /// The program a `program` step's reference resolves to, when the
    /// address parses and something answers.
    fn resolved(&self, step: &Step) -> Option<&'a Program> {
        let reference = ChildRef::parse(step.program.as_deref()?).ok()?;
        let resolution = reference.resolve(self.registry)?;
        self.registry.get(&resolution.slug)
    }

    /// The child bounds a `program` step declares, in a fixed order. A
    /// declared bound that is not a count is a contract problem, not a
    /// declaration.
    fn declared(&mut self, step: &Step) -> Vec<(&'static str, u64)> {
        let mut declared = Vec::new();
        for bound in CHILD_BOUNDS {
            let Some(value) = step.bounds.get(*bound) else {
                continue;
            };
            match value.as_u64() {
                Some(count) => declared.push((*bound, count)),
                None => self.problems.push(Problem::Contract {
                    step: step.name.clone(),
                    reason: format!("declares bound {bound:?} as {value}, which is not a count"),
                }),
            }
        }
        declared
    }

    /// Expand one program: check its own contract once, then each
    /// `program` step's child — cycle, widening, and budget first, the
    /// child's own subtree after. Returns what the subtree spent.
    fn visit(&mut self, program: &Program, limits: &Budget) -> Usage {
        if self.checked.insert(program.slug.clone()) {
            self.contract(program);
        }
        self.visiting.push(program.slug.clone());
        let mut usage = Usage {
            depth: 1,
            steps: program.steps.len() as u64,
            calls: 0,
        };
        if usage.steps > limits.steps {
            self.problems.push(Problem::Budget {
                what: "steps",
                total: usage.steps,
                bound: limits.steps,
            });
        } else {
            for step in &program.steps {
                if step.kind != Kind::Program {
                    continue;
                }
                usage.calls += 1;
                if usage.calls > limits.calls {
                    self.problems.push(Problem::Budget {
                        what: "calls",
                        total: usage.calls,
                        bound: limits.calls,
                    });
                    break;
                }
                let Some(child) = self.resolved(step) else {
                    continue;
                };
                if let Some(start) = self.visiting.iter().position(|slug| slug == &child.slug) {
                    let path = self.visiting[start..]
                        .iter()
                        .cloned()
                        .chain([child.slug.clone()])
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    self.problems.push(Problem::Cyclic { path });
                    continue;
                }
                let declared = self.declared(step);
                let remaining = Budget {
                    depth: limits.depth.saturating_sub(usage.depth),
                    steps: limits.steps.saturating_sub(usage.steps),
                    calls: limits.calls.saturating_sub(usage.calls),
                };
                let mut grant = remaining;
                let mut widened = false;
                for (bound, want) in declared {
                    let have = remaining.of(bound);
                    if want > have {
                        self.problems.push(Problem::Widening {
                            step: step.name.clone(),
                            bound,
                            declared: want,
                            remaining: have,
                        });
                        widened = true;
                    } else {
                        grant.narrow(bound, want);
                    }
                }
                if widened {
                    continue;
                }
                if grant.depth == 0 {
                    self.problems.push(Problem::Depth {
                        step: step.name.clone(),
                        depth: self.visiting.len() as u64 + 1,
                        bound: self.visiting.len() as u64,
                    });
                    continue;
                }
                let spent = self.visit(child, &grant);
                usage.depth = usage.depth.max(1 + spent.depth);
                usage.steps += spent.steps;
                usage.calls += spent.calls;
                if usage.steps > limits.steps {
                    self.problems.push(Problem::Budget {
                        what: "steps",
                        total: usage.steps,
                        bound: limits.steps,
                    });
                    break;
                }
                if usage.calls > limits.calls {
                    self.problems.push(Problem::Budget {
                        what: "calls",
                        total: usage.calls,
                        bound: limits.calls,
                    });
                    break;
                }
            }
        }
        self.visiting.pop();
        usage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A `program` step's JSON, stating the propagation table every
    /// well-formed one carries.
    fn call_step(name: &str, address: &str, bounds: &str) -> String {
        call_step_with(name, address, bounds, "")
    }

    /// A `program` step's JSON with extra fields appended.
    fn call_step_with(name: &str, address: &str, bounds: &str, extra: &str) -> String {
        format!(
            r#"{{"name":"{name}","kind":"program","program":"{address}","propagation":{{"completed":"success","failed":"failure","refused":"refusal"}},"bounds":{bounds}{extra}}}"#
        )
    }

    /// A program file under `dir` with the steps given.
    fn stage(dir: &Path, slug: &str, steps: &str) {
        std::fs::write(
            dir.join(format!("{slug}.json")),
            format!(r#"{{"v":1,"slug":"{slug}","steps":[{steps}]}}"#),
        )
        .unwrap();
    }

    /// A program of one `program` step, naming `next` pinned by release.
    fn chain(dir: &Path, slug: &str, next: &str) {
        stage(
            dir,
            slug,
            &call_step("call", &format!("{next}@1.0.0"), "{}"),
        );
    }

    fn registry(dir: &Path) -> Registry {
        Registry::read(dir).expect("the staged programs all read")
    }

    #[test]
    fn a_program_that_reaches_itself_through_a_chain_is_refused_with_the_path() {
        let dir = tempfile::tempdir().unwrap();
        chain(dir.path(), "a", "b");
        chain(dir.path(), "b", "c");
        chain(dir.path(), "c", "a");
        let registry = registry(dir.path());
        assert!(registry.refused().is_empty(), "{:?}", registry.refused());

        let problems = Composition::check(registry.get("a").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Cyclic {
                path: "a -> b -> c -> a".to_string()
            }]
        );
    }

    #[test]
    fn a_composition_nested_past_the_depth_bound_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for i in 1..9 {
            chain(dir.path(), &format!("p{i}"), &format!("p{}", i + 1));
        }
        stage(
            dir.path(),
            "p9",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("p1").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Depth {
                step: "call".to_string(),
                depth: MAX_DEPTH + 1,
                bound: MAX_DEPTH,
            }]
        );
    }

    #[test]
    fn a_composition_with_more_steps_than_the_bound_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let steps = (0..=MAX_STEPS)
            .map(|i| format!(r#"{{"name":"s{i}","kind":"query","bounds":{{}}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        stage(dir.path(), "fat", &steps);
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("fat").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Budget {
                what: "steps",
                total: MAX_STEPS + 1,
                bound: MAX_STEPS,
            }]
        );
    }

    #[test]
    fn a_composition_with_more_calls_than_the_bound_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        let calls = (0..=MAX_CALLS)
            .map(|i| call_step(&format!("c{i}"), "leaf@1.0.0", "{}"))
            .collect::<Vec<_>>()
            .join(",");
        stage(dir.path(), "wide", &calls);
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("wide").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Budget {
                what: "calls",
                total: MAX_CALLS + 1,
                bound: MAX_CALLS,
            }]
        );
    }

    #[test]
    fn a_child_declaring_more_than_its_parent_has_left_is_a_widening_not_a_clamp() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        stage(
            dir.path(),
            "root",
            &call_step("call", "leaf@1.0.0", r#"{"depth":9,"steps":300}"#),
        );
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [
                Problem::Widening {
                    step: "call".to_string(),
                    bound: "depth",
                    declared: 9,
                    remaining: MAX_DEPTH - 1,
                },
                Problem::Widening {
                    step: "call".to_string(),
                    bound: "steps",
                    declared: 300,
                    remaining: MAX_STEPS - 1,
                },
            ]
        );
    }

    #[test]
    fn a_bare_name_resolves_and_reports_unpinned() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        stage(dir.path(), "root", &call_step("call", "leaf", "{}"));
        let registry = registry(dir.path());

        let resolution = ChildRef::parse("leaf")
            .unwrap()
            .resolve(&registry)
            .expect("a bare name still resolves");
        assert_eq!(
            resolution.pin,
            Pin::Unpinned {
                name: "leaf".to_string()
            },
            "resolve reports the pin rather than trusting the name silently"
        );

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Unpinned {
                step: "call".to_string(),
                name: "leaf".to_string(),
            }]
        );
    }

    #[test]
    fn a_step_consuming_an_output_no_earlier_step_produces_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "root",
            r#"{"name":"one","kind":"query","bounds":{}},
               {"name":"two","kind":"query","bounds":{},"binding":{
                   "inputs":{"work":{"from":"step","step":"one"},
                             "rest":{"from":"step","step":"ghost"}},
                   "produces":{"out":"text"},"exposes":["out"]}}"#,
        );
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [
                Problem::MissingProducer {
                    step: "two".to_string(),
                    input: "rest".to_string(),
                    producer: "ghost".to_string(),
                },
                Problem::MissingProducer {
                    step: "two".to_string(),
                    input: "work".to_string(),
                    producer: "one".to_string(),
                },
            ],
            "a step that exists earlier but declares no binding produces nothing, and neither does a step that is not there"
        );
    }

    #[test]
    fn the_propagation_table_is_honored_exactly_as_stated() {
        let step: Step = serde_json::from_str(
            r#"{"name":"call","kind":"program","program":"leaf@1.0.0",
                "propagation":{"completed":"success","failed":"refusal","refused":"success"},
                "bounds":{}}"#,
        )
        .unwrap();
        let table = Propagation::of(&step)
            .unwrap()
            .expect("the step states a table");
        assert_eq!(Outcome::Completed.propagation(&table), Effect::Success);
        assert_eq!(Outcome::Failed.propagation(&table), Effect::Refusal);
        assert_eq!(
            Outcome::Refused.propagation(&table),
            Effect::Success,
            "an unusual row is honored, not corrected"
        );

        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        stage(
            dir.path(),
            "root",
            r#"{"name":"call","kind":"program","program":"leaf@1.0.0","bounds":{}}"#,
        );
        let registry = registry(dir.path());
        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Propagation {
                step: "call".to_string()
            }],
            "a table never stated is a problem, never a default"
        );
    }

    #[test]
    fn executable_content_in_a_step_refuses_with_the_field_named() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "root",
            r#"{"name":"s1","kind":"query","command":"cargo test","bounds":{}},
               {"name":"s2","kind":"query","code":"fn main() {}","bounds":{}},
               {"name":"s3","kind":"query","wasm":"0061736d","bounds":{}}"#,
        );
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [
                Problem::Executable {
                    step: "s1".to_string(),
                    field: "command".to_string(),
                },
                Problem::Executable {
                    step: "s2".to_string(),
                    field: "code".to_string(),
                },
                Problem::Executable {
                    step: "s3".to_string(),
                    field: "wasm".to_string(),
                },
            ]
        );
    }

    #[test]
    fn a_valid_nested_composition_checks_clean() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        stage(
            dir.path(),
            "mid",
            &format!(
                r#"{{"name":"read","kind":"query","bounds":{{}},"binding":{{
                    "inputs":{{"topic":{{"from":"state","field":"topic"}}}},
                    "produces":{{"items":"list"}},"exposes":["items"]}}}},
                {}"#,
                call_step_with(
                    "call",
                    "leaf@1.0.0",
                    r#"{"depth":1,"steps":8,"calls":1}"#,
                    r#","binding":{"inputs":{"work":{"from":"step","step":"read","field":"items"}},"produces":{"summary":"text"},"exposes":["summary"]}"#,
                )
            ),
        );
        stage(
            dir.path(),
            "root",
            &format!(
                r#"{{"name":"select","kind":"query","bounds":{{}},"binding":{{
                    "inputs":{{"request":{{"from":"state","field":"request"}}}},
                    "produces":{{"items":"list","count":"number"}},"exposes":["items"]}}}},
                {}"#,
                call_step_with(
                    "call",
                    "mid@1.0.0",
                    r#"{"depth":2,"steps":20,"calls":2}"#,
                    r#","binding":{"inputs":{"work":{"from":"step","step":"select","field":"items"},"mode":{"from":"literal","value":"deep"}},"produces":{"answer":"text"},"exposes":["answer"]}"#,
                )
            ),
        );
        let registry = registry(dir.path());
        assert!(registry.refused().is_empty(), "{:?}", registry.refused());

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(problems, Vec::<Problem>::new());
    }

    #[test]
    fn a_reference_nothing_answers_is_unresolved() {
        let dir = tempfile::tempdir().unwrap();
        stage(dir.path(), "root", &call_step("call", "ghost@1.0.0", "{}"));
        let registry = registry(dir.path());

        let problems = Composition::check(registry.get("root").unwrap(), &registry);
        assert_eq!(
            problems,
            [Problem::Unresolved {
                step: "call".to_string(),
                reference: "ghost@1.0.0".to_string(),
            }]
        );
    }

    #[test]
    fn problems_are_reported_in_a_deterministic_order() {
        let dir = tempfile::tempdir().unwrap();
        stage(
            dir.path(),
            "leaf",
            r#"{"name":"one","kind":"query","bounds":{}}"#,
        );
        stage(
            dir.path(),
            "root",
            r#"{"name":"s1","kind":"query","command":"rm -rf .","bounds":{}},
               {"name":"s2","kind":"query","bounds":{},"binding":{
                   "inputs":{"work":{"from":"step","step":"ghost"}},
                   "produces":{},"exposes":[]}},
               {"name":"s3","kind":"program","program":"leaf","bounds":{}},
               {"name":"s4","kind":"program","program":"ghost@1.0.0","bounds":{},
                "propagation":{"completed":"success","failed":"failure","refused":"refusal"}}"#,
        );
        let registry = registry(dir.path());
        let root = registry.get("root").unwrap();

        let first = Composition::check(root, &registry);
        let second = Composition::check(root, &registry);
        assert_eq!(first, second);
        assert_eq!(
            first,
            [
                Problem::Executable {
                    step: "s1".to_string(),
                    field: "command".to_string(),
                },
                Problem::MissingProducer {
                    step: "s2".to_string(),
                    input: "work".to_string(),
                    producer: "ghost".to_string(),
                },
                Problem::Propagation {
                    step: "s3".to_string(),
                },
                Problem::Unpinned {
                    step: "s3".to_string(),
                    name: "leaf".to_string(),
                },
                Problem::Unresolved {
                    step: "s4".to_string(),
                    reference: "ghost@1.0.0".to_string(),
                },
            ],
            "step problems in step order, then the descent, every run the same"
        );
    }
}
