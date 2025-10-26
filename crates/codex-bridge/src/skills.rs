use anyhow::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use serde_yaml as yaml;
use jsonschema::JSONSchema;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillMeta {
    pub license: Option<String>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub meta: SkillMeta,
    /// Origin of the skill: "user" (~/.openagents/skills) or "registry" (repo skills/)
    pub source: Option<String>,
}

/// Resolve the base OpenAgents home directory (env or ~/.openagents).
pub fn openagents_home() -> PathBuf {
    if let Some(p) = std::env::var("OPENAGENTS_HOME").ok() { return PathBuf::from(p); }
    if let Some(home) = std::env::var("HOME").ok() { return PathBuf::from(home).join(".openagents"); }
    PathBuf::from(".openagents")
}

/// Return the user Skills directory path.
pub fn skills_dir() -> PathBuf { openagents_home().join("skills") }

/// Create the Skills directory if missing.
pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(skills_dir()).context("create skills dir")
}

/// Enumerate all skills from user and registry folders (dedup by id).
pub fn list_skills() -> Result<Vec<Skill>> {
    let mut out: Vec<Skill> = Vec::new();
    // 1) User-local skills (~/.openagents/skills)
    let user_dir = skills_dir();
    if !user_dir.exists() { let _ = ensure_dirs(); }
    append_skills_from(&user_dir, "user", &mut out)?;
    // 2) Registry skills inside the repo (skills/ adjacent to repo root)
    for reg in registry_skills_dirs() {
        append_skills_from(&reg, "registry", &mut out)?;
    }
    // De-duplicate by id preferring user over registry
    out.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut dedup: Vec<Skill> = Vec::new();
    for s in out { if seen.insert(s.id.clone()) { dedup.push(s) } }
    Ok(dedup)
}

/// Append skills found under `dir` (source tag applied) into `out`.
fn append_skills_from(dir: &std::path::Path, source: &str, out: &mut Vec<Skill>) -> Result<()> {
    if !dir.exists() { return Ok(()); }
    for ent in fs::read_dir(&dir).context("read skills dir")? {
        let p = ent?.path();
        if !p.is_dir() { continue; }
        let skill_file = p.join("SKILL.md");
        if !skill_file.exists() { continue; }
        if let Some(s) = fs::read_to_string(&skill_file).ok() {
            if let Some(fm) = extract_frontmatter_yaml(&s) {
                if !validate_against_schema(include_str!("../schemas/skill.schema.json"), &fm) { continue; }
                let mut sk = match map_fm_to_skill(&p, &fm) { std::result::Result::Ok(v) => v, _ => continue };
                sk.source = Some(source.to_string());
                out.push(sk);
            }
        }
    }
    Ok(())
}

/// List skills from a specific directory (Claude-compatible folders containing SKILL.md).
/// `source` is tagged onto each skill (e.g., "user", "registry", or "project").
pub fn list_skills_from_dir(dir: &Path, source: &str) -> Result<Vec<Skill>> {
    let mut out: Vec<Skill> = Vec::new();
    append_skills_from(dir, source, &mut out)?;
    out.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn map_fm_to_skill(dir: &Path, fm: &yaml::Value) -> Result<Skill> {
    let id = dir.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
    let name = fm.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let description = fm.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let license = fm.get("license").and_then(|v| v.as_str()).map(|s| s.to_string());
    let allowed_tools = fm.get("allowed-tools").and_then(|v| v.as_sequence()).map(|seq| {
        seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
    });
    let metadata = fm.get("metadata").and_then(|v| serde_json::to_value(v).ok());
    Ok(Skill { id, name, description, meta: SkillMeta { license, allowed_tools, metadata }, source: None })
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

/// Candidate registry skills directories: env override or ./skills in repo.
pub fn registry_skills_dirs() -> Vec<PathBuf> {
    // Priority: explicit env, then ./skills under current working directory if present
    let mut out = Vec::new();
    if let std::result::Result::Ok(p) = std::env::var("OPENAGENTS_REGISTRY_SKILLS_DIR") {
        let path = PathBuf::from(p);
        if path.is_dir() { out.push(path); }
    }
    if let std::result::Result::Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("skills");
        if p.is_dir() { out.push(p); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, folder: &str, yaml_fm: &str, body: &str) -> PathBuf {
        let root = dir.join(folder);
        fs::create_dir_all(&root).expect("mk skill dir");
        let p = root.join("SKILL.md");
        let content = format!("---\n{}\n---\n\n{}\n", yaml_fm, body);
        fs::write(&p, content).expect("write skill");
        root
    }

    #[test]
    fn list_skills_from_dir_parses_valid_skill() {
        let td = tempfile::tempdir().unwrap();
        let yaml = r#"
name: hello-world
description: Hello skill
license: MIT
allowed-tools: [fs, shell]
metadata:
  level: 1
"#;
        write_skill(td.path(), "hello-world", yaml, "# Body\nContent");
        let items = list_skills_from_dir(td.path(), "user").expect("list ok");
        assert_eq!(items.len(), 1);
        let s = &items[0];
        assert_eq!(s.id, "hello-world");
        assert_eq!(s.name, "hello-world");
        assert_eq!(s.description, "Hello skill");
        assert_eq!(s.source.as_deref(), Some("user"));
        assert_eq!(s.meta.license.as_deref(), Some("MIT"));
        assert_eq!(s.meta.allowed_tools.as_ref().map(|v| v.len()), Some(2));
        assert!(s.meta.metadata.is_some());
    }

    #[test]
    fn list_skills_from_dir_skips_invalid_schema() {
        let td = tempfile::tempdir().unwrap();
        // Missing/invalid name (fails kebab-case pattern)
        let yaml = r#"
name: NotKebab
description: Desc
"#;
        write_skill(td.path(), "NotKebab", yaml, "");
        let items = list_skills_from_dir(td.path(), "user").expect("list ok");
        assert_eq!(items.len(), 0, "invalid skill should be skipped");
    }
}
//! File‑backed Skills model (FS <-> structs) for bridge/clients.
//!
//! Reads skill folders containing `SKILL.md` from user and registry locations,
//! validates YAML frontmatter against the schema, and exposes list helpers.
