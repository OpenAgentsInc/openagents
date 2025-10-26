//! File‑backed Projects model (FS <-> structs) for bridge/clients.
//!
//! Reads and writes Project folders under `~/.openagents/projects`, validating
//! YAML frontmatter in `PROJECT.md` against the canonical JSON schema.

use anyhow::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use serde_yaml as yaml;
use jsonschema::JSONSchema;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRepo { pub provider: Option<String>, pub remote: Option<String>, pub url: Option<String>, pub branch: Option<String> }

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTodo { pub text: String, pub completed: bool }

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub working_dir: String,
    pub repo: Option<ProjectRepo>,
    pub agent_file: Option<String>,
    pub instructions: Option<String>,
    pub todos: Option<Vec<ProjectTodo>>,
    pub approvals: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

/// Resolve the base OpenAgents home directory (env or ~/.openagents).
pub fn openagents_home() -> PathBuf {
    if let Some(p) = std::env::var("OPENAGENTS_HOME").ok() { return PathBuf::from(p); }
    if let Some(home) = std::env::var("HOME").ok() { return PathBuf::from(home).join(".openagents"); }
    PathBuf::from(".openagents")
}

/// Return the user Projects directory path.
pub fn projects_dir() -> PathBuf { openagents_home().join("projects") }

/// Create the Projects directory if missing.
pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(projects_dir()).context("create projects dir")
}

