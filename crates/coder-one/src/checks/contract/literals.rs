//! Literal output obligations, extracted without a model and checked by metadata.
//!
//! This opt-in component has its own schema. A matched requirement never means
//! that the task passes; only an observed contradiction produces a failure call.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Outcome;
use super::extract::{Block, Unit, as_path, is_output, segment};
use super::host::{Host, Stat};

pub const PLAN_SCHEMA: &str = "openagents.coder-one.literal-artifact-plan.v1";
pub const REPORT_SCHEMA: &str = "openagents.coder-one.literal-artifact-report.v1";

/// A necessary condition stated by the task, not a sufficient success test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Requirement {
    Exists,
    /// An inclusive byte limit, normalized from the quoted requirement.
    MaxBytes {
        maximum: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub id: String,
    pub path: String,
    /// The exact source sentence or list item.
    pub span: String,
    /// The output declaration supporting a size requirement.
    pub output_span: String,
    pub requirement: Requirement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub task: String,
    pub workdir: String,
    pub instruction_sha256: String,
    pub obligations: Vec<Obligation>,
    pub digest: String,
}

impl Plan {
    fn computed_digest(&self) -> String {
        let mut copy = self.clone();
        copy.digest.clear();
        atif::digest(&json!(copy))
    }

    /// Refuse an altered plan before inspecting any candidate path.
    ///
    /// # Errors
    /// Returns a reason for a different schema, digest, or unsupported path.
    pub fn verify(&self) -> Result<(), String> {
        if self.schema != PLAN_SCHEMA || self.digest != self.computed_digest() {
            return Err("literal artifact plan schema or digest differs".into());
        }
        if !normalized_absolute(&self.workdir) {
            return Err("literal artifact workdir must be an absolute normalized path".into());
        }
        let mut ids = BTreeSet::new();
        for item in &self.obligations {
            if !inside(&item.path, &self.workdir)
                || item.id.is_empty()
                || !ids.insert(&item.id)
                || item.span.is_empty()
                || item.output_span.is_empty()
            {
                return Err(
                    "literal artifact obligation has an unsupported identity or path".into(),
                );
            }
        }
        Ok(())
    }
}

fn normalized_absolute(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("//")
        && !path.split('/').any(|s| matches!(s, "." | ".."))
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

fn inside(path: &str, workdir: &str) -> bool {
    normalized_absolute(path)
        && Path::new(path) != Path::new(workdir)
        && Path::new(path).starts_with(workdir)
}

fn uncertain(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphabetic() && c != '\'')
        .any(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "if" | "unless"
                    | "either"
                    | "or"
                    | "optional"
                    | "optionally"
                    | "may"
                    | "might"
                    | "can"
                    | "could"
                    | "example"
                    | "examples"
                    | "temporary"
                    | "intermediate"
                    | "transient"
                    | "not"
                    | "never"
                    | "don't"
                    | "no"
            )
        })
}

fn path_pattern() -> Regex {
    Regex::new(r"(?:/[A-Za-z0-9_.+\-]+)+/?|(?:\./)?[A-Za-z0-9_+\-]+(?:/[A-Za-z0-9_.+\-]+)*\.[A-Za-z][A-Za-z0-9]{0,11}")
        .expect("literal path pattern")
}

fn occurrences(text: &str, workdir: &str) -> Vec<(usize, usize, String)> {
    path_occurrences(text, workdir)
        .into_iter()
        .filter(|(_, _, path)| inside(path, workdir))
        .collect()
}

fn path_occurrences(text: &str, workdir: &str) -> Vec<(usize, usize, String)> {
    path_pattern()
        .find_iter(text)
        .filter_map(|m| {
            // Do not read part of a URL, placeholder, option, or escaped name.
            let before = text[..m.start()].chars().next_back();
            if before.is_some_and(|c| !c.is_whitespace() && !matches!(c, '`' | '\'' | '"' | '(')) {
                return None;
            }
            let raw = m.as_str().trim_end_matches('.');
            let end = m.start() + raw.len();
            if text[end..].chars().next().is_some_and(|c| {
                !c.is_whitespace() && !matches!(c, '`' | '\'' | '"' | ')' | ',' | ';' | ':' | '.')
            }) {
                return None;
            }
            let path = as_path(raw, workdir)?;
            normalized_absolute(&path).then_some((m.start(), end, path))
        })
        .collect()
}

