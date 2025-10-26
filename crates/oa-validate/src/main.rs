use anyhow::*;
use clap::Parser;
use serde_json as json;
use serde_yaml as yaml;
use std::{env, fs, path::{Path, PathBuf}};
use jsonschema::JSONSchema;

/// Validate OpenAgents Project/Skill files (Markdown with YAML frontmatter)
#[derive(Parser, Debug)]
#[command(name = "oa-validate", version, about = "Validate OpenAgents projects/skills frontmatter")] 
struct Opts {
    /// One or more paths to .project.md or .skill.md files
    paths: Vec<String>,
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    // Compile schemas once
    let project_schema = compile_schema(include_str!("../../codex-bridge/schemas/project.schema.json"))
        .context("compile project schema")?;
    let skill_schema = compile_schema(include_str!("../../codex-bridge/schemas/skill.schema.json"))
        .context("compile skill schema")?;

    let mut targets: Vec<PathBuf> = Vec::new();
    if opts.paths.is_empty() {
        // Default: validate all under OPENAGENTS_HOME (or ~/.openagents)/{projects,skills}
        let base = match env::var("OPENAGENTS_HOME") {
            std::result::Result::Ok(root) => PathBuf::from(root),
            Err(_) => {
                let home = env::var("HOME").context("$HOME not set for default scan")?;
                Path::new(&home).join(".openagents")
            }
        };
        // New folder-based formats
        targets.extend(collect_special_files(base.join("projects"), "PROJECT.md")?);
        targets.extend(collect_special_files(base.join("skills"), "SKILL.md")?);
        // Legacy single-file formats
        targets.extend(collect_markdown_files(base.join("projects"), Some(".project.md"))?);
        targets.extend(collect_markdown_files(base.join("skills"), Some(".skill.md"))?);
        if targets.is_empty() {
            println!("No projects or skills found under ~/.openagents");
            return Ok(());
        }
    } else {
        for p in &opts.paths {
            let pth = PathBuf::from(p);
            if pth.is_dir() {
                // If directory directly contains SKILL.md or PROJECT.md, add them
                let skill = pth.join("SKILL.md");
                let project = pth.join("PROJECT.md");
                if skill.exists() { targets.push(skill); continue; }
                if project.exists() { targets.push(project); continue; }
                // Otherwise, scan for legacy single-file formats
                targets.extend(collect_markdown_files(&pth, Some(".project.md"))?);
                targets.extend(collect_markdown_files(&pth, Some(".skill.md"))?);
            } else {
                targets.push(pth);
            }
        }
    }

    let mut ok = 0usize;
    let mut fail = 0usize;
    for p in targets {
        let res = validate_path_with_schemas(&p, &project_schema, &skill_schema);
        if let Err(e) = res {
            eprintln!("ERR {}: {:#}", p.display(), e);
            fail += 1;
        } else {
            println!("OK  {}", p.display());
            ok += 1;
        }
    }
    println!("Summary: {} ok, {} failed", ok, fail);
    if fail > 0 { std::process::exit(1) } else { Ok(()) }
}

fn validate_path_with_schemas(path: &Path, project_schema: &JSONSchema, skill_schema: &JSONSchema) -> Result<()> {
    let s = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let fm = extract_frontmatter_yaml(&s).context("missing or invalid YAML frontmatter (---) at top of file")?;

    // Convert YAML -> JSON for schema validation
    let fm_json: json::Value = serde_json::to_value(&fm).context("convert YAML to JSON value")?;

    // Choose schema by filename suffix; fallback to try-both
    let name = path.file_name().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    if name == "project.md" || name.ends_with(".project.md") {
        validate_against(project_schema, &fm_json).context("Project schema validation failed")?;
        return Ok(());
    }
    if name == "skill.md" || name.ends_with(".skill.md") {
        validate_against(skill_schema, &fm_json).context("Skill schema validation failed")?;
        return Ok(());
    }

    // Fallback: detect by keys
    match detect_kind_str(&fm) {
        Some("project") => validate_against(project_schema, &fm_json).context("Project schema validation failed")?,
        Some("skill") => validate_against(skill_schema, &fm_json).context("Skill schema validation failed")?,
        _ => {
            // Last resort: try both, accept if either passes
            let proj_res = validate_against(project_schema, &fm_json);
            if proj_res.is_ok() { return Ok(()); }
            let skill_res = validate_against(skill_schema, &fm_json);
            if skill_res.is_ok() { return Ok(()); }
            bail!("could not match project or skill schema")
        }
    }
    Ok(())
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

fn detect_kind_str(fm: &yaml::Value) -> Option<&'static str> {
    // Project requires workingDir; Skill requires description
    let has_working = fm.get("workingDir").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
    if has_working { return Some("project"); }
    let has_desc = fm.get("description").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
    if has_desc { return Some("skill"); }
    None
}
fn compile_schema(schema_json: &str) -> Result<JSONSchema> {
    let raw_owned: json::Value = json::from_str(schema_json).context("parse schema JSON")?;
    // Leak the schema JSON to satisfy 'static lifetime required by jsonschema::JSONSchema
    let raw_static: &'static json::Value = Box::leak(Box::new(raw_owned));
    let compiled = JSONSchema::options()
        .compile(raw_static)
        .context("compile JSON Schema")?;
    Ok(compiled)
}

fn validate_against(schema: &JSONSchema, value: &json::Value) -> Result<()> {
    let result = schema.validate(value);
    if let Err(errors) = result {
        let mut msgs = Vec::new();
        for e in errors {
            let path = e.instance_path.to_string();
            let msg = e.to_string();
            msgs.push(format!("{}: {}", path, msg));
        }
        if msgs.is_empty() {
            bail!("schema validation failed")
        } else {
            bail!("{}", msgs.join("\n"))
        }
    }
    Ok(())
}

fn collect_markdown_files<P: AsRef<Path>>(dir: P, required_suffix: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let dir = dir.as_ref();
    if !dir.exists() { return Ok(out); }
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let ok = match required_suffix {
                Some(sfx) => p.file_name().and_then(|n| n.to_str()).map(|n| n.ends_with(sfx)).unwrap_or(false),
                None => true,
            };
            if ok { out.push(p); }
        }
    }
    Ok(out)
}

fn collect_special_files<P: AsRef<Path>>(dir: P, special_name: &str) -> Result<Vec<PathBuf>> {
    // Collect files named `special_name` in immediate subdirectories of `dir`.
    let mut out = Vec::new();
    let dir = dir.as_ref();
    if !dir.exists() { return Ok(out); }
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            let candidate = p.join(special_name);
            if candidate.exists() && candidate.is_file() { out.push(candidate); }
        }
    }
    Ok(out)
}
//! CLI to validate OpenAgents Project/Skill frontmatter against JSON Schemas.
//!
//! Accepts file paths or scans default `~/.openagents/{projects,skills}` when
//! none are provided. Exits non‑zero if any file fails validation.
