use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const OPENAGENTS_PARITY_ISSUE_ID: &str = "VCAD-PARITY-003";
pub const OPENAGENTS_PARITY_PINNED_COMMIT: &str = "04faa5227f077c419f1c5c52ddebbb7552838fd4";

const WORKSPACE_CARGO_TOML: &str = "Cargo.toml";
const CAD_LIB_PATH: &str = "crates/cad/src/lib.rs";
const CAD_INTENT_PATH: &str = "crates/cad/src/intent.rs";
const CAD_DYNAMIC_TOOLS_PATH: &str = "apps/autopilot-desktop/src/openagents_dynamic_tools.rs";

const DOC_SOURCES: [&str; 7] = [
    "crates/cad/docs/PLAN.md",
    "crates/cad/docs/decisions/0001-kernel-strategy.md",
    "crates/cad/docs/CAD_FEATURE_OPS.md",
    "crates/cad/docs/CAD_SKETCH_CONSTRAINTS.md",
    "crates/cad/docs/CAD_SKETCH_FEATURE_OPS.md",
    "crates/cad/docs/CAD_STEP_IMPORT.md",
    "crates/cad/docs/CAD_STEP_EXPORT.md",
];

#[derive(Debug, thiserror::Error)]
pub enum OpenagentsCrawlerError {
    #[error("git command failed in {repo}: {args:?}\n{stderr}")]
    GitFailed {
        repo: String,
        args: Vec<String>,
        stderr: String,
    },
    #[error("utf8 decoding failed for git output: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("workspace members array not found in Cargo.toml")]
    MissingWorkspaceMembers,
    #[error("missing package.name in {member_path}")]
    MissingPackageName { member_path: String },
}

