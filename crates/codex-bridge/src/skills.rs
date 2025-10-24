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
    let dir = skills_dir();
    let mut out: Vec<Skill> = Vec::new();
    if !dir.exists() { ensure_dirs()?; return Ok(out); }
    for ent in fs::read_dir(&dir).context("read skills dir")? {
        let p = ent?.path();
        if !p.is_dir() { continue; }
        let skill_file = p.join("SKILL.md");
        if !skill_file.exists() { continue; }
        if let Some(s) = fs::read_to_string(&skill_file).ok() {
            if let Some(fm) = extract_frontmatter_yaml(&s) {
                // Validate frontmatter against schema (skip invalid)
                if !validate_against_schema(include_str!("../schemas/skill.schema.json"), &fm) {
                    continue;
                }
                let sk = match map_fm_to_skill(&p, &fm) { std::result::Result::Ok(v) => v, _ => continue };
                out.push(sk);
            }
        }
    }
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
    Ok(Skill { id, name, description, meta: SkillMeta { license, allowed_tools, metadata } })
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
