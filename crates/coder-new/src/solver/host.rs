//! A small NIP-PRG program host: it reads a pinned program document,
//! walks its steps in array order, and hands each step's effect to an
//! [`Effects`] implementation. The host owns the graph semantics, which
//! are the part another operator's host has to agree with:
//!
//! - `after` names earlier steps; a dependency that failed with
//!   `on_error: stop` blocks the step, recorded `cancelled`.
//! - `when` is `{source, equals}` or `{source, present: true}`. A source
//!   that does not resolve makes the condition false, and a false
//!   condition records `skipped`, never an invocation.
//! - A binding is `literal`, `from` (`input` or `step:<name>` with a JSON
//!   pointer into the step's envelope), `object`, or `array`. A step's
//!   `/value` resolves only when the step completed; the whole envelope
//!   (`pointer: ""`) always resolves, so a later step can read a skipped
//!   step's outcome. A missing input refuses the step and ends the run.
//! - The binding's `requires` refuses a step while a named step's value is
//!   empty, as `crates/coder`'s runtime does.
//! - A `decide` step writes its typed decision as its value and the raw
//!   answers at `<name>$answers`.
//!
//! The host refuses what it does not run (`each`, a `retry` of more than
//! one attempt, and `query`, `program`, and `module` steps) rather than
//! skipping it, because a program whose steps were skipped is a different
//! program.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The step kinds this host dispatches.
const RUNS: &[&str] = &["delegate", "check", "invoke", "decide"];

/// A program document whose bytes are pinned by digest.
#[derive(Clone, Debug)]
pub struct Pinned {
    /// `sha256:<hex>` of the document's bytes.
    pub digest: String,
    /// The NIP-PRG definition, as written.
    pub definition: Value,
    /// The definition projected through the host binding.
    pub program: coder::program::Program,
}

impl Pinned {
    /// Reads and admits a program file, holding it to `pin` when given.
    ///
    /// # Errors
    /// The file is unreadable, its digest is not `pin`, or [`Pinned::parse`]
    /// refuses it.
    pub fn load(path: &Path, pin: Option<&str>) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("cannot read program {}: {e}", path.display()))?;
        let pinned = Self::parse(&bytes)?;
        if let Some(pin) = pin {
            if pinned.digest != pin {
                return Err(format!(
                    "{} is {}, not the pinned {pin}",
                    path.display(),
                    pinned.digest
                ));
            }
        }
        Ok(pinned)
    }

    /// Admits a program document: a valid NIP-PRG definition, a binding
    /// the coder registry accepts, and no feature this host does not run.
    ///
    /// # Errors
    /// The first reason the document is not one this host runs.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let doc: Value =
            serde_json::from_slice(bytes).map_err(|e| format!("not a program: {e}"))?;
        let definition = doc
            .get("definition")
            .cloned()
            .ok_or("the document has no definition")?;
        nostr::prg::parse_definition(&definition).map_err(|e| e.to_string())?;
        let program = coder::program::Program::parse(bytes)?;
        for step in definition["steps"].as_array().into_iter().flatten() {
            let name = step["name"].as_str().unwrap_or_default();
            let kind = step["kind"].as_str().unwrap_or_default();
            if !RUNS.contains(&kind) {
                return Err(format!("step {name}: this host does not run a {kind} step"));
            }
            if step.get("each").is_some() {
                return Err(format!("step {name}: this host does not run each"));
            }
            if step["retry"]["max_attempts"]
                .as_u64()
                .is_some_and(|n| n > 1)
            {
                return Err(format!("step {name}: this host does not retry"));
            }
        }
        Ok(Self {
            digest,
            definition,
            program,
        })
    }
}

/// Where a step's effect happens. A live host hands these to the hands
/// and the harness; a replay answers them from a recording.
pub trait Effects {
    /// A `delegate` step: `executor` does bounded work on `input`.
    ///
    /// # Errors
    /// The delegation failed.
    fn delegate(&mut self, step: &str, executor: &str, input: &Value) -> Result<Value, String>;
    /// A `check` step: the registered deterministic `checker` judges `input`.
    ///
    /// # Errors
    /// The checker could not run.
    fn check(&mut self, step: &str, checker: &str, input: &Value) -> Result<Value, String>;
    /// An `invoke` step: the admitted host `operation` runs on `input`.
    ///
    /// # Errors
    /// The operation failed.
    fn invoke(&mut self, step: &str, operation: &str, input: &Value) -> Result<Value, String>;
    /// A `decide` step: `question` goes to the configured door. Returns the
    /// typed decision and the raw answers.
    ///
    /// # Errors
    /// The door gave no usable answer.
    fn decide(
        &mut self,
        step: &str,
        question: &str,
        input: &Value,
    ) -> Result<(Value, Value), String>;
}

