use anyhow::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use serde_yaml as yaml;

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
    pub voice_aliases: Option<Vec<String>>,
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

pub fn openagents_home() -> PathBuf {
    if let Some(p) = std::env::var("OPENAGENTS_HOME").ok() { return PathBuf::from(p); }
    if let Some(home) = std::env::var("HOME").ok() { return PathBuf::from(home).join(".openagents"); }
    PathBuf::from(".openagents")
}

pub fn projects_dir() -> PathBuf { openagents_home().join("projects") }

pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(projects_dir()).context("create projects dir")
}

pub fn list_projects() -> Result<Vec<Project>> {
    let dir = projects_dir();
    let mut out: Vec<Project> = Vec::new();
    if !dir.exists() { ensure_dirs()?; return Ok(out); }
    for ent in fs::read_dir(&dir).context("read projects dir")? {
        let p = ent?.path();
        if !(p.extension().and_then(|e| e.to_str()) == Some("md") || p.extension().and_then(|e| e.to_str()) == Some("skill.md")) {
            // also allow *.skill.md via ends_with
            if let Some(name) = p.file_name().and_then(|x| x.to_str()) { if !name.ends_with(".skill.md") { continue; } }
        }
        if let Some(s) = fs::read_to_string(&p).ok() {
            if let Some(mut pr) = parse_skill_markdown(&s).ok() {
                if pr.id.trim().is_empty() {
                    let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
                    let base = name.strip_suffix(".skill.md").or_else(|| name.strip_suffix(".md")).unwrap_or(name);
                    let base = base.strip_suffix(".skill").unwrap_or(base);
                    pr.id = base.to_string();
                }
                out.push(pr);
            }
        }
    }
    out.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

pub fn save_project(p: &Project) -> Result<()> {
    ensure_dirs()?;
    let path = projects_dir().join(format!("{}.skill.md", &p.id));
    let s = render_skill_markdown(p);
    fs::write(path, s).context("write project file")
}

pub fn delete_project(id: &str) -> Result<()> {
    // Prefer .skill.md but also fall back to .md
    let candidates = [
        projects_dir().join(format!("{}.skill.md", id)),
        projects_dir().join(format!("{}.md", id)),
        projects_dir().join(format!("{}", id)),
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
        let p = Project { id: "alpha".into(), name: "Alpha".into(), working_dir: "/tmp/x".into(), voice_aliases: None, repo: None, agent_file: None, instructions: Some("Custom".into()), todos: None, approvals: None, model: None, sandbox: None, created_at: None, updated_at: None };
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

fn parse_skill_markdown(s: &str) -> Result<Project> {
    // Expect frontmatter delimited by --- ... --- at top
    let mut lines = s.lines();
    let first = lines.next().unwrap_or("").trim();
    if first != "---" { bail!("missing frontmatter start"); }
    let mut yaml_lines: Vec<&str> = Vec::new();
    for line in lines.by_ref() {
        if line.trim() == "---" { break; }
        yaml_lines.push(line);
    }
    let fm: yaml::Value = yaml::from_str(&yaml_lines.join("\n")).context("parse yaml frontmatter")?;
    // Map YAML keys to Project (camelCase keys in YAML)
    let mut p = Project {
        id: String::new(),
        name: fm.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        working_dir: fm.get("workingDir").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        voice_aliases: fm.get("voiceAliases").and_then(|v| v.as_sequence()).map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()),
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

fn render_skill_markdown(p: &Project) -> String {
    // Minimal skill header; body includes placeholders the app/agent can edit later
    let mut fm = yaml::Mapping::new();
    fm.insert(yaml::Value::String("name".into()), yaml::Value::String(p.name.clone()));
    fm.insert(yaml::Value::String("description".into()), yaml::Value::String(p.instructions.clone().unwrap_or_default()));
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
    let body = "\n## Overview\n\nDescribe the project or skill here.\n\n## Workflow\n\n1. Step one\n2. Step two\n";
    format!("{}{}", header, body)
}
