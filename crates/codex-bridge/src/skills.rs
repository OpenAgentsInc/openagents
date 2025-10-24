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

pub fn openagents_home() -> PathBuf {
    if let Some(p) = std::env::var("OPENAGENTS_HOME").ok() { return PathBuf::from(p); }
    if let Some(home) = std::env::var("HOME").ok() { return PathBuf::from(home).join(".openagents"); }
    PathBuf::from(".openagents")
}

pub fn skills_dir() -> PathBuf { openagents_home().join("skills") }

pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(skills_dir()).context("create skills dir")
}

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
