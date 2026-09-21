//! Programs: the steps a run takes, and what bounds them.
//!
//! A program is a state machine of named steps with per-step bounds,
//! defined by [NIP-PRG](../../../nips/openagents/NIP-PRG.md) as
//! `kind:30182`. It carries no code, no commands, and no prompts, which is
//! what makes one safe to read from a stranger: the worst a hostile program
//! can do is describe a shape this host declines.
//!
//! This module reads programs from local files and lists what it found.
//! Nothing here runs a step.
//!
//! # The query is recorded beside the answer
//!
//! A registry read records the Nostr filter it would have sent even though
//! the answer came off disk, because the query is the part that has to keep
//! working when the answer does not. The filter is built with the same
//! [`nostr::domain::Filter`] a relay read will use, so a local answer and a
//! relay answer are the same question asked twice rather than two questions
//! that resemble each other.
//!
//! # Refusing beats skipping
//!
//! A host that does not recognize a step kind refuses the whole program.
//! [`Registry::read`] leaves such a program out of the listing and names it
//! among [`Registry::refused`], because a program whose unknown steps are
//! skipped is a different program — and it is the one a host would run by
//! accident.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use atif::{Call, Outcome};
use nostr::domain::Filter;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::capability::is_slug;

/// The event kind a program publishes as, per NIP-PRG.
pub const PROGRAM_KIND: u16 = 30182;

/// The program body version this reads.
pub const PROGRAM_VERSION: u32 = 1;

/// The name a registry read records itself under in a trace.
pub const REGISTRY_CALL: &str = "program_registry";

/// The variable that moves the program directory.
pub const DIR_ENV: &str = "CODER_PROGRAM_DIR";

/// The fields a `decide` step must not carry.
///
/// A `decide` step names a question and must not carry its wording.
/// Question text belongs to a separately digested question set, because
/// rewording a question changes what was asked and two runs of "the same"
/// program would stop being comparable.
const QUESTION_TEXT: &[&str] = &["instructions", "criteria", "questions", "text", "prompt"];

/// One program: named steps, each with bounds.
///
/// `slug` and `name` are the `d` and `name` tags the published
/// `kind:30182` carries, and the distinct step kinds are its `step` tags.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Program {
    pub v: u32,
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    pub steps: Vec<Step>,
}

/// One step. Its output is addressable by its name.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Step {
    pub name: String,
    pub kind: Kind,
    /// The question identifier a `decide` step puts to a decision model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// The source a `query` step looks its work up from, by slug. A
    /// program names a source and never a command: a step carrying an
    /// argv would be code, and a program carries none. A `query` step
    /// naming no source reads the work the request carried, which
    /// [`crate::source::REQUEST`] is the name of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The address of the child program a `program` step runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The module a `module` step runs, named by content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<Value>,
    /// What bounds the step. Every step carries bounds, and a host refuses
    /// a step whose bounds it cannot enforce rather than running it
    /// unbounded.
    #[serde(default)]
    pub bounds: Map<String, Value>,
    /// Anything else the step declared. Kept rather than dropped so a host
    /// can refuse what it must and ignore what it may.
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// What a step does.
///
/// Deserializing an unrecognized kind fails, which refuses the program that
/// carried it. The registry is open and a future NIP may define more; a
/// host that meets one refuses rather than guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A structured lookup. Deterministic given its inputs.
    Query,
    /// A deterministic admission test. Passes or refuses.
    Check,
    /// A typed question put to a decision model.
    Decide,
    /// Work handed to an executor.
    Delegate,
    /// Another program, by address.
    Program,
    /// A WebAssembly module, by content hash.
    Module,
}

impl Kind {
    /// The kind's name, as a `step` tag spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Query => "query",
            Kind::Check => "check",
            Kind::Decide => "decide",
            Kind::Delegate => "delegate",
            Kind::Program => "program",
            Kind::Module => "module",
        }
    }
}