pub type OpenagentsCrawlerResult<T> = Result<T, OpenagentsCrawlerError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenagentsCapabilityInventory {
    pub manifest_version: u64,
    pub issue_id: String,
    pub crawler: String,
    pub openagents_commit: String,
    pub source_files: Vec<String>,
    pub docs: Vec<OpenagentsDocCapability>,
    pub crates: Vec<OpenagentsCrateCapability>,
    pub commands: Vec<OpenagentsCommandCapability>,
    pub summary: OpenagentsCapabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenagentsDocCapability {
    pub source_file: String,
    pub section: String,
    pub capability: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenagentsCrateCapability {
    pub member_path: String,
    pub package_name: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenagentsCommandCapability {
    pub source_file: String,
    pub command: String,
    pub kind: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenagentsCapabilitySummary {
    pub docs_capability_count: usize,
    pub docs_status_counts: BTreeMap<String, usize>,
    pub crate_count: usize,
    pub command_count: usize,
}

pub fn crawl_openagents_capabilities(
    openagents_repo: &Path,
    commit: &str,
) -> OpenagentsCrawlerResult<OpenagentsCapabilityInventory> {
    let workspace_cargo = git_show(openagents_repo, commit, WORKSPACE_CARGO_TOML)?;
    let cad_lib = git_show(openagents_repo, commit, CAD_LIB_PATH)?;
    let cad_intent = git_show(openagents_repo, commit, CAD_INTENT_PATH)?;
    let cad_dynamic_tools = git_show(openagents_repo, commit, CAD_DYNAMIC_TOOLS_PATH)?;

    let mut docs = Vec::new();
    for source_file in DOC_SOURCES {
        let content = git_show(openagents_repo, commit, source_file)?;
        docs.extend(parse_doc_capabilities(source_file, &content));
    }
    docs.sort_by(|left, right| {
        left.source_file
            .cmp(&right.source_file)
            .then_with(|| left.section.cmp(&right.section))
            .then_with(|| left.capability.cmp(&right.capability))
            .then_with(|| left.status.cmp(&right.status))
    });

    let mut crates = parse_workspace_cad_surfaces(openagents_repo, commit, &workspace_cargo)?;
    crates.extend(parse_cad_modules(&cad_lib));
    crates.sort_by(|left, right| {
        left.package_name
            .cmp(&right.package_name)
            .then_with(|| left.member_path.cmp(&right.member_path))
    });

    let mut commands = parse_cad_intents(&cad_intent);
    commands.extend(parse_cad_dynamic_tools(&cad_dynamic_tools));
    commands.sort_by(|left, right| {
        left.command
            .cmp(&right.command)
            .then_with(|| left.kind.cmp(&right.kind))
    });

    let mut status_counts = BTreeMap::new();
    for doc in &docs {
        let count = status_counts.entry(doc.status.clone()).or_insert(0);
        *count += 1;
    }

    let summary = OpenagentsCapabilitySummary {
        docs_capability_count: docs.len(),
        docs_status_counts: status_counts,
        crate_count: crates.len(),
        command_count: commands.len(),
    };

    let mut source_files = vec![
        WORKSPACE_CARGO_TOML.to_string(),
        CAD_LIB_PATH.to_string(),
        CAD_INTENT_PATH.to_string(),
        CAD_DYNAMIC_TOOLS_PATH.to_string(),
    ];
    source_files.extend(DOC_SOURCES.iter().map(|path| (*path).to_string()));

    Ok(OpenagentsCapabilityInventory {
        manifest_version: 1,
        issue_id: OPENAGENTS_PARITY_ISSUE_ID.to_string(),
        crawler: "openagents-cad::parity::openagents_crawler".to_string(),
        openagents_commit: commit.to_string(),
        source_files,
        docs,
        crates,
        commands,
        summary,
    })
}

fn git_show(repo: &Path, commit: &str, path: &str) -> OpenagentsCrawlerResult<String> {
    let selector = format!("{commit}:{path}");
    let args = vec!["show".to_string(), selector];
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg(format!("{commit}:{path}"))
        .output()?;
    if !output.status.success() {
        return Err(OpenagentsCrawlerError::GitFailed {
            repo: repo.display().to_string(),
            args,
            stderr: String::from_utf8(output.stderr)?,
        });
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn parse_doc_capabilities(source_file: &str, markdown: &str) -> Vec<OpenagentsDocCapability> {
    let mut entries = Vec::new();
    let mut headings: Vec<(usize, String)> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some((level, heading)) = parse_heading(trimmed) {
            while headings
                .last()
                .is_some_and(|(existing_level, _)| *existing_level >= level)
            {
                headings.pop();
            }
            headings.push((level, heading.clone()));
            if level >= 2 {
                entries.push(OpenagentsDocCapability {
                    source_file: source_file.to_string(),
                    section: parent_section(&headings),
                    capability: heading,
                    status: infer_heading_status(trimmed),
                });
            }
            continue;
        }

        if let Some((done, capability)) = parse_checkbox_line(trimmed) {
            entries.push(OpenagentsDocCapability {
                source_file: source_file.to_string(),
                section: current_section(&headings),
                capability,
                status: if done { "done" } else { "todo" }.to_string(),
            });
        }
    }

    entries
}

fn parse_workspace_cad_surfaces(
    repo: &Path,
    commit: &str,
    workspace_toml: &str,
) -> OpenagentsCrawlerResult<Vec<OpenagentsCrateCapability>> {
    let members = parse_workspace_members(workspace_toml)?;
    let mut capabilities = Vec::new();
    for member_path in members {
        if !member_path.contains("cad") && member_path != "apps/autopilot-desktop" {
            continue;
        }
        let cargo_toml_path = format!("{member_path}/Cargo.toml");
        let cargo_toml = git_show(repo, commit, &cargo_toml_path)?;
        let package_name = parse_package_name(&cargo_toml).ok_or_else(|| {
            OpenagentsCrawlerError::MissingPackageName {
                member_path: member_path.clone(),
            }
        })?;
        capabilities.push(OpenagentsCrateCapability {
            member_path: member_path.clone(),
            package_name: package_name.clone(),
            category: categorize_workspace_package(&package_name).to_string(),
        });
    }
    Ok(capabilities)
}

fn parse_cad_modules(lib_rs: &str) -> Vec<OpenagentsCrateCapability> {
    let mut capabilities = Vec::new();
    for line in lib_rs.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub mod ") || !trimmed.ends_with(';') {
            continue;
        }
        let module_name = trimmed
            .trim_start_matches("pub mod ")
            .trim_end_matches(';')
            .trim();
        if module_name.is_empty() {
            continue;
        }
        capabilities.push(OpenagentsCrateCapability {
            member_path: format!("crates/cad/src/{module_name}.rs"),
            package_name: format!("openagents-cad::{module_name}"),
            category: "cad-module".to_string(),
        });
    }
    capabilities
}

fn parse_cad_intents(intent_rs: &str) -> Vec<OpenagentsCommandCapability> {
    let mut commands = Vec::new();
    let mut in_enum = false;
    let mut enum_depth = 0usize;
    for line in intent_rs.lines() {
        let trimmed = line.trim();
        if !in_enum {
            if trimmed.contains("pub enum CadIntent {") {
                in_enum = true;
                enum_depth = 1;
            }
            continue;
        }
        if enum_depth == 1
            && let Some(variant) = parse_variant_name(trimmed)
        {
            commands.push(OpenagentsCommandCapability {
                source_file: CAD_INTENT_PATH.to_string(),
                command: variant.clone(),
                kind: "cad_intent".to_string(),
                detail: Some(to_kebab_case(&variant)),
            });
        }
        enum_depth = adjust_brace_depth(enum_depth, line);
        if enum_depth == 0 {
            break;
        }
    }
    commands
}

fn parse_cad_dynamic_tools(source: &str) -> Vec<OpenagentsCommandCapability> {
    let mut commands = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("OPENAGENTS_TOOL_CAD_") || !trimmed.contains('"') {
            continue;
        }
        let mut quoted = extract_quoted_values(trimmed).into_iter();
        let Some(tool_name) = quoted.next() else {
            continue;
        };
        commands.push(OpenagentsCommandCapability {
            source_file: CAD_DYNAMIC_TOOLS_PATH.to_string(),
            command: tool_name,
            kind: "dynamic_tool".to_string(),
            detail: None,
        });
    }
    commands
}

fn parse_workspace_members(workspace_toml: &str) -> OpenagentsCrawlerResult<Vec<String>> {
    let mut in_members = false;
    let mut members = Vec::new();
    let mut found_members = false;

    for line in workspace_toml.lines() {
        let trimmed = line.trim();
        if !in_members {
            if trimmed.starts_with("members") && trimmed.contains('[') {
                in_members = true;
                found_members = true;
                members.extend(extract_quoted_values(trimmed));
                if trimmed.contains(']') {
                    in_members = false;
                }
            }
            continue;
        }

        if trimmed.starts_with(']') {
            in_members = false;
            continue;
        }
        members.extend(extract_quoted_values(trimmed));
    }

    if !found_members {
        return Err(OpenagentsCrawlerError::MissingWorkspaceMembers);
    }

    Ok(members)
}

fn parse_package_name(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let mut quoted = extract_quoted_values(trimmed).into_iter();
        if let Some(name) = quoted.next() {
            return Some(name);
        }
    }
    None
}

fn categorize_workspace_package(package_name: &str) -> &'static str {
    if package_name == "openagents-cad" {
        return "cad-core";
    }
    if package_name == "autopilot-desktop" {
        return "cad-app";
    }
    "cad-related"
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let mut level = 0usize;
    for char in line.chars() {
        if char == '#' {
            level += 1;
        } else {
            break;
        }
    }
    if level == 0 {
        return None;
    }
    let heading = line[level..].trim();
    if heading.is_empty() {
        return None;
    }
    Some((level, heading.to_string()))
}

