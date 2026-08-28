//! `openagents gym dataset` — named, versioned groups of traces and tasks.
//!
//! Datasets bridge the flat corpus and a pinned effectiveness suite. Each
//! membership change is append-recorded to `membership.jsonl` so the log stays
//! readable and immutable.

use crate::auth::home_directory;
use crate::errors::CliError;
use crate::gym::corpus::InventoryDocument;
use crate::gym::schemas::{DatasetMember, DatasetView};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;

pub const DATASET_VIEW_SCHEMA: &str = "openagents.gym.dataset_view.v1";
const DATASET_DIFF_SCHEMA: &str = "openagents.gym.dataset_diff.v1";
const DATASET_LIST_SCHEMA: &str = "openagents.gym.dataset_list.v1";
const EFFECTIVENESS_SUITE_SCHEMA: &str = "openagents.effectiveness_suite.v1";

#[derive(Args, Debug)]
pub struct DatasetArgs {
    #[command(subcommand)]
    pub action: DatasetAction,
}

#[derive(Subcommand, Debug)]
pub enum DatasetAction {
    /// List all known datasets.
    List,
    /// Create a new dataset.
    Create {
        #[arg(help = "Dataset id")]
        id: String,
        #[arg(long, help = "Dataset description")]
        description: Option<String>,
    },
    /// Add one or more trace or task references to a dataset.
    Add {
        #[arg(help = "Dataset id")]
        id: String,
        #[arg(required = true, help = "Trace or task references to add")]
        refs: Vec<String>,
        #[arg(long, help = "Tag the added members; repeatable")]
        tag: Vec<String>,
    },
    /// Remove one or more members from a dataset.
    Remove {
        #[arg(help = "Dataset id")]
        id: String,
        #[arg(required = true, help = "References to remove")]
        refs: Vec<String>,
    },
    /// Show a dataset and its membership log.
    Show {
        #[arg(help = "Dataset id")]
        id: String,
    },
    /// Pin a dataset to an effectiveness suite manifest.
    Pin {
        #[arg(help = "Dataset id")]
        id: String,
        #[arg(long, help = "Output path for the suite manifest")]
        out: Option<String>,
    },
    /// Show drift between a dataset and a pinned suite file.
    Diff {
        #[arg(help = "Dataset id")]
        id: String,
        #[arg(help = "Path to the pinned suite file")]
        suite_file: PathBuf,
    },
    /// Distill labeled sessions into candidate task drafts. Never promotes.
    Distill {
        #[arg(long, help = "JSONL labels file")]
        labels: PathBuf,
        #[arg(long, help = "Directory to write draft task files")]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatasetMeta {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LogRow {
    pub op: String,
    pub at: String,
    pub who: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub kind: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskFile {
    pub id: String,
    pub pin: TaskPin,
    #[serde(rename = "environmentAvailable")]
    pub environment_available: bool,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum TaskPin {
    #[serde(rename = "harbor-registry")]
    HarborRegistry {
        dataset: String,
        #[serde(rename = "gitUrl")]
        git_url: String,
        commit: String,
        path: String,
    },
    #[serde(rename = "tracker-closed-issue")]
    TrackerClosedIssue {
        repo: String,
        issue: u64,
        #[serde(rename = "acceptedCommit")]
        accepted_commit: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SuiteManifest {
    pub schema: String,
    pub id: String,
    pub tier: String,
    pub description: String,
    pub tasks: Vec<SuiteTask>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SuiteTask {
    pub id: String,
    pub pin: TaskPin,
    pub environment_available: bool,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiffReport {
    pub dataset_id: String,
    pub suite_file: PathBuf,
    pub drifted: bool,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

pub fn run_dataset(args: DatasetArgs, json: bool) -> Result<(), CliError> {
    match args.action {
        DatasetAction::List => list_datasets(json),
        DatasetAction::Create { id, description } => {
            create_dataset(&id, description.as_deref(), json)
        }
        DatasetAction::Add { id, refs, tag } => add_members(&id, &refs, &tag, json),
        DatasetAction::Remove { id, refs } => remove_members(&id, &refs, json),
        DatasetAction::Show { id } => show_dataset(&id, json),
        DatasetAction::Pin { id, out } => pin_dataset(&id, out.as_deref(), json),
        DatasetAction::Diff { id, suite_file } => diff_dataset(&id, &suite_file, json),
        DatasetAction::Distill { labels, out } => distill_labels(&labels, out.as_deref(), json),
    }
}

// ---------------------------------------------------------------------------
// paths and helpers
// ---------------------------------------------------------------------------

fn datasets_dir() -> PathBuf {
    home_directory()
        .join(".openagents")
        .join("gym")
        .join("datasets")
}

fn dataset_dir(id: &str) -> PathBuf {
    datasets_dir().join(id)
}

fn meta_path(dir: &Path) -> PathBuf {
    dir.join("meta.json")
}

fn log_path(dir: &Path) -> PathBuf {
    dir.join("membership.jsonl")
}

fn now_iso() -> Result<String, CliError> {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|e| CliError::Output(format!("could not format timestamp: {e}")))
}

fn who() -> String {
    std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

fn read_meta(dir: &Path) -> Result<DatasetMeta, CliError> {
    let text = fs::read_to_string(meta_path(dir))
        .map_err(|e| CliError::Configuration(format!("could not read dataset meta: {e}")))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::Configuration(format!("dataset meta is invalid: {e}")))
}

fn write_meta(dir: &Path, meta: &DatasetMeta) -> Result<(), CliError> {
    if let Some(parent) = dir.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(meta)
        .map_err(|e| CliError::Output(format!("could not serialize dataset meta: {e}")))?;
    fs::write(meta_path(dir), text)
        .map_err(|e| CliError::Configuration(format!("could not write dataset meta: {e}")))
}

fn read_log(dir: &Path) -> Result<Vec<LogRow>, CliError> {
    let path = log_path(dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&path)
        .map_err(|e| CliError::Configuration(format!("could not read membership log: {e}")))?;
    let mut rows = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line =
            line.map_err(|e| CliError::Configuration(format!("membership log read error: {e}")))?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).map_err(|e| {
            CliError::Configuration(format!("membership log contains invalid row: {e}"))
        })?);
    }
    Ok(rows)
}

fn append_log(dir: &Path, row: &LogRow) -> Result<(), CliError> {
    let path = log_path(dir);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| CliError::Configuration(format!("could not open membership log: {e}")))?;
    let line = serde_json::to_string(row)
        .map_err(|e| CliError::Output(format!("could not serialize log row: {e}")))?;
    writeln!(file, "{line}")
        .map_err(|e| CliError::Configuration(format!("could not append membership log: {e}")))
}

fn active_members(log: &[LogRow]) -> BTreeMap<String, LogRow> {
    let mut active = BTreeMap::new();
    for row in log {
        match row.op.as_str() {
            "add" => {
                active.insert(row.reference.clone(), row.clone());
            }
            "remove" => {
                active.remove(&row.reference);
            }
            _ => {}
        }
    }
    active
}

fn build_view(meta: &DatasetMeta, log: &[LogRow]) -> DatasetView {
    let active = active_members(log);
    let members: Vec<_> = active
        .values()
        .map(|row| DatasetMember {
            reference: row.reference.clone(),
            kind: row.kind.clone(),
            added_by: row.who.clone(),
            added_at: row.at.clone(),
            reason: row.reason.clone(),
            tags: row.tags.clone().unwrap_or_default(),
        })
        .collect();
    DatasetView {
        schema: DATASET_VIEW_SCHEMA.to_string(),
        dataset_id: meta.id.clone(),
        description: meta.description.clone(),
        version: meta.version,
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        members,
    }
}

fn inventory_path() -> Result<PathBuf, CliError> {
    if let Some(env) = std::env::var_os("OPENAGENTS_GYM_INVENTORY") {
        let path = PathBuf::from(env);
        if path.is_file() {
            return Ok(path);
        }
    }
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::Configuration(format!("could not read current directory: {e}")))?;
    let default = cwd.join("docs").join("coderbench").join("inventory.json");
    if default.is_file() {
        return Ok(default);
    }
    let home = home_directory()
        .join(".openagents")
        .join("gym")
        .join("corpus")
        .join("inventory.json");
    if home.is_file() {
        return Ok(home);
    }
    Err(CliError::Configuration(
        "no corpus inventory found. Run `gym corpus inventory` or set OPENAGENTS_GYM_INVENTORY"
            .to_string(),
    ))
}

fn read_inventory() -> Result<InventoryDocument, CliError> {
    let path = inventory_path()?;
    let text = fs::read_to_string(&path).map_err(|e| {
        CliError::Configuration(format!(
            "could not read inventory {}: {}",
            path.display(),
            e
        ))
    })?;
    serde_json::from_str(&text).map_err(|e| {
        CliError::Configuration(format!(
            "{} is not a valid inventory: {}",
            path.display(),
            e
        ))
    })
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

fn nearest_digest(target: &str, inventory: &InventoryDocument) -> Option<String> {
    let candidates: Vec<_> = inventory
        .rows
        .iter()
        .filter_map(|r| r.digest.as_ref())
        .collect();
    let mut best: Option<(usize, &String)> = None;
    for cand in &candidates {
        let d = edit_distance(target, cand);
        if d == 0 {
            continue;
        }
        if best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, cand));
        }
    }
    // Suggest only when the candidate is plausibly a typo.
    best.filter(|(d, _)| *d <= 6 || (*d as f64) / (target.len() as f64) <= 0.25)
        .map(|(_, s)| s.clone())
}

