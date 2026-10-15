//! Local lookup service shared by the CLI and Coder. No model calls or uploads.

use crate::{lookup, recipes, store};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const TOOL: &str = "code_lookup";
pub const INSTRUCTIONS: &str = "For a repository task, use code_lookup to find a source-backed extension recipe before exploring files. Read its source references, preserve required companion changes, and check applicability. matched with required_roles_complete is a reading guide, not proof of a correct edit. For ambiguous, stale, no_match, or incomplete answers, use graph_symbol, graph_impact, and text search to recover. Never stop an edit merely because a recipe's short list omits a file. Run appropriate behavior checks.\n";

#[derive(Clone, Debug)]
pub struct Configuration {
    pub store: Option<PathBuf>,
    pub workspace: String,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            store: None,
            workspace: "local".into(),
        }
    }
}

pub fn locate(
    cwd: &Path,
    config: &Configuration,
) -> Result<(PathBuf, recipes::Snapshot, store::Store), String> {
    let root = PathBuf::from(
        String::from_utf8(recipes::git(cwd, &["rev-parse", "--show-toplevel"])?)
            .map_err(|e| e.to_string())?
            .trim(),
    );
    if !root.join("Cargo.toml").is_file() {
        return Err("This repository has no Rust task index.".into());
    }
    let snapshot = recipes::snapshot(&root, "HEAD", &config.workspace)?;
    let base = if let Some(path) = &config.store {
        path.clone()
    } else {
        PathBuf::from(
            String::from_utf8(recipes::git(
                &root,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )?)
            .map_err(|e| e.to_string())?
            .trim(),
        )
        .join("code-index")
    };
    let store = store::Store::new(&base, &snapshot);
    Ok((root, snapshot, store))
}

pub fn available(cwd: &Path, config: &Configuration) -> bool {
    locate(cwd, config)
        .ok()
        .and_then(|(_, _, store)| {
            store
                .active()
                .ok()
                .flatten()
                .and_then(|id| store.load(&id).ok())
        })
        .is_some()
}

pub fn definition() -> Value {
    json!({"type":"function","function":{"name":TOOL,"description":"Find source-backed code extension recipes, required companion changes, examples, and checks for a task.","parameters":{"type":"object","additionalProperties":false,"required":["question"],"properties":{
        "question":{"type":"string"},"operation":{"type":"string"},"max_bytes":{"type":"integer","minimum":1024,"maximum":131072},"conditions":{"type":"object","additionalProperties":{"type":"boolean"}}
    }}}})
}

pub fn execute(cwd: &Path, arguments: &Value, config: &Configuration) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let (root, snapshot, store) = locate(cwd, config)?;
    let id = store
        .active()?
        .ok_or("Build or fetch a task index before using this tool.")?;
    let bundle = store.load(&id)?;
    let question = arguments
        .get("question")
        .and_then(Value::as_str)
        .ok_or("A question is required.")?
        .to_owned();
    let conditions: BTreeMap<String, bool> = arguments
        .get("conditions")
        .map(|v| {
            serde_json::from_value(v.clone()).map_err(|_| "Conditions must be named yes/no values.")
        })
        .transpose()?
        .unwrap_or_default();
    let max_bytes = arguments
        .get("max_bytes")
        .map(|v| {
            v.as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or("Choose an integer context budget.")
        })
        .transpose()?
        .unwrap_or(16000);
    let mut request = lookup::Request {
        question,
        operation: arguments
            .get("operation")
            .and_then(Value::as_str)
            .map(str::to_owned),
        snapshot,
        max_bytes,
        conditions,
        file_candidates: vec![],
        query_vector: None,
        recipe_vectors: BTreeMap::new(),
        decision: None,
    };
    if let Ok(Some(selector)) = crate::learning::active(&store.directory, &bundle.artifact) {
        let _ = crate::learning::apply(&selector, &bundle.artifact, &mut request);
    }
    let dirty = if bundle.artifact.snapshot == request.snapshot {
        let graph = code_graph::store::open(&root)?;
        store::dirty(&bundle, &root, &graph.index)?
    } else {
        Default::default()
    };
    let answer = store::query(&bundle, &request, &dirty)?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let observation = json!({"schema":"openagents.lookup-observation.v1","at_ms":atif::now_ms(),"repository":request.snapshot.repository,"workspace":request.snapshot.workspace,"commit":request.snapshot.commit,"artifact_digest":answer.artifact_digest,
        "question_digest":crate::digest(request.question.as_bytes()),"status":answer.status,"required_roles_complete":answer.required_roles_complete,"candidates":answer.candidates.iter().map(|c|&c.id).collect::<Vec<_>>(),"elapsed_ms":started.elapsed().as_millis(),"cost_usd":null});
    let name = format!(
        "lookup-{}-{}-{sequence}.json",
        atif::now_ms(),
        std::process::id()
    );
    // Measurement failure cannot turn a successful read into an execution failure.
    let _ = crate::private::write(
        &store.directory.join("observations"),
        &name,
        &serde_json::to_vec(&observation).map_err(|e| e.to_string())?,
    );
    serde_json::to_value(answer).map_err(|e| e.to_string())
}
