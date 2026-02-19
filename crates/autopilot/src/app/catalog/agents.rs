use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AgentModel {
    Default,
    Mini,
    Reasoning,
    Inherit,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentDefinition {
    pub(crate) description: String,
    #[allow(dead_code)]
    pub(crate) prompt: String,
    pub(crate) tools: Option<Vec<String>>,
    pub(crate) disallowed_tools: Option<Vec<String>>,
    pub(crate) model: Option<AgentModel>,
    #[allow(dead_code)]
    pub(crate) critical_system_reminder_experimental: Option<String>,
}

use super::super::parsing::{frontmatter_list, frontmatter_scalar, parse_frontmatter};
use super::super::sanitize_tokens;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AgentSource {
    Project,
    User,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentEntry {
    pub(crate) name: String,
    pub(crate) definition: AgentDefinition,
    pub(crate) source: AgentSource,
    pub(crate) created_at: Option<u64>,
}

pub(crate) struct AgentCatalog {
    pub(crate) entries: Vec<AgentEntry>,
    pub(crate) error: Option<String>,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) user_path: Option<PathBuf>,
}

fn parse_agent_model(value: &str) -> Option<AgentModel> {
    match value.trim().to_ascii_lowercase().as_str() {
        "default" => Some(AgentModel::Default),
        "mini" => Some(AgentModel::Mini),
        "reasoning" => Some(AgentModel::Reasoning),
        "inherit" => Some(AgentModel::Inherit),
        _ => None,
    }
}

fn agent_project_dir(cwd: &Path) -> PathBuf {
    cwd.join(".openagents").join("agents")
}

fn agent_user_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".openagents").join("agents"))
}

fn file_timestamp(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

pub(crate) fn load_agent_entries(cwd: &Path) -> AgentCatalog {
    let project_dir = agent_project_dir(cwd);
    let user_dir = agent_user_dir();
    let mut errors = Vec::new();
    let mut map: HashMap<String, AgentEntry> = HashMap::new();

    if let Some(user_dir) = user_dir.as_ref() {
        for entry in load_agent_dir(user_dir, AgentSource::User, &mut errors) {
            map.insert(entry.name.clone(), entry);
        }
    }
    for entry in load_agent_dir(&project_dir, AgentSource::Project, &mut errors) {
        map.insert(entry.name.clone(), entry);
    }

    let mut entries: Vec<AgentEntry> = map.into_values().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    AgentCatalog {
        entries,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join(" | "))
        },
        project_path: Some(project_dir),
        user_path: user_dir,
    }
}

fn load_agent_dir(dir: &Path, source: AgentSource, errors: &mut Vec<String>) -> Vec<AgentEntry> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            errors.push(format!("Failed to read {}: {}", dir.display(), err));
            return Vec::new();
        }
    };

    let mut agents = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                errors.push(format!("Failed to read agent entry: {}", err));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_md = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        match parse_agent_file(&path, source) {
            Ok(Some(agent)) => agents.push(agent),
            Ok(None) => {}
            Err(err) => errors.push(err),
        }
    }
    agents
}

fn parse_agent_file(path: &Path, source: AgentSource) -> Result<Option<AgentEntry>, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
    let (frontmatter, body) = parse_frontmatter(&content);
    let name = frontmatter_scalar(&frontmatter, "name")
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
        })
        .unwrap_or_default();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(format!("Agent file {} missing name.", path.display()));
    }

    let description = frontmatter_scalar(&frontmatter, "description")
        .unwrap_or_else(|| format!("Agent {}", name));
    let prompt = body.trim();
    let prompt = if !prompt.is_empty() {
        prompt.to_string()
    } else if !description.trim().is_empty() {
        description.clone()
    } else {
        format!("You are {}.", name)
    };

    let tools = frontmatter_list(&frontmatter, "tools")
        .or_else(|| frontmatter_list(&frontmatter, "allowed_tools"))
        .map(sanitize_tokens)
        .filter(|list| !list.is_empty());
    let disallowed_tools = frontmatter_list(&frontmatter, "disallowed_tools")
        .map(sanitize_tokens)
        .filter(|list| !list.is_empty());
    let model =
        frontmatter_scalar(&frontmatter, "model").and_then(|value| parse_agent_model(&value));

    let definition = AgentDefinition {
        description: description.clone(),
        prompt,
        tools,
        disallowed_tools,
        model,
        critical_system_reminder_experimental: None,
    };

    Ok(Some(AgentEntry {
        name,
        definition,
        source,
        created_at: file_timestamp(path),
    }))
}