/// One step the host reached, in program order.
#[derive(Clone, Debug)]
pub struct Reached {
    pub step: String,
    pub kind: String,
    /// The executor, checker, operation, or question the step named.
    pub target: String,
    /// The step envelope: `outcome`, `value`, `receipt`, `verification`,
    /// `integration`.
    pub envelope: Value,
    /// Why the step did not complete, when it did not.
    pub why: Option<String>,
}

impl Reached {
    #[must_use]
    pub fn outcome(&self) -> &str {
        self.envelope["outcome"].as_str().unwrap_or_default()
    }

    /// Whether the step's effect was dispatched.
    #[must_use]
    pub fn dispatched(&self) -> bool {
        matches!(self.outcome(), "completed" | "failed")
    }
}

/// What one run of a program did.
#[derive(Clone, Debug, Default)]
pub struct Ran {
    pub steps: Vec<Reached>,
    /// Host-owned state keys, such as `done$answers`.
    pub sidecars: BTreeMap<String, Value>,
    /// The definition's `result`, when it resolved.
    pub result: Option<Value>,
    /// Why the run ended early, when it did.
    pub stopped: Option<String>,
}

impl Ran {
    #[must_use]
    pub fn envelope(&self, step: &str) -> Option<&Value> {
        self.steps
            .iter()
            .find(|reached| reached.step == step)
            .map(|reached| &reached.envelope)
    }
}

fn envelope(outcome: &str, value: Value) -> Value {
    json!({
        "outcome": outcome,
        "value": value,
        "receipt": null,
        "verification": "not_run",
        "integration": "not_requested",
    })
}

/// Resolves a binding against the run's input and the envelopes so far.
/// `None` is a missing value.
#[must_use]
pub fn resolve(binding: &Value, input: &Value, state: &BTreeMap<String, Value>) -> Option<Value> {
    if let Some(literal) = binding.get("literal") {
        return Some(literal.clone());
    }
    if let Some(from) = binding.get("from").and_then(Value::as_str) {
        let pointer = binding["pointer"].as_str().unwrap_or_default();
        if from == "input" {
            return input.pointer(pointer).cloned();
        }
        let envelope = state.get(from.strip_prefix("step:")?)?;
        if pointer.starts_with("/value") && envelope["outcome"] != "completed" {
            return None;
        }
        return envelope.pointer(pointer).cloned();
    }
    if let Some(object) = binding.get("object").and_then(Value::as_object) {
        let mut out = Map::new();
        for (key, field) in object {
            out.insert(key.clone(), resolve(field, input, state)?);
        }
        return Some(Value::Object(out));
    }
    if let Some(array) = binding.get("array").and_then(Value::as_array) {
        return array
            .iter()
            .map(|item| resolve(item, input, state))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array);
    }
    None
}

/// Whether a `when` condition holds.
#[must_use]
pub fn holds(when: &Value, input: &Value, state: &BTreeMap<String, Value>) -> bool {
    let resolved = resolve(&when["source"], input, state);
    match when.get("equals") {
        Some(expected) => resolved.as_ref() == Some(expected),
        None => resolved.is_some(),
    }
}

fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        _ => false,
    }
}