fn parent_section(headings: &[(usize, String)]) -> String {
    if headings.len() <= 1 {
        return "root".to_string();
    }
    let labels: Vec<&str> = headings[..headings.len() - 1]
        .iter()
        .filter_map(|(level, heading)| {
            if *level >= 2 {
                Some(heading.as_str())
            } else {
                None
            }
        })
        .collect();
    if labels.is_empty() {
        return "root".to_string();
    }
    labels.join(" / ")
}

fn current_section(headings: &[(usize, String)]) -> String {
    let labels: Vec<&str> = headings
        .iter()
        .filter_map(|(level, heading)| {
            if *level >= 2 {
                Some(heading.as_str())
            } else {
                None
            }
        })
        .collect();
    if labels.is_empty() {
        return "root".to_string();
    }
    labels.join(" / ")
}

fn infer_heading_status(heading_line: &str) -> String {
    let lower = heading_line.to_ascii_lowercase();
    if lower.contains("(shipped)") || lower.contains("✅") || lower.contains("complete") {
        return "shipped".to_string();
    }
    if lower.contains("(partial)") || lower.contains("partial") {
        return "partial".to_string();
    }
    if lower.contains("(planned)") || lower.contains("planned") {
        return "planned".to_string();
    }
    if lower.contains("(proposed)") || lower.contains("proposed") {
        return "proposed".to_string();
    }
    if lower.contains("in progress") {
        return "in-progress".to_string();
    }
    if lower.contains("not started") {
        return "not-started".to_string();
    }
    "documented".to_string()
}