fn is_trace_ref(reference: &str) -> bool {
    reference.starts_with("sha256:") || reference.starts_with("/trace/")
}

fn resolve_trace_ref(reference: &str, inventory: &InventoryDocument) -> Result<String, CliError> {
    if reference.starts_with("sha256:") {
        if inventory
            .rows
            .iter()
            .any(|r| r.digest.as_deref() == Some(reference))
        {
            return Ok(reference.to_string());
        }
        if let Some(nearest) = nearest_digest(reference, inventory) {
            return Err(CliError::Input(format!(
                "digest {} is not in the corpus ledger (nearest: {})",
                reference, nearest
            )));
        }
        return Err(CliError::Input(format!(
            "digest {} is not in the corpus ledger",
            reference
        )));
    }
    if let Some(uuid) = reference.strip_prefix("/trace/") {
        let uuid = uuid.trim();
        if !uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return Err(CliError::Input(format!(
                "{} is not a valid /trace/ reference",
                reference
            )));
        }
        let found = inventory
            .rows
            .iter()
            .any(|r| r.path.to_string_lossy().contains(uuid));
        if found {
            return Ok(reference.to_string());
        }
        return Err(CliError::Input(format!(
            "{} does not resolve to a local corpus row",
            reference
        )));
    }
    Err(CliError::Input(format!(
        "{} is not a trace reference",
        reference
    )))
}