impl Program {
    /// Reads a program from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a program this host
    /// runs: unreadable, unparseable, a `v` it does not know, an
    /// unrecognized step kind, a repeated step name, a `decide` step with
    /// no question or with a question's wording inlined.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let program: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        program
            .validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(program)
    }

    /// Whether this program is one this host would run.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != PROGRAM_VERSION {
            return Err(format!(
                "body version is {}, this version reads {PROGRAM_VERSION}",
                self.v
            ));
        }
        if !is_slug(&self.slug) {
            return Err(format!("slug {:?} is not a program slug", self.slug));
        }
        // The selection question answers `none` when a request asks for no
        // program, so a program under that slug could be chosen and never
        // reached. Refusing the file is better than resolving a program
        // nothing can select.
        if self.slug == crate::runtime::NO_PROGRAM {
            return Err(format!(
                "slug {:?} is the answer that means no program, so a program under it could never be selected",
                self.slug
            ));
        }
        if self.steps.is_empty() {
            return Err("a program with no steps describes no work".to_string());
        }
        let mut seen = Vec::new();
        for step in &self.steps {
            if seen.contains(&step.name) {
                return Err(format!("step name {:?} is used twice", step.name));
            }
            seen.push(step.name.clone());
            if step.kind == Kind::Decide {
                if step.question.is_none() {
                    return Err(format!("decide step {:?} names no question", step.name));
                }
                if let Some(field) = QUESTION_TEXT
                    .iter()
                    .find(|field| step.rest.contains_key(**field))
                {
                    return Err(format!(
                        "decide step {:?} carries {field}, which is a question's wording and belongs to a question set",
                        step.name
                    ));
                }
            }
            match (&step.source, step.kind) {
                (Some(source), Kind::Query) if !is_slug(source) => {
                    return Err(format!(
                        "query step {:?} names {source:?}, which is not a source slug",
                        step.name
                    ));
                }
                (Some(source), kind) if kind != Kind::Query => {
                    return Err(format!(
                        "step {:?} is a {} step and names the source {source:?}, which only a query step reads",
                        step.name,
                        kind.word()
                    ));
                }
                _ => {}
            }
            if step.kind == Kind::Program && step.program.is_none() {
                return Err(format!("program step {:?} names no program", step.name));
            }
            if step.kind == Kind::Module && step.module.is_none() {
                return Err(format!("module step {:?} names no module", step.name));
            }
        }
        Ok(())
    }

    /// The step names, in order.
    #[must_use]
    pub fn step_names(&self) -> Vec<String> {
        self.steps.iter().map(|step| step.name.clone()).collect()
    }

    /// The distinct step kinds, as the published `step` tags list them.
    #[must_use]
    pub fn step_kinds(&self) -> Vec<&'static str> {
        let mut kinds = Vec::new();
        for step in &self.steps {
            if !kinds.contains(&step.kind.word()) {
                kinds.push(step.kind.word());
            }
        }
        kinds
    }
}

/// One program a host would not run, and why.
#[derive(Clone, Debug)]
pub struct Refused {
    pub source: String,
    pub reason: String,
}

/// The programs a host has resolved, and the ones it refused.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    programs: Vec<Program>,
    refused: Vec<Refused>,
    /// Where the answer came from: `local file`, and one day a relay.
    source: String,
}

