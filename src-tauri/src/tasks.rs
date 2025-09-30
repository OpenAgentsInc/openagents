use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { New, Planned, Running, Paused, Completed, Canceled, Error }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus { #[default] Pending, Running, Done, Skipped, Error }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutonomyBudget {
  pub approvals: String,
  pub sandbox: String,
  pub max_turns: Option<u32>,
  pub max_tokens: Option<u32>,
  pub max_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StopConditions {
  pub done_regex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
  pub turns: u64,
  pub tokens_in: u64,
  pub tokens_out: u64,
  pub wall_clock_minutes: u64,
  #[serde(default)]
  pub retries: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Subtask {
  pub id: String,
  pub title: String,
  pub status: SubtaskStatus,
  #[serde(default)]
  pub inputs: serde_json::Value,
  pub session_id: Option<String>,
  pub rollout_path: Option<String>,
  pub last_error: Option<String>,
  #[serde(default)]
  pub retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta { pub id: String, pub name: String, pub status: TaskStatus, pub updated_at: DateTime<Utc> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
  pub version: u8,
  pub id: String,
  pub name: String,
  pub status: TaskStatus,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  #[serde(default)] pub autonomy_budget: AutonomyBudget,
  #[serde(default)] pub stop_conditions: StopConditions,
  #[serde(default)] pub queue: Vec<Subtask>,
  #[serde(default)] pub metrics: Metrics,
}

impl Default for Task { fn default() -> Self { Self { version:1, id:String::new(), name:String::new(), status:TaskStatus::New, created_at:Utc::now(), updated_at:Utc::now(), autonomy_budget:AutonomyBudget::default(), stop_conditions:StopConditions::default(), queue:Vec::new(), metrics:Metrics::default() } } }

fn codex_home() -> PathBuf {
  if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return PathBuf::from(val); } }
  dirs::home_dir().map(|mut h| { h.push(".codex"); h }).expect("home dir")
}

fn tasks_root() -> PathBuf { let mut p = codex_home(); p.push("master-tasks"); p }

// Workspace-local fallback used by the Tauri command on create failures.
fn workspace_fallback_tasks_root() -> Option<PathBuf> {
  let cwd = std::env::current_dir().ok()?;
  let run_root = if cwd.ends_with("src-tauri") { cwd.parent().map(|p| p.to_path_buf()).unwrap_or(cwd) } else { cwd };
  Some(run_root.join(".codex-dev").join("master-tasks"))
}

fn task_path(id: &str) -> PathBuf {
  // Choose the newest copy between default root and workspace fallback.
  let mut p_def = tasks_root();
  p_def.push(format!("{}.task.json", id));
  let mut cand_def: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
  if let Ok(meta) = std::fs::metadata(&p_def) { if let Ok(m) = meta.modified() { cand_def = Some((p_def.clone(), m)); } }
  let mut cand_fb: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
  if let Some(mut fb) = workspace_fallback_tasks_root() {
    fb.push(format!("{}.task.json", id));
    if let Ok(meta) = std::fs::metadata(&fb) { if let Ok(m) = meta.modified() { cand_fb = Some((fb.clone(), m)); } }
  }
  match (cand_def, cand_fb) {
    (Some((p1, t1)), Some((p2, t2))) => if t2 > t1 { p2 } else { p1 },
    (Some((p, _)), None) => p,
    (None, Some((p, _))) => p,
    _ => p_def,
  }
}

pub fn tasks_list() -> anyhow::Result<Vec<TaskMeta>> {
  use std::collections::HashMap;
  let mut by_id: HashMap<String, TaskMeta> = HashMap::new();
  // Search default root first, then workspace fallback
  let mut roots: Vec<PathBuf> = vec![tasks_root()];
  if let Some(fb) = workspace_fallback_tasks_root() { roots.push(fb); }
  for root in roots {
    if !root.exists() { continue; }
    for ent in fs::read_dir(root)? {
      let ent = ent?;
      if ent.file_type()?.is_file() {
        if let Ok(text) = fs::read_to_string(ent.path()) {
          if let Ok(task) = serde_json::from_str::<Task>(&text) {
            let meta = TaskMeta { id: task.id.clone(), name: task.name.clone(), status: task.status.clone(), updated_at: task.updated_at };
            match by_id.get(&meta.id) {
              Some(old) if old.updated_at >= meta.updated_at => {}
              _ => { by_id.insert(meta.id.clone(), meta); }
            }
          }
        }
      }
    }
  }
  let mut out: Vec<TaskMeta> = by_id.into_values().collect();
  out.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
  Ok(out)
}

pub fn task_get(id: &str) -> anyhow::Result<Task> { let text = fs::read_to_string(task_path(id))?; Ok(serde_json::from_str::<Task>(&text)?) }

pub fn task_delete(id: &str) -> anyhow::Result<()> { let p = task_path(id); if p.exists() { fs::remove_file(p)?; } Ok(()) }

fn atomic_write(path: &Path, text: &str) -> anyhow::Result<()> {
  if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
  let mut tmp = path.to_path_buf(); tmp.set_extension("task.json.tmp");
  { let mut f = fs::File::create(&tmp)?; f.write_all(text.as_bytes())?; f.sync_all()?; }
  fs::rename(tmp, path)?; Ok(())
}

pub fn task_update(mut t: Task) -> anyhow::Result<Task> { t.updated_at = Utc::now(); let text = serde_json::to_string_pretty(&t)?; atomic_write(&task_path(&t.id), &text)?; Ok(t) }

pub fn task_create(name: &str, settings: AutonomyBudget) -> anyhow::Result<Task> {
  let t = Task { version:1, id:Uuid::new_v4().to_string(), name:name.to_string(), status:TaskStatus::New, created_at:Utc::now(), updated_at:Utc::now(), autonomy_budget:settings, stop_conditions:StopConditions::default(), queue:Vec::new(), metrics:Metrics::default() };
  task_update(t)
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;
  use std::thread;
  use std::time::Duration;
  use std::sync::OnceLock;

  static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

  #[test]
  fn create_list_load_delete() {
    let _guard = ENV_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
    let td = TempDir::new().unwrap();
    std::env::set_var("CODEX_HOME", td.path().display().to_string());
    let a = task_create("Test Task", AutonomyBudget { approvals:"never".into(), sandbox:"danger-full-access".into(), max_turns:Some(10), max_tokens:None, max_minutes:None }).unwrap();
    assert!(!a.id.is_empty());
    let list = tasks_list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, a.id);
    let mut loaded = task_get(&a.id).unwrap();
    assert_eq!(loaded.name, "Test Task");
    loaded.name = "Renamed".into();
    let _saved = task_update(loaded).unwrap();
    assert_eq!(task_get(&a.id).unwrap().name, "Renamed");
    task_delete(&a.id).unwrap();
    assert!(tasks_list().unwrap().is_empty());
  }

  #[test]
  fn list_sorts_by_updated_desc() {
    let _guard = ENV_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
    let td = TempDir::new().unwrap();
    std::env::set_var("CODEX_HOME", td.path().display().to_string());
    let a = task_create("A", AutonomyBudget::default()).unwrap();
    // ensure distinct timestamps
    thread::sleep(Duration::from_millis(5));
    let b = task_create("B", AutonomyBudget::default()).unwrap();
    // touch A to be newest
    let mut a2 = task_get(&a.id).unwrap();
    a2.name = "A2".into();
    let _ = task_update(a2).unwrap();
    let list = tasks_list().unwrap();
    assert_eq!(list.len(), 2);
    // A2 should be first (newest)
    assert_eq!(list[0].id, a.id);
    assert_eq!(list[1].id, b.id);
  }
}