fn resolve_task_path(reference: &str) -> Result<PathBuf, CliError> {
    if let Some(root) = std::env::var_os("OPENAGENTS_GYM_TASKS_DIR") {
        let p = PathBuf::from(&root).join(reference);
        if p.is_file() {
            return Ok(p);
        }
        let with_json = p.with_extension("json");
        if with_json.is_file() {
            return Ok(with_json);
        }
        return Err(CliError::Input(format!(
            "task file not found for {} under {}",
            reference,
            root.to_string_lossy()
        )));
    }
    let p = PathBuf::from(reference);
    if p.is_file() {
        return Ok(p);
    }
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::Configuration(format!("could not read current directory: {e}")))?;
    let rel = cwd.join(reference);
    if rel.is_file() {
        return Ok(rel);
    }
    let with_json = rel.with_extension("json");
    if with_json.is_file() {
        return Ok(with_json);
    }
    Err(CliError::Input(format!(
        "task file not found for {}",
        reference
    )))
}

fn read_task_file(reference: &str) -> Result<TaskFile, CliError> {
    let path = resolve_task_path(reference)?;
    let text = fs::read_to_string(&path).map_err(|e| {
        CliError::Configuration(format!("could not read {}: {}", path.display(), e))
    })?;
    let task: TaskFile = serde_json::from_str(&text).map_err(|e| {
        CliError::Configuration(format!("{} is not a valid task pin: {}", path.display(), e))
    })?;
    Ok(task)
}

fn short_trace_id(reference: &str) -> String {
    if let Some(digest) = reference.strip_prefix("sha256:") {
        let end = digest.len().min(12);
        return format!("trace-{}", &digest[..end]);
    }
    if let Some(uuid) = reference.strip_prefix("/trace/") {
        let end = uuid.len().min(8);
        return format!("trace-{}", &uuid[..end]);
    }
    reference.to_string()
}

