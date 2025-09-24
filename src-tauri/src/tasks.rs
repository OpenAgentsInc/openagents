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
pub struct Metrics { pub turns: u64, pub tokens_in: u64, pub tokens_out: u64, pub wall_clock_minutes: u64 }

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

fn task_path(id: &str) -> PathBuf { let mut p = tasks_root(); p.push(format!("{}.task.json", id)); p }

pub fn tasks_list() -> anyhow::Result<Vec<TaskMeta>> {
  let root = tasks_root();
  let mut out = Vec::new();
  if !root.exists() { return Ok(out); }
  for ent in fs::read_dir(root)? { let ent = ent?; if ent.file_type()?.is_file() {
      if let Ok(text) = fs::read_to_string(ent.path()) { if let Ok(task) = serde_json::from_str::<Task>(&text) {
          out.push(TaskMeta { id: task.id.clone(), name: task.name.clone(), status: task.status.clone(), updated_at: task.updated_at });
      }}
  }}
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

  #[test]
  fn create_list_load_delete() {
    let td = TempDir::new().unwrap();
    std::env::set_var("CODEX_HOME", td.path());
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
}