impl Registry {
    /// Reads every program in one directory, in slug order.
    ///
    /// # Errors
    ///
    /// Returns the underlying error when the directory cannot be read.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        let mut programs = Vec::new();
        let mut refused = Vec::new();
        for path in paths {
            match Program::load(&path) {
                Ok(program) => programs.push(program),
                Err(reason) => refused.push(Refused {
                    source: path.display().to_string(),
                    reason,
                }),
            }
        }
        programs.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(Registry {
            programs,
            refused,
            source: "local file".to_string(),
        })
    }

    /// Reads each directory in `dirs` in turn. The first definition of a
    /// slug wins, so an operator's own directory overrides the
    /// repository's.
    #[must_use]
    pub fn open(dirs: &[PathBuf]) -> Self {
        let mut merged = Registry {
            source: "local file".to_string(),
            ..Registry::default()
        };
        for dir in dirs {
            let Ok(registry) = Registry::read(dir) else {
                continue;
            };
            for program in registry.programs {
                if merged.get(&program.slug).is_none() {
                    merged.programs.push(program);
                }
            }
            merged.refused.extend(registry.refused);
        }
        merged.programs.sort_by(|a, b| a.slug.cmp(&b.slug));
        merged
    }

    /// The programs, in slug order.
    #[must_use]
    pub fn programs(&self) -> &[Program] {
        &self.programs
    }

    /// One program by slug.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&Program> {
        self.programs.iter().find(|program| program.slug == slug)
    }

    /// The files this host would not run, each with its reason.
    #[must_use]
    pub fn refused(&self) -> &[Refused] {
        &self.refused
    }

    /// The slugs, in listing order.
    #[must_use]
    pub fn slugs(&self) -> Vec<String> {
        self.programs
            .iter()
            .map(|program| program.slug.clone())
            .collect()
    }

    /// The filter a relay read would carry, for the programs this listing
    /// holds.
    ///
    /// `authors` is left out here and named in the call's arguments,
    /// because a local read has no signer to filter by and a filter
    /// carrying a placeholder where a pubkey goes would not survive being
    /// sent.
    #[must_use]
    pub fn query(&self) -> Filter {
        let mut tags = BTreeMap::new();
        tags.insert("d".to_string(), self.slugs());
        Filter {
            kinds: Some(vec![PROGRAM_KIND]),
            tags,
            ..Filter::default()
        }
    }

    /// What each program's steps are, as the trace records them.
    #[must_use]
    pub fn listing(&self) -> Value {
        let mut out = Map::new();
        for program in &self.programs {
            out.insert(
                program.slug.clone(),
                json!({ "steps": program.step_names() }),
            );
        }
        Value::Object(out)
    }

    /// The sentence a surface shows beside the read.
    #[must_use]
    pub fn message(&self) -> String {
        "Resolved the available programs.".to_string()
    }

    /// What the read answered, in one line.
    #[must_use]
    pub fn output(&self) -> String {
        format!(
            "{} programs: {}",
            self.programs.len(),
            self.slugs().join(", ")
        )
    }

    /// The read as a trace records it.
    ///
    /// `operator` is the pubkey whose programs the host resolves, or
    /// `None` on a machine that has not been told. The query goes in the
    /// `extra` beside the answer.
    #[must_use]
    pub fn call(&self, operator: Option<&str>) -> Call {
        let author = operator.unwrap_or("<operator>");
        let mut extra = Map::new();
        extra.insert("resolved_from".to_string(), json!(self.source));
        extra.insert("nip".to_string(), json!("NIP-PRG"));
        extra.insert(
            "registry_query".to_string(),
            serde_json::to_value(self.query()).unwrap_or(Value::Null),
        );
        extra.insert(
            "note".to_string(),
            json!(
                "The Nostr query is recorded even though the answer came from disk, because the query is the part that has to keep working when the answer does not."
            ),
        );
        extra.insert("programs".to_string(), self.listing());
        if !self.refused.is_empty() {
            extra.insert(
                "refused".to_string(),
                json!(
                    self.refused
                        .iter()
                        .map(|refused| json!({
                            "source": refused.source,
                            "reason": refused.reason,
                        }))
                        .collect::<Vec<_>>()
                ),
            );
        }
        Call {
            id: String::new(),
            name: REGISTRY_CALL.to_string(),
            arguments: json!({
                "kinds": [PROGRAM_KIND],
                "authors": [author],
                "source": "local",
            }),
            output: self.output(),
            outcome: Outcome::Completed,
            milliseconds: 0,
            purpose: Some("List candidate programs before asking which one applies.".to_string()),
            extra,
        }
    }
}