fn trace_to_suite_task(
    reference: &str,
    inventory: &InventoryDocument,
    dataset_id: &str,
) -> Result<SuiteTask, CliError> {
    if reference.starts_with("sha256:") {
        let row = inventory
            .rows
            .iter()
            .find(|r| r.digest.as_deref() == Some(reference))
            .ok_or_else(|| {
                CliError::Configuration(format!("{} is not in the inventory", reference))
            })?;
        let repo_hint = row.repo_hint.as_deref().unwrap_or("openagents/corpus");
        let git_url = format!("https://github.com/{repo_hint}.git");
        let digest = row.digest.as_deref().unwrap_or(reference);
        return Ok(SuiteTask {
            id: short_trace_id(reference),
            pin: TaskPin::HarborRegistry {
                dataset: dataset_id.to_string(),
                git_url,
                commit: digest.to_string(),
                path: "trace".to_string(),
            },
            environment_available: true,
            rationale: Some("corpus trace".to_string()),
        });
    }
    if let Some(uuid) = reference.strip_prefix("/trace/") {
        return Ok(SuiteTask {
            id: short_trace_id(reference),
            pin: TaskPin::HarborRegistry {
                dataset: dataset_id.to_string(),
                git_url: format!("https://openagents.com/trace/{uuid}"),
                commit: uuid.to_string(),
                path: "trace".to_string(),
            },
            environment_available: true,
            rationale: Some("corpus trace".to_string()),
        });
    }
    Err(CliError::Configuration(format!(
        "{} is not a trace reference",
        reference
    )))
}

fn task_id_for_reference(
    reference: &str,
    _inventory: &InventoryDocument,
) -> Result<String, CliError> {
    if is_trace_ref(reference) {
        return Ok(short_trace_id(reference));
    }
    let task = read_task_file(reference)?;
    Ok(task.id)
}

// ---------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------