/// Enumerate all Projects, supporting both folder (`PROJECT.md`) and
/// legacy single‑file (`*.project.md`) formats.
pub fn list_projects() -> Result<Vec<Project>> {
    let dir = projects_dir();
    let mut out: Vec<Project> = Vec::new();
    if !dir.exists() { ensure_dirs()?; return Ok(out); }
    for ent in fs::read_dir(&dir).context("read projects dir")? {
        let p = ent?.path();
        if p.is_dir() {
            let proj_md = p.join("PROJECT.md");
            if !proj_md.exists() { continue; }
            if let Some(s) = fs::read_to_string(&proj_md).ok() {
                if let Some(fm) = extract_frontmatter_yaml(&s) {
                    if !validate_against_schema(include_str!("../schemas/project.schema.json"), &fm) { continue; }
                    let mut pr = match parse_project_frontmatter(&fm) { std::result::Result::Ok(pr) => pr, _ => continue };
                    // id = directory name
                    if pr.id.trim().is_empty() {
                        pr.id = p.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
                    }
                    out.push(pr);
                }
            }
        } else {
            // Backward-compat: support single-file .project.md
            let is_project_file = p
                .file_name()
                .and_then(|x| x.to_str())
                .map(|n| n.ends_with(".project.md") || n.ends_with(".md"))
                .unwrap_or(false);
            if !is_project_file { continue; }
            if let Some(s) = fs::read_to_string(&p).ok() {
                if let Some(fm) = extract_frontmatter_yaml(&s) {
                    if !validate_against_schema(include_str!("../schemas/project.schema.json"), &fm) { continue; }
                    let mut pr = match parse_project_frontmatter(&fm) { std::result::Result::Ok(pr) => pr, _ => continue };
                    if pr.id.trim().is_empty() {
                        let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
                        let base = name.strip_suffix(".project.md").or_else(|| name.strip_suffix(".md")).unwrap_or(name);
                        pr.id = base.to_string();
                    }
                    out.push(pr);
                }
            }
        }
    }
    out.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Save or create a Project folder with validated `PROJECT.md` frontmatter.
pub fn save_project(p: &Project) -> Result<()> {
    ensure_dirs()?;
    let dir = projects_dir().join(&p.id);
    fs::create_dir_all(&dir).context("create project dir")?;
    let path = dir.join("PROJECT.md");
    let s = render_project_markdown(p);
    if let Some(fm) = extract_frontmatter_yaml(&s) {
        if !validate_against_schema(include_str!("../schemas/project.schema.json"), &fm) {
            bail!("project frontmatter does not conform to schema");
        }
    }
    fs::write(path, s).context("write project file")
}

/// Delete a Project by id (folder if present, else legacy single‑file fallback).
pub fn delete_project(id: &str) -> Result<()> {
    // Prefer directory removal; fallback to file removal for legacy
    let dir = projects_dir().join(id);
    if dir.exists() && dir.is_dir() {
        fs::remove_dir_all(&dir).context("remove project dir")?;
        return Ok(());
    }
    let candidates = [
        projects_dir().join(format!("{}.project.md", id)),
        projects_dir().join(format!("{}.md", id)),
    ];
    for path in candidates.iter() {
        if path.exists() { fs::remove_file(path).context("remove project file")?; return Ok(()); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_projects_dir() {
        let td = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("OPENAGENTS_HOME", td.path().to_string_lossy().to_string()); }
    ensure_dirs().unwrap();
        // write one
        let p = Project { id: "alpha".into(), name: "Alpha".into(), working_dir: "/tmp/x".into(), repo: None, agent_file: None, instructions: Some("Custom".into()), todos: None, approvals: None, model: None, sandbox: None, created_at: None, updated_at: None };
        save_project(&p).unwrap();
        let items = list_projects().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "alpha");
        assert_eq!(items[0].instructions.as_deref(), Some("Custom"));
        // delete
        delete_project("alpha").unwrap();
        let items2 = list_projects().unwrap();
        assert_eq!(items2.len(), 0);
    }
}

fn parse_project_frontmatter(fm: &yaml::Value) -> Result<Project> {
    let mut p = Project {
        id: String::new(),
        name: fm.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        working_dir: fm.get("workingDir").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        repo: None,
        agent_file: fm.get("agentFile").and_then(|v| v.as_str()).map(|s| s.to_string()),
        instructions: fm.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
        todos: None, approvals: None, model: None, sandbox: None, created_at: None, updated_at: None,
    };
    if let Some(r) = fm.get("repo") {
        let pr = ProjectRepo {
            provider: r.get("provider").and_then(|v| v.as_str()).map(|s| s.to_string()),
            remote: r.get("remote").and_then(|v| v.as_str()).map(|s| s.to_string()),
            url: r.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            branch: r.get("branch").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        p.repo = Some(pr);
    }
    Ok(p)
}

fn render_project_markdown(p: &Project) -> String {
    // Minimal skill header; body includes placeholders the app/agent can edit later
    let mut fm = yaml::Mapping::new();
    fm.insert(yaml::Value::String("name".into()), yaml::Value::String(p.name.clone()));
    if let Some(desc) = &p.instructions { fm.insert(yaml::Value::String("description".into()), yaml::Value::String(desc.clone())); }
    fm.insert(yaml::Value::String("workingDir".into()), yaml::Value::String(p.working_dir.clone()));
    if let Some(repo) = &p.repo {
        let mut m = yaml::Mapping::new();
        if let Some(x) = &repo.provider { m.insert(yaml::Value::String("provider".into()), yaml::Value::String(x.clone())); }
        if let Some(x) = &repo.remote { m.insert(yaml::Value::String("remote".into()), yaml::Value::String(x.clone())); }
        if let Some(x) = &repo.url { m.insert(yaml::Value::String("url".into()), yaml::Value::String(x.clone())); }
        if let Some(x) = &repo.branch { m.insert(yaml::Value::String("branch".into()), yaml::Value::String(x.clone())); }
        fm.insert(yaml::Value::String("repo".into()), yaml::Value::Mapping(m));
    }
    let header = format!("---\n{}\n---\n", yaml::to_string(&yaml::Value::Mapping(fm)).unwrap_or_default());
    let body = "\n## Overview\n\nDescribe the project here.\n\n## Workflow\n\n1. Step one\n2. Step two\n";
    format!("{}{}", header, body)
}

fn extract_frontmatter_yaml(s: &str) -> Option<yaml::Value> {
    let mut lines = s.lines();
    if lines.next()?.trim() != "---" { return None; }
    let mut buf: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim() == "---" { break; }
        buf.push(line);
    }
    yaml::from_str(&buf.join("\n")).ok()
}

fn validate_against_schema(schema_json: &str, yaml_val: &yaml::Value) -> bool {
    let json_val: serde_json::Value = if let Some(v) = serde_json::to_value(yaml_val).ok() { v } else { return false };
    let schema_val: serde_json::Value = if let Some(v) = serde_json::from_str(schema_json).ok() { v } else { return false };
    if let Some(compiled) = JSONSchema::compile(&schema_val).ok() { compiled.is_valid(&json_val) } else { false }
}