/// Where a host looks for programs, in order.
///
/// `CODER_PROGRAM_DIR` first, then the repository's own `programs/`
/// directory, then the operator's `~/.openagents/programs`.
#[must_use]
pub fn search(repository: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(root) = repository {
        dirs.push(root.join("programs"));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents").join("programs"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_programs() -> Registry {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../programs");
        Registry::read(&dir).expect("the repository carries a programs dir")
    }

    #[test]
    fn the_registry_lists_the_five_programs_the_repository_carries() {
        let registry = repository_programs();
        assert!(registry.refused().is_empty(), "{:?}", registry.refused());
        let mut slugs = registry.slugs();
        slugs.sort();
        assert_eq!(
            slugs,
            [
                "answer-question",
                "burn-down",
                "delegate-fan-out",
                "review-changes",
                "run-suite"
            ]
        );
        assert_eq!(
            registry.get("delegate-fan-out").unwrap().step_names(),
            ["select", "independence", "admit", "fan_out", "accept"]
        );
        assert_eq!(
            registry.get("review-changes").unwrap().step_names(),
            ["mechanical", "review"]
        );
        assert_eq!(
            registry.get("answer-question").unwrap().step_names(),
            ["answer"]
        );
        assert_eq!(registry.get("run-suite").unwrap().step_names(), ["score"]);
    }

    #[test]
    fn the_read_records_the_query_beside_the_answer() {
        let registry = repository_programs();
        let call = registry.call(None);

        assert_eq!(call.name, REGISTRY_CALL);
        assert_eq!(call.arguments["kinds"], json!([PROGRAM_KIND]));
        assert_eq!(call.arguments["authors"], json!(["<operator>"]));
        assert_eq!(call.arguments["source"], json!("local"));
        assert_eq!(call.extra["resolved_from"], json!("local file"));
        assert_eq!(call.extra["nip"], json!("NIP-PRG"));
        assert_eq!(call.extra["registry_query"]["kinds"], json!([30182]));
        assert_eq!(
            call.extra["registry_query"]["#d"],
            json!(registry.slugs()),
            "the filter names the programs the listing holds"
        );
        assert_eq!(
            call.extra["programs"]["delegate-fan-out"]["steps"],
            json!(["select", "independence", "admit", "fan_out", "accept"])
        );
        assert!(call.output.starts_with("5 programs: "));
    }

    #[test]
    fn the_query_is_the_filter_a_relay_would_answer() {
        let registry = repository_programs();
        let filter = registry.query();
        filter
            .validate()
            .expect("the recorded query is a filter a relay accepts");
    }

    #[test]
    fn an_unrecognized_step_kind_refuses_the_whole_program() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exotic.json");
        std::fs::write(
            &path,
            r#"{"v":1,"slug":"exotic","steps":[
                {"name":"one","kind":"query","bounds":{}},
                {"name":"two","kind":"teleport","bounds":{}}]}"#,
        )
        .unwrap();

        let registry = Registry::read(dir.path()).unwrap();
        assert!(
            registry.programs().is_empty(),
            "the program is refused, not partly run"
        );
        assert_eq!(registry.refused().len(), 1);
        assert!(registry.refused()[0].reason.contains("teleport"));
    }

    #[test]
    fn a_decide_step_that_inlines_its_wording_is_refused() {
        let program: Program = serde_json::from_str(
            r#"{"v":1,"slug":"inlined","steps":[
                {"name":"ask","kind":"decide","question":"openagents.independence.v1",
                 "instructions":"The six tasks can run in parallel.","bounds":{}}]}"#,
        )
        .unwrap();
        let reason = program.validate().expect_err("wording is refused");
        assert!(reason.contains("instructions"), "{reason}");
    }

    /// A `query` step names a source by slug. A step that named a command
    /// would make the program code, and a source on a step that reads none
    /// was written for a host that does something else with it.
    #[test]
    fn a_source_that_is_not_a_slug_or_not_on_a_query_step_is_refused() {
        let program: Program = serde_json::from_str(
            r#"{"v":1,"slug":"commanded","steps":[
                {"name":"one","kind":"query","source":"gh issue list","bounds":{}}]}"#,
        )
        .unwrap();
        let reason = program.validate().expect_err("that is not a slug");
        assert!(reason.contains("source slug"), "{reason}");

        let program: Program = serde_json::from_str(
            r#"{"v":1,"slug":"misplaced","steps":[
                {"name":"one","kind":"check","source":"backlog",
                 "bounds":{"refuse_on":"cannot_enforce_intersection"}}]}"#,
        )
        .unwrap();
        let reason = program.validate().expect_err("a check reads no source");
        assert!(reason.contains("only a query step reads"), "{reason}");
    }

    #[test]
    fn a_repeated_step_name_is_refused() {
        let program: Program = serde_json::from_str(
            r#"{"v":1,"slug":"twice","steps":[
                {"name":"one","kind":"query","bounds":{}},
                {"name":"one","kind":"check","bounds":{}}]}"#,
        )
        .unwrap();
        assert!(
            program.validate().is_err(),
            "a step's output is addressed by its name"
        );
    }

    #[test]
    fn a_version_this_host_does_not_know_is_refused() {
        let program: Program = serde_json::from_str(
            r#"{"v":7,"slug":"future","steps":[{"name":"one","kind":"query","bounds":{}}]}"#,
        )
        .unwrap();
        assert!(program.validate().is_err());
    }

    #[test]
    fn the_fan_out_program_keeps_its_bounds() {
        let registry = repository_programs();
        let program = registry.get("delegate-fan-out").unwrap();
        let fan_out = program
            .steps
            .iter()
            .find(|step| step.name == "fan_out")
            .unwrap();
        assert_eq!(fan_out.kind, Kind::Delegate);
        assert_eq!(fan_out.bounds["concurrent_max"], json!(6));
        assert_eq!(fan_out.bounds["isolation"], json!("worktree"));

        let select = program
            .steps
            .iter()
            .find(|step| step.name == "select")
            .unwrap();
        assert_eq!(select.kind, Kind::Query);
        assert_eq!(
            select.source.as_deref(),
            Some("request"),
            "the lookup's source is named in the program"
        );
        assert_eq!(select.bounds["max_results"], json!(12));
        assert_eq!(select.bounds["on_overflow"], json!("refuse"));

        let independence = program
            .steps
            .iter()
            .find(|step| step.name == "independence")
            .unwrap();
        assert_eq!(independence.kind, Kind::Decide);
        assert_eq!(
            independence.question.as_deref(),
            Some("openagents.independence.v1")
        );
        assert_eq!(independence.bounds["refuse_below"], json!(0.7));
        assert_eq!(
            program.step_kinds(),
            ["query", "decide", "check", "delegate"]
        );
    }
}