fn parse_checkbox_line(line: &str) -> Option<(bool, String)> {
    let mut cursor = line.trim();
    if cursor.starts_with("- ") || cursor.starts_with("* ") {
        cursor = &cursor[2..];
    } else {
        let mut idx = 0usize;
        let chars: Vec<char> = cursor.chars().collect();
        while idx < chars.len() && chars[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx > 0 && idx + 1 < chars.len() && chars[idx] == '.' && chars[idx + 1] == ' ' {
            cursor = &cursor[(idx + 2)..];
        }
    }

    if let Some(rest) = cursor.strip_prefix("[x] ") {
        return Some((true, rest.trim().to_string()));
    }
    if let Some(rest) = cursor.strip_prefix("[X] ") {
        return Some((true, rest.trim().to_string()));
    }
    if let Some(rest) = cursor.strip_prefix("[ ] ") {
        return Some((false, rest.trim().to_string()));
    }
    None
}

fn parse_variant_name(line: &str) -> Option<String> {
    if line.is_empty() || line.starts_with('#') || line.starts_with('/') {
        return None;
    }
    let token = line
        .split(|char: char| {
            char == '{' || char == ',' || char == '(' || char == ')' || char.is_whitespace()
        })
        .find(|token| !token.is_empty())?;
    let first = token.chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    if token
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '_')
    {
        return Some(token.to_string());
    }
    None
}

fn to_kebab_case(name: &str) -> String {
    let mut output = String::new();
    let mut prev_lower_or_digit = false;
    for char in name.chars() {
        if char.is_ascii_uppercase() {
            if prev_lower_or_digit {
                output.push('-');
            }
            output.push(char.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else {
            output.push(char);
            prev_lower_or_digit = char.is_ascii_lowercase() || char.is_ascii_digit();
        }
    }
    output
}

fn adjust_brace_depth(mut depth: usize, line: &str) -> usize {
    for char in line.chars() {
        if char == '{' {
            depth += 1;
        } else if char == '}' {
            depth = depth.saturating_sub(1);
        }
    }
    depth
}

fn extract_quoted_values(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0usize;
    while let Some(start_rel) = line[cursor..].find('"') {
        let start = cursor + start_rel + 1;
        let Some(end_rel) = line[start..].find('"') else {
            break;
        };
        let end = start + end_rel;
        values.push(line[start..end].to_string());
        cursor = end + 1;
    }
    values
}

#[cfg(test)]
mod tests {
    use super::{
        parse_cad_dynamic_tools, parse_cad_intents, parse_checkbox_line, parse_doc_capabilities,
        parse_workspace_members,
    };

    #[test]
    fn parse_doc_capabilities_extracts_headings_and_checkboxes() {
        let markdown = r#"
## Phase A
1. [x] First capability
2. [ ] Second capability
### Detail Section (Shipped)
- [x] Detail capability
"#;
        let entries = parse_doc_capabilities("test.md", markdown);
        assert!(entries.iter().any(|entry| entry.capability == "Phase A"));
        assert!(
            entries
                .iter()
                .any(|entry| entry.capability == "First capability")
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.capability == "Second capability")
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.capability == "Detail Section (Shipped)")
        );
    }

    #[test]
    fn parse_workspace_members_reads_multiline_array() {
        let workspace = r#"
[workspace]
members = [
  "apps/autopilot-desktop",
  "crates/cad",
]
"#;
        let members = parse_workspace_members(workspace).expect("members parse");
        assert_eq!(members, vec!["apps/autopilot-desktop", "crates/cad"]);
    }

    #[test]
    fn parse_checkbox_line_parses_numbered_and_bulleted_items() {
        let first = parse_checkbox_line("1. [x] done").expect("first");
        let second = parse_checkbox_line("- [ ] todo").expect("second");
        assert_eq!(first, (true, "done".to_string()));
        assert_eq!(second, (false, "todo".to_string()));
    }

    #[test]
    fn parse_cad_intents_extracts_variants() {
        let intent_rs = r#"
pub enum CadIntent {
    CreateRackSpec(CreateRackSpecIntent),
    Export(ExportIntent),
}
"#;
        let commands = parse_cad_intents(intent_rs);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command, "CreateRackSpec");
        assert_eq!(commands[0].detail.as_deref(), Some("create-rack-spec"));
    }

    #[test]
    fn parse_cad_dynamic_tools_extracts_tool_constants() {
        let source = r#"
pub(crate) const OPENAGENTS_TOOL_CAD_INTENT: &str = "openagents_cad_intent";
pub(crate) const OPENAGENTS_TOOL_CAD_ACTION: &str = "openagents_cad_action";
"#;
        let tools = parse_cad_dynamic_tools(source);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].command, "openagents_cad_intent");
    }
}