fn list_datasets(json: bool) -> Result<(), CliError> {
    let mut summaries = Vec::new();
    if datasets_dir().is_dir() {
        for entry in fs::read_dir(datasets_dir())
            .map_err(|e| CliError::Configuration(format!("could not list datasets: {e}")))?
        {
            let entry =
                entry.map_err(|e| CliError::Configuration(format!("dataset entry error: {e}")))?;
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let meta = read_meta(&dir)?;
            summaries.push(serde_json::json!({
                "id": meta.id,
                "description": meta.description,
                "updated_at": meta.updated_at,
                "version": meta.version,
            }));
        }
    }
    summaries.sort_by(|a, b| {
        let a = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let b = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        a.cmp(b)
    });
    let value = serde_json::json!({
        "schema": DATASET_LIST_SCHEMA,
        "datasets": summaries,
    });
    let human: Vec<String> = if summaries.is_empty() {
        vec!["No datasets.".to_string()]
    } else {
        summaries
            .iter()
            .map(|s| {
                format!(
                    "{}  {}",
                    s.get("id").and_then(|v| v.as_str()).unwrap_or("?"),
                    s.get("updated_at").and_then(|v| v.as_str()).unwrap_or("")
                )
            })
            .collect()
    };
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn create_dataset(id: &str, description: Option<&str>, json: bool) -> Result<(), CliError> {
    let dir = dataset_dir(id);
    if dir.exists() {
        return Err(CliError::Input(format!("dataset {} already exists", id)));
    }
    fs::create_dir_all(&dir)
        .map_err(|e| CliError::Configuration(format!("could not create dataset directory: {e}")))?;
    let now = now_iso()?;
    let meta = DatasetMeta {
        id: id.to_string(),
        description: description.map(String::from),
        created_at: now.clone(),
        updated_at: now,
        version: 0,
    };
    write_meta(&dir, &meta)?;
    // Empty log is written on first append.
    let view = build_view(&meta, &[]);
    let value = serde_json::to_value(&view)
        .map_err(|e| CliError::Output(format!("could not render dataset view: {e}")))?;
    let human = vec![
        format!("Created dataset {}", id),
        format!("  {}", dir.display()),
    ];
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn add_members(id: &str, refs: &[String], tags: &[String], json: bool) -> Result<(), CliError> {
    let dir = dataset_dir(id);
    if !dir.is_dir() {
        return Err(CliError::Input(format!("dataset {} does not exist", id)));
    }
    let mut meta = read_meta(&dir)?;
    let mut log = read_log(&dir)?;
    let need_inventory = refs.iter().any(|r| is_trace_ref(r));
    let inventory = if need_inventory {
        Some(read_inventory()?)
    } else {
        None
    };
    let at = now_iso()?;
    let who = who();
    for reference in refs {
        let kind = if is_trace_ref(reference) {
            resolve_trace_ref(reference, inventory.as_ref().unwrap())?;
            "trace".to_string()
        } else {
            read_task_file(reference)?;
            "task".to_string()
        };
        let reason = if tags.is_empty() {
            "gym dataset add".to_string()
        } else {
            format!("gym dataset add; tags: {}", tags.join(","))
        };
        let row = LogRow {
            op: "add".to_string(),
            at: at.clone(),
            who: who.clone(),
            reference: reference.clone(),
            kind,
            reason,
            tags: if tags.is_empty() {
                None
            } else {
                Some(tags.to_vec())
            },
        };
        append_log(&dir, &row)?;
        log.push(row);
        meta.version += 1;
        meta.updated_at = at.clone();
    }
    write_meta(&dir, &meta)?;
    let view = build_view(&meta, &log);
    let value = serde_json::to_value(&view)
        .map_err(|e| CliError::Output(format!("could not render dataset view: {e}")))?;
    let human: Vec<String> = std::iter::once(format!("Added {} member(s) to {}", refs.len(), id))
        .chain(refs.iter().map(|r| format!("  + {}", r)))
        .collect();
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn remove_members(id: &str, refs: &[String], json: bool) -> Result<(), CliError> {
    let dir = dataset_dir(id);
    if !dir.is_dir() {
        return Err(CliError::Input(format!("dataset {} does not exist", id)));
    }
    let mut meta = read_meta(&dir)?;
    let mut log = read_log(&dir)?;
    let active = active_members(&log);
    let at = now_iso()?;
    let who = who();
    for reference in refs {
        if !active.contains_key(reference) {
            return Err(CliError::Input(format!(
                "{} is not a member of dataset {}",
                reference, id
            )));
        }
        let row = LogRow {
            op: "remove".to_string(),
            at: at.clone(),
            who: who.clone(),
            reference: reference.clone(),
            kind: active[reference].kind.clone(),
            reason: "gym dataset remove".to_string(),
            tags: None,
        };
        append_log(&dir, &row)?;
        log.push(row);
        meta.version += 1;
        meta.updated_at = at.clone();
    }
    write_meta(&dir, &meta)?;
    let view = build_view(&meta, &log);
    let value = serde_json::to_value(&view)
        .map_err(|e| CliError::Output(format!("could not render dataset view: {e}")))?;
    let human: Vec<String> =
        std::iter::once(format!("Removed {} member(s) from {}", refs.len(), id))
            .chain(refs.iter().map(|r| format!("  - {}", r)))
            .collect();
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn show_dataset(id: &str, json: bool) -> Result<(), CliError> {
    let dir = dataset_dir(id);
    if !dir.is_dir() {
        return Err(CliError::Input(format!("dataset {} does not exist", id)));
    }
    let meta = read_meta(&dir)?;
    let log = read_log(&dir)?;
    let view = build_view(&meta, &log);
    let value = serde_json::to_value(&view)
        .map_err(|e| CliError::Output(format!("could not render dataset view: {e}")))?;
    let mut human = vec![format!(
        "{}  v{}  updated {}",
        view.dataset_id, view.version, view.updated_at
    )];
    if let Some(desc) = &view.description {
        human.push(format!("  {}", desc));
    }
    human.push(format!("  {} member(s)", view.members.len()));
    for member in &view.members {
        let tags = if member.tags.is_empty() {
            String::new()
        } else {
            format!("  tags=[{}]", member.tags.join(","))
        };
        human.push(format!(
            "  {:<24} {:<8} {}  {}  {}{}",
            member.reference, member.kind, member.added_by, member.added_at, member.reason, tags
        ));
    }
    human.push("Log:".to_string());
    for row in &log {
        let tags = match &row.tags {
            Some(t) if !t.is_empty() => format!("  tags=[{}]", t.join(",")),
            _ => String::new(),
        };
        human.push(format!(
            "  {}  {}  {}  {}  ({}){}",
            row.at, row.who, row.op, row.reference, row.reason, tags
        ));
    }
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn pin_dataset(id: &str, out: Option<&str>, json: bool) -> Result<(), CliError> {
    let dir = dataset_dir(id);
    if !dir.is_dir() {
        return Err(CliError::Input(format!("dataset {} does not exist", id)));
    }
    let meta = read_meta(&dir)?;
    let log = read_log(&dir)?;
    let active = active_members(&log);
    let inventory = if active.values().any(|r| is_trace_ref(&r.reference)) {
        Some(read_inventory()?)
    } else {
        None
    };
    let mut tasks = Vec::with_capacity(active.len());
    let mut seen = BTreeSet::new();
    for row in active.values() {
        let task = if row.kind == "task" {
            let file = read_task_file(&row.reference)?;
            SuiteTask {
                id: file.id,
                pin: file.pin,
                environment_available: file.environment_available,
                rationale: file.rationale,
            }
        } else {
            trace_to_suite_task(&row.reference, inventory.as_ref().unwrap(), id)?
        };
        if !seen.insert(task.id.clone()) {
            return Err(CliError::Configuration(format!(
                "compiled task id {} appears more than once",
                task.id
            )));
        }
        tasks.push(task);
    }
    // Stable order.
    tasks.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    let suite = SuiteManifest {
        schema: EFFECTIVENESS_SUITE_SCHEMA.to_string(),
        id: id.to_string(),
        tier: "smoke".to_string(),
        description: meta.description.clone().unwrap_or_else(|| id.to_string()),
        tasks,
    };
    let path: PathBuf = match out {
        Some(s) => PathBuf::from(s),
        None => {
            let cwd = std::env::current_dir().map_err(|e| {
                CliError::Configuration(format!("could not read current directory: {e}"))
            })?;
            cwd.join("bench")
                .join("suites")
                .join(format!("{id}.suite.json"))
        }
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CliError::Configuration(format!("could not create suite directory: {e}"))
        })?;
    }
    let text = serde_json::to_string_pretty(&suite)
        .map_err(|e| CliError::Output(format!("could not serialize suite: {e}")))?;
    fs::write(&path, text).map_err(|e| {
        CliError::Configuration(format!("could not write suite {}: {}", path.display(), e))
    })?;
    let suite_value = serde_json::to_value(&suite)
        .map_err(|e| CliError::Output(format!("could not render suite: {e}")))?;
    let human = vec![format!(
        "Wrote {} with {} task(s)",
        path.display(),
        suite.tasks.len()
    )];
    crate::cli::emit(json, &suite_value, &human);
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuiteFileMinimal {
    tasks: Vec<MinimalTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinimalTask {
    id: String,
}

fn suite_task_ids(suite_file: &Path) -> Result<BTreeSet<String>, CliError> {
    let text = fs::read_to_string(suite_file).map_err(|e| {
        CliError::Configuration(format!(
            "could not read suite {}: {}",
            suite_file.display(),
            e
        ))
    })?;
    let suite: SuiteFileMinimal = serde_json::from_str(&text).map_err(|e| {
        CliError::Configuration(format!(
            "{} is not a valid suite: {}",
            suite_file.display(),
            e
        ))
    })?;
    Ok(suite.tasks.into_iter().map(|t| t.id).collect())
}

pub fn diff_report(id: &str, suite_file: &Path) -> Result<DiffReport, CliError> {
    let dir = dataset_dir(id);
    if !dir.is_dir() {
        return Err(CliError::Input(format!("dataset {} does not exist", id)));
    }
    let meta = read_meta(&dir)?;
    let log = read_log(&dir)?;
    let active = active_members(&log);
    let inventory = if active.values().any(|r| is_trace_ref(&r.reference)) {
        Some(read_inventory()?)
    } else {
        None
    };
    let mut dataset_ids = BTreeSet::new();
    for row in active.values() {
        dataset_ids.insert(task_id_for_reference(
            &row.reference,
            inventory.as_ref().unwrap_or(&InventoryDocument {
                schema: crate::gym::corpus::INVENTORY_SCHEMA.to_string(),
                stores: Vec::new(),
                rows: Vec::new(),
                counts: None,
            }),
        )?);
    }
    let suite_ids = suite_task_ids(suite_file)?;
    let missing: Vec<_> = dataset_ids.difference(&suite_ids).cloned().collect();
    let extra: Vec<_> = suite_ids.difference(&dataset_ids).cloned().collect();
    Ok(DiffReport {
        dataset_id: meta.id,
        suite_file: suite_file.to_path_buf(),
        drifted: !missing.is_empty() || !extra.is_empty(),
        missing,
        extra,
    })
}

fn diff_dataset(id: &str, suite_file: &Path, json: bool) -> Result<(), CliError> {
    let report = diff_report(id, suite_file)?;
    let value = serde_json::json!({
        "schema": DATASET_DIFF_SCHEMA,
        "dataset_id": report.dataset_id,
        "suite_file": report.suite_file.to_string_lossy(),
        "drifted": report.drifted,
        "missing": report.missing,
        "extra": report.extra,
    });
    let human: Vec<String> = if !report.drifted {
        vec![format!(
            "no drift between {} and {}",
            id,
            report.suite_file.display()
        )]
    } else {
        let mut lines = vec![format!(
            "drift between {} and {}",
            id,
            report.suite_file.display()
        )];
        for id in &report.missing {
            lines.push(format!("  + {}  (in dataset, not suite)", id));
        }
        for id in &report.extra {
            lines.push(format!("  - {}  (in suite, not dataset)", id));
        }
        lines
    };
    crate::cli::emit(json, &value, &human);
    Ok(())
}

/// Distill labeled sessions into candidate task drafts.
///
/// Drafts are candidates. This command never writes a live task under
/// `bench/tasks/coderbench/` without a `drafts` directory, and it never sets
/// a draft to anything but `status: draft`.
pub fn distill_labels(labels: &Path, out: Option<&Path>, json: bool) -> Result<(), CliError> {
    let out_dir = match out {
        Some(p) => p.to_path_buf(),
        None => repo_root()?.join("bench/tasks/coderbench/drafts"),
    };
    fs::create_dir_all(&out_dir)
        .map_err(|e| CliError::Output(format!("could not create {}: {e}", out_dir.display())))?;
    let text = fs::read_to_string(labels)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", labels.display())))?;
    let mut written = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line).map_err(|e| {
            CliError::Input(format!(
                "{} line {} is not JSON: {e}",
                labels.display(),
                i + 1
            ))
        })?;
        let draft = distill_one(&row)?;
        let id = draft
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("draft")
            .to_string();
        let path = out_dir.join(format!("{id}.task.json"));
        let body =
            serde_json::to_string_pretty(&draft).map_err(|e| CliError::Output(e.to_string()))?;
        fs::write(&path, body)
            .map_err(|e| CliError::Output(format!("could not write {}: {e}", path.display())))?;
        written.push(path);
    }
    let value = serde_json::json!({
        "schema": "openagents.gym.task_draft_batch.v1",
        "status": "draft",
        "count": written.len(),
        "out": out_dir,
        "files": written,
    });
    let human = vec![format!(
        "wrote {} draft(s) under {} (candidates, not deployments)",
        written.len(),
        out_dir.display()
    )];
    crate::cli::emit(json, &value, &human);
    Ok(())
}

fn distill_one(row: &Value) -> Result<Value, CliError> {
    let issues = row
        .get("issues_closed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let issue = issues
        .first()
        .and_then(Value::as_u64)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unspecified".into());
    let outcome = row
        .get("outcome_type")
        .and_then(Value::as_str)
        .unwrap_or("feature");
    let oracle = row.get("oracle").cloned().unwrap_or(Value::Null);
    let command = oracle
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let kind = oracle
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("gate")
        .to_string();
    let commits = row
        .get("commits_landed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let start = commits
        .first()
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let id = format!("issue-{issue}");
    let instruction = format!(
        "Close the {outcome} work for issue #{issue} so the oracle exits 0. Rebuild the outcome from the issue and the named gate; do not copy a historical solution from the source session."
    );
    Ok(serde_json::json!({
        "schema": "openagents.gym.task_draft.v1",
        "status": "draft",
        "id": id,
        "instruction": instruction,
        "start_commit": start,
        "oracle": { "kind": kind, "command": command },
        "provenance": {
            "trace_ref": row.get("trace_ref"),
            "issues_closed": issues,
            "commits_landed": commits,
        },
        "gradeable": false,
    }))
}

fn repo_root() -> Result<PathBuf, CliError> {
    let mut dir = std::env::current_dir().map_err(|e| CliError::Configuration(e.to_string()))?;
    loop {
        if dir.join("bench").join("suites").is_dir() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => {
                return Err(CliError::Configuration(
                    "could not find bench/suites from the current directory".into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_inventory() -> serde_json::Value {
        serde_json::json!({
            "schema": "openagents.gym.corpus_inventory.v1",
            "stores": [],
            "rows": [
                {
                    "source": "openagents",
                    "path": "/Users/example/.openagents/exports/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.json",
                    "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "bytes": 1048576,
                    "steps": 12,
                    "started_at": "2026-08-26T14:00:00Z",
                    "ended_at": "2026-08-26T14:05:00Z",
                    "model": "openai/gpt-4",
                    "repo_hint": "OpenAgentsInc/openagents",
                    "domain": "agent-building",
                    "qualifies": true,
                }
            ]
        })
    }

    fn fixture_task() -> serde_json::Value {
        serde_json::json!({
            "id": "regex-log",
            "pin": {
                "kind": "harbor-registry",
                "dataset": "terminal-bench@2.0",
                "gitUrl": "https://github.com/laude-institute/terminal-bench-2.git",
                "commit": "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
                "path": "regex-log"
            },
            "environmentAvailable": true,
            "rationale": "quick-shaped"
        })
    }

    #[test]
    fn dataset_membership_round_trip_and_pin_check_diff() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let inventory = tmp.path().join("inventory.json");
        let task_root = tmp.path().join("bench").join("tasks").join("coderbench");
        let suite_dir = tmp.path().join("bench").join("suites");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&task_root).unwrap();
        fs::create_dir_all(&suite_dir).unwrap();

        let inventory_text = serde_json::to_string(&fixture_inventory()).unwrap();
        fs::write(&inventory, inventory_text).unwrap();
        let task_text = serde_json::to_string_pretty(&fixture_task()).unwrap();
        fs::write(task_root.join("regex-log.json"), task_text).unwrap();

        unsafe {
            std::env::set_var("HOME", home.to_str().unwrap());
            std::env::set_var("OPENAGENTS_GYM_INVENTORY", inventory.to_str().unwrap());
            std::env::set_var("OPENAGENTS_GYM_TASKS_DIR", tmp.path().to_str().unwrap());
        }

        let id = "coderbench-agent-building-v1";
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Create {
                    id: id.to_string(),
                    description: Some("Agent-building dataset".to_string()),
                },
            },
            false,
        )
        .unwrap();

        let trace_ref = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let task_ref = "bench/tasks/coderbench/regex-log";
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Add {
                    id: id.to_string(),
                    refs: vec![trace_ref.to_string(), task_ref.to_string()],
                    tag: vec!["smoke".to_string()],
                },
            },
            false,
        )
        .unwrap();

        // Show should not fail.
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Show { id: id.to_string() },
            },
            false,
        )
        .unwrap();

        let log_path = dataset_dir(id).join("membership.jsonl");
        assert!(log_path.is_file(), "membership log was written");
        let log_text = fs::read_to_string(&log_path).unwrap();
        assert!(log_text.contains("add"), "log records add operations");
        assert!(log_text.contains("who"), "log records who");
        assert!(log_text.contains("at"), "log records when");
        assert!(log_text.contains("reason"), "log records why");

        let out = suite_dir.join(format!("{id}.suite.json"));
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Pin {
                    id: id.to_string(),
                    out: Some(out.to_str().unwrap().to_string()),
                },
            },
            false,
        )
        .unwrap();
        assert!(out.is_file(), "suite manifest was written");

        let check = crate::gym::suite::run_suite(
            crate::gym::suite::SuiteAction::Check { file: out.clone() },
            false,
        );
        assert!(check.is_ok(), "pinned suite passes gym suite check");

        // Remove the trace and diff the now-pinned suite.
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Remove {
                    id: id.to_string(),
                    refs: vec![trace_ref.to_string()],
                },
            },
            false,
        )
        .unwrap();

        let report = diff_report(id, &out).unwrap();
        assert!(report.drifted, "diff reports drift after membership change");
        assert!(
            report.extra.iter().any(|x| x.starts_with("trace-")),
            "drift names the removed trace task"
        );

        // Refuse an unknown digest and name the nearest one.
        let bad = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab";
        let result = run_dataset(
            DatasetArgs {
                action: DatasetAction::Add {
                    id: id.to_string(),
                    refs: vec![bad.to_string()],
                    tag: vec![],
                },
            },
            false,
        );
        assert!(result.is_err(), "missing digest is refused");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("nearest:"), "error names the nearest digest");
        assert!(
            msg.contains("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "nearest suggestion is the real digest"
        );
    }

    #[test]
    fn distill_writes_drafts_and_never_promotes() {
        let tmp = tempfile::tempdir().unwrap();
        let labels = tmp.path().join("labels.jsonl");
        fs::write(
            &labels,
            r#"{"trace_ref":"issue:168","domain":"agent-building","issues_closed":[168],"commits_landed":["2229dc907b"],"outcome_type":"feature","oracle":{"kind":"test-suite","command":"cargo test"},"gradeable":false,"holdout_ok":false,"owner_reviewed":true}"#,
        )
        .unwrap();
        let out = tmp.path().join("drafts");
        run_dataset(
            DatasetArgs {
                action: DatasetAction::Distill {
                    labels,
                    out: Some(out.clone()),
                },
            },
            false,
        )
        .unwrap();
        let draft_path = out.join("issue-168.task.json");
        let draft: Value = serde_json::from_str(&fs::read_to_string(&draft_path).unwrap()).unwrap();
        assert_eq!(draft["status"], "draft");
        assert_eq!(draft["gradeable"], false);
        assert_eq!(draft["schema"], "openagents.gym.task_draft.v1");
        assert!(
            draft["instruction"]
                .as_str()
                .unwrap()
                .contains("do not copy a historical solution"),
            "{}",
            draft["instruction"]
        );
    }
}