fn maximum(after: &str) -> Option<u64> {
    let after = after.trim_start_matches(['`', '\'', '"']).trim();
    let pattern =
        Regex::new(r"(?i)^must be (at most|less than) ([0-9][0-9,]*) bytes(?:[.;,]|\s|$)")
            .expect("literal byte limit pattern");
    let captures = pattern.captures(after)?;
    let number = &captures[2];
    if number.contains(',') {
        let parts: Vec<_> = number.split(',').collect();
        if !(1..=3).contains(&parts[0].len()) || parts[1..].iter().any(|p| p.len() != 3) {
            return None;
        }
    }
    let count: u64 = number.replace(',', "").parse().ok()?;
    if captures[1].eq_ignore_ascii_case("less than") {
        count.checked_sub(1)
    } else {
        Some(count)
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn retire(text: &str, workdir: &str, active: &BTreeSet<String>, retired: &mut BTreeSet<String>) {
    let tokens = words(text);
    let removes = tokens
        .iter()
        .any(|w| matches!(w.as_str(), "delete" | "remove" | "unlink" | "rm" | "rmdir"));
    let moves = tokens
        .iter()
        .any(|w| matches!(w.as_str(), "move" | "rename" | "mv"));
    if !removes && !moves {
        return;
    }
    let found = path_occurrences(text, workdir);
    // A cleanup instruction without a named path cannot identify which of the
    // earlier outputs remains. Abstain instead of resolving anaphora by guess.
    let Some((start, _, first)) = found.first() else {
        retired.extend(active.iter().cloned());
        return;
    };
    if !removes && moves {
        let prefix = words(&text[..*start]);
        if prefix
            .last()
            .is_some_and(|w| matches!(w.as_str(), "to" | "into"))
        {
            retired.extend(
                active
                    .iter()
                    .filter(|path| !Path::new(path).starts_with(first))
                    .cloned(),
            );
        } else {
            retired.extend(
                active
                    .iter()
                    .filter(|path| Path::new(path).starts_with(first))
                    .cloned(),
            );
        }
    } else {
        retired.extend(
            active
                .iter()
                .filter(|path| found.iter().any(|(_, _, p)| Path::new(path).starts_with(p)))
                .cloned(),
        );
    }
}

fn retired_paths(
    units: &[Unit],
    blocks: &[Block],
    outputs: &BTreeMap<String, String>,
    workdir: &str,
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    let mut retired = BTreeSet::new();
    for (index, unit) in units.iter().enumerate() {
        active.extend(
            outputs
                .iter()
                .filter(|(_, span)| **span == unit.text)
                .map(|(path, _)| path.clone()),
        );
        retire(&unit.text, workdir, &active, &mut retired);
        for block in blocks.iter().filter(|b| b.lead == Some(index)) {
            for line in block.body.lines() {
                retire(line, workdir, &active, &mut retired);
            }
        }
    }
    retired
}

/// Extract only unconditional, explicit outputs and their literal byte limits.
/// Unsupported wording stays outside the plan; no missing item means success.
#[must_use]
pub fn plan(task: &str, instruction: &str, workdir: &str) -> Plan {
    let (units, blocks) = segment(instruction);
    let headings: BTreeSet<_> = instruction
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.starts_with('#')
                .then(|| line.trim_start_matches('#').trim())
        })
        .collect();
    let mut outputs = BTreeMap::<String, String>::new();
    let mut obligations = Vec::new();
    let mut context = String::new();
    let declaration = Regex::new(r"(?i)^(?:please |you must |you shall )?(?:write|create|save|produce|generate|output|emit|deliver|store)\b")
        .expect("literal output declaration pattern");
    let deferred = Regex::new(r"(?i)\b(?:to|for)\s+(?:write|writ(e|ing)|creat(e|ing)|sav(e|ing)|generat(e|ing)|produc(e|ing)|output|print|emit|store)\b")
        .expect("deferred output action pattern");
    for unit in &units {
        if headings.contains(unit.text.as_str()) {
            context.clear();
        }
        // A conditional lead-in also qualifies its following list items.
        let blocked = uncertain(&unit.text) || uncertain(&context);
        if unit.text.ends_with(':') || headings.contains(unit.text.as_str()) {
            context.push_str(&unit.text);
            context.push(' ');
        }
        if !normalized_absolute(workdir) {
            continue;
        }
        let found = occurrences(&unit.text, workdir);
        let mut previous_end = 0;
        for (start, end, path) in &found {
            let before = &unit.text[previous_end..*start];
            previous_end = *end;
            // Quotes delimit names, not context; keep the nearest input/output cue.
            let before = before.trim_end_matches(['`', '\'', '"']);
            let indirect = before.split(|c: char| !c.is_ascii_alphabetic()).any(|s| {
                matches!(
                    s.to_ascii_lowercase().as_str(),
                    "that"
                        | "which"
                        | "when"
                        | "after"
                        | "before"
                        | "once"
                        | "mention"
                        | "mentioning"
                        | "describe"
                        | "describing"
                        | "reference"
                        | "referencing"
                        | "explain"
                        | "command"
                        | "commands"
                        | "snippet"
                        | "instructions"
                        | "documentation"
                        | "contents"
                        | "function"
                        | "method"
                )
            });
            if !blocked
                && declaration.is_match(&unit.text)
                && !indirect
                && !deferred.is_match(before)
                && is_output(before) == Some(true)
                && !outputs.contains_key(path)
            {
                outputs.insert(path.clone(), unit.text.clone());
                obligations.push(Obligation {
                    id: String::new(),
                    path: path.clone(),
                    span: unit.text.clone(),
                    output_span: unit.text.clone(),
                    requirement: Requirement::Exists,
                });
            }
        }
        for (start, end, path) in found {
            // An optional method followed by an explicit mandatory bound is
            // different from an optional output. The output must already be
            // declared independently. Conditional lead-ins still block it.
            let mandatory_clause = unit.text[..start].rfind(", but ").is_some_and(|at| {
                !uncertain(&context)
                    && !unit.text[..at]
                        .split_whitespace()
                        .any(|s| matches!(s.to_ascii_lowercase().as_str(), "if" | "unless"))
                    && !uncertain(&unit.text[at + 6..])
            });
            if blocked && !mandatory_clause {
                continue;
            }
            if let (Some(output_span), Some(maximum)) =
                (outputs.get(&path), maximum(&unit.text[end..]))
            {
                obligations.push(Obligation {
                    id: String::new(),
                    path,
                    span: unit.text.clone(),
                    output_span: output_span.clone(),
                    requirement: Requirement::MaxBytes { maximum },
                });
            }
        }
    }
    let retired = retired_paths(&units, &blocks, &outputs, workdir);
    obligations.retain(|item| !retired.contains(&item.path));
    for (i, item) in obligations.iter_mut().enumerate() {
        item.id = format!("L{}", i + 1);
    }
    let mut plan = Plan {
        schema: PLAN_SCHEMA.into(),
        task: task.into(),
        workdir: workdir.into(),
        instruction_sha256: crate::accept::sha256(instruction.as_bytes()),
        obligations,
        digest: String::new(),
    };
    plan.digest = plan.computed_digest();
    plan
}