/// Runs `pinned` on `input`, each effect through `effects`.
pub fn run(pinned: &Pinned, input: &Value, effects: &mut dyn Effects) -> Ran {
    let mut ran = Ran::default();
    let mut state: BTreeMap<String, Value> = BTreeMap::new();
    let mut on_error: BTreeMap<String, String> = BTreeMap::new();
    let raw = pinned.definition["steps"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (step, host) in raw.iter().zip(&pinned.program.steps) {
        let name = host.name.clone();
        let kind = step["kind"].as_str().unwrap_or_default().to_owned();
        let target = match kind.as_str() {
            "delegate" => host.rest.get("executor").and_then(Value::as_str),
            "invoke" => host.rest.get("operation").and_then(Value::as_str),
            "decide" => host.question.as_deref(),
            _ => None,
        }
        .map(str::to_owned)
        .unwrap_or_else(|| {
            step["target"]["id"]
                .as_str()
                .and_then(|id| id.rsplit('/').next())
                .unwrap_or_default()
                .to_owned()
        });
        on_error.insert(
            name.clone(),
            step["on_error"].as_str().unwrap_or("stop").to_owned(),
        );
        let reach = |outcome: &str,
                     value: Value,
                     why: Option<String>,
                     ran: &mut Ran,
                     state: &mut BTreeMap<String, Value>| {
            let env = envelope(outcome, value);
            state.insert(name.clone(), env.clone());
            ran.steps.push(Reached {
                step: name.clone(),
                kind: kind.clone(),
                target: target.clone(),
                envelope: env,
                why,
            });
        };
        if ran.stopped.is_some() {
            reach("cancelled", Value::Null, None, &mut ran, &mut state);
            continue;
        }
        let blocked = step["after"].as_array().into_iter().flatten().find(|dep| {
            let dep = dep.as_str().unwrap_or_default();
            let failed = state.get(dep).is_some_and(|env| {
                matches!(
                    env["outcome"].as_str(),
                    Some("failed" | "refused" | "cancelled" | "unknown")
                )
            });
            failed && on_error.get(dep).map(String::as_str) == Some("stop")
        });
        if let Some(dep) = blocked {
            let why = format!("{} did not complete", dep.as_str().unwrap_or_default());
            reach("cancelled", Value::Null, Some(why), &mut ran, &mut state);
            continue;
        }
        if let Some(when) = step.get("when") {
            if !holds(when, input, &state) {
                reach("skipped", Value::Null, None, &mut ran, &mut state);
                continue;
            }
        }
        if let Some(missing) = host.requires.iter().find(|required| {
            state
                .get(required.as_str())
                .is_none_or(|env| empty(&env["value"]))
        }) {
            let why = format!("{name} requires {missing}, whose output is empty");
            reach(
                "refused",
                Value::Null,
                Some(why.clone()),
                &mut ran,
                &mut state,
            );
            ran.stopped = Some(why);
            continue;
        }
        let Some(arguments) = resolve(&step["input"], input, &state) else {
            let why = format!("{name}'s input does not resolve");
            reach(
                "refused",
                Value::Null,
                Some(why.clone()),
                &mut ran,
                &mut state,
            );
            ran.stopped = Some(why);
            continue;
        };
        let answered = match kind.as_str() {
            "delegate" => effects.delegate(&name, &target, &arguments),
            "check" => effects.check(&name, &target, &arguments),
            "invoke" => effects.invoke(&name, &target, &arguments),
            "decide" => effects
                .decide(&name, &target, &arguments)
                .map(|(decision, answers)| {
                    ran.sidecars.insert(format!("{name}$answers"), answers);
                    decision
                }),
            other => Err(format!("this host does not run a {other} step")),
        };
        match answered {
            Ok(value) => reach("completed", value, None, &mut ran, &mut state),
            Err(why) => reach("failed", Value::Null, Some(why), &mut ran, &mut state),
        }
    }
    ran.result = resolve(&pinned.definition["result"], input, &state);
    ran
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn artifact(media: &str) -> Value {
        json!({
            "digest": "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0",
            "size": 17,
            "media_type": media,
        })
    }

    fn step(name: &str, kind: &str, after: &[&str], input: Value) -> Value {
        json!({
            "name": name,
            "kind": kind,
            "target": {"id": format!("{A}:openagents/{name}"), "artifact": artifact("application/octet-stream")},
            "after": after,
            "input": input,
            "output": artifact("application/schema+json"),
            "bounds": {},
            "on_error": "stop",
        })
    }

    fn document(steps: Vec<Value>, binding: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "definition": {
                "v": 1,
                "requires": [],
                "id": format!("{A}:openagents/tiny"),
                "summary": "A tiny program.",
                "input": artifact("application/schema+json"),
                "output": artifact("application/schema+json"),
                "steps": steps,
                "result": {"from": "step:ask", "pointer": "/value"},
                "bounds": {},
            },
            "binding": {"name": "Tiny", "steps": binding},
        }))
        .unwrap()
    }

    struct Echo(Vec<String>);

    impl Effects for Echo {
        fn delegate(&mut self, step: &str, executor: &str, input: &Value) -> Result<Value, String> {
            self.0.push(format!("{step}:{executor}"));
            if executor == "breaks" {
                return Err("broke".into());
            }
            Ok(json!({"ok": input.clone()}))
        }
        fn check(&mut self, step: &str, _: &str, _: &Value) -> Result<Value, String> {
            self.0.push(step.to_owned());
            Ok(json!({"status": "failed"}))
        }
        fn invoke(&mut self, step: &str, _: &str, _: &Value) -> Result<Value, String> {
            self.0.push(step.to_owned());
            Ok(json!({}))
        }
        fn decide(&mut self, step: &str, _: &str, _: &Value) -> Result<(Value, Value), String> {
            self.0.push(step.to_owned());
            Ok((json!("yes"), json!([{"answer": "yes", "p": 0.9}])))
        }
    }

    #[test]
    fn a_false_condition_skips_and_a_decide_writes_its_answers_beside_its_value() {
        let bytes = document(
            vec![
                step(
                    "work",
                    "delegate",
                    &[],
                    json!({"from": "input", "pointer": "/issue"}),
                ),
                step(
                    "test",
                    "check",
                    &["work"],
                    json!({"from": "step:work", "pointer": "/value"}),
                ),
                json!({
                    "when": {"source": {"from": "step:test", "pointer": "/value/status"}, "equals": "passed"},
                    "name": "ship", "kind": "invoke",
                    "target": {"id": format!("{A}:openagents/ship"), "artifact": artifact("application/octet-stream")},
                    "after": ["test"], "input": {"literal": 1},
                    "output": artifact("application/schema+json"), "bounds": {}, "on_error": "stop",
                }),
                step(
                    "ask",
                    "decide",
                    &["ship"],
                    json!({"from": "step:ship", "pointer": ""}),
                ),
            ],
            json!({
                "work": {"executor": "worker"},
                "ship": {"operation": "ship"},
                "ask": {"question": "openagents.completion.v1"},
            }),
        );
        let pinned = Pinned::parse(&bytes).unwrap();
        let mut echo = Echo(Vec::new());
        let ran = run(&pinned, &json!({"issue": 7}), &mut echo);
        assert_eq!(echo.0, ["work:worker", "test", "ask"]);
        assert_eq!(ran.envelope("ship").unwrap()["outcome"], "skipped");
        assert_eq!(ran.envelope("work").unwrap()["value"], json!({"ok": 7}));
        assert_eq!(ran.result, Some(json!("yes")));
        assert_eq!(ran.sidecars["ask$answers"][0]["p"], 0.9);
        for reached in &ran.steps {
            nostr::prg::parse_envelope(&reached.envelope).unwrap();
        }
    }

    #[test]
    fn a_failed_dependency_blocks_and_requires_refuses_on_an_empty_output() {
        let bytes = document(
            vec![
                step("work", "delegate", &[], json!({"literal": 1})),
                step("ask", "decide", &["work"], json!({"literal": 1})),
            ],
            json!({
                "work": {"executor": "breaks"},
                "ask": {"question": "openagents.completion.v1"},
            }),
        );
        let pinned = Pinned::parse(&bytes).unwrap();
        let ran = run(&pinned, &json!({}), &mut Echo(Vec::new()));
        assert_eq!(ran.envelope("work").unwrap()["outcome"], "failed");
        assert_eq!(ran.envelope("ask").unwrap()["outcome"], "cancelled");

        let bytes = document(
            vec![
                step("work", "invoke", &[], json!({"literal": 1})),
                step("ask", "decide", &["work"], json!({"literal": 1})),
            ],
            json!({
                "work": {"operation": "noop"},
                "ask": {"question": "openagents.completion.v1", "requires": ["work"]},
            }),
        );
        let pinned = Pinned::parse(&bytes).unwrap();
        let ran = run(&pinned, &json!({}), &mut Echo(Vec::new()));
        assert_eq!(ran.envelope("ask").unwrap()["outcome"], "refused");
        assert!(ran.stopped.unwrap().contains("requires work"));
    }

    #[test]
    fn the_host_refuses_a_step_kind_it_does_not_run() {
        let bytes = document(
            vec![
                step("work", "query", &[], json!({"literal": 1})),
                step("ask", "decide", &["work"], json!({"literal": 1})),
            ],
            json!({"ask": {"question": "openagents.completion.v1"}}),
        );
        let error = Pinned::parse(&bytes).unwrap_err();
        assert!(error.contains("does not run a query step"), "{error}");
    }
}