/// Check sealed obligations without running a command or asking a model.
///
/// # Errors
/// Refuses an altered or unsupported plan before any host access.
pub async fn run(plan: &Plan, candidate: &str, host: &impl Host) -> Result<Value, String> {
    plan.verify()?;
    let mut results = Vec::new();
    let mut failed = false;
    for item in &plan.obligations {
        let observation = host.stat(&item.path).await;
        let outcome = match (&item.requirement, observation) {
            (_, Err(why)) => Outcome::CouldNotRun { why },
            (Requirement::Exists, Ok(Stat::Missing)) => Outcome::Differed {
                diff: format!("required output is missing: {}", item.path),
                similarity: None,
            },
            (Requirement::Exists, Ok(stat)) => Outcome::Matched {
                observed: format!("required path exists: {stat:?}"),
            },
            (Requirement::MaxBytes { maximum }, Ok(Stat::File(bytes))) if bytes > *maximum => {
                Outcome::Differed {
                    diff: format!("{} has {bytes} bytes; maximum is {maximum}", item.path),
                    similarity: None,
                }
            }
            (Requirement::MaxBytes { maximum }, Ok(Stat::File(bytes))) => Outcome::Matched {
                observed: format!("{bytes} bytes is at most {maximum}"),
            },
            (Requirement::MaxBytes { .. }, Ok(_)) => Outcome::CouldNotRun {
                why: format!("no regular file at {} to measure", item.path),
            },
        };
        failed |= matches!(outcome, Outcome::Differed { .. });
        results.push(json!({"id": item.id, "path": item.path, "outcome": outcome}));
    }
    Ok(json!({
        "schema": REPORT_SCHEMA, "task": plan.task, "plan": plan.digest,
        "candidate": candidate, "call": if failed {Some("fail")} else {None},
        "scope": "necessary artifact obligations only; matched items never certify task completion",
        "items": results,
    }))
}

#[cfg(test)]
mod tests;
