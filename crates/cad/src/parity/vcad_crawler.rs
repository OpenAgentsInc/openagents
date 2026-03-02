use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const VCAD_PARITY_ISSUE_ID: &str = "VCAD-PARITY-002";
pub const VCAD_PARITY_PINNED_COMMIT: &str = "1b59e7948efcdb848d8dba6848785d57aa310e81";

const FEATURE_INDEX_PATH: &str = "docs/features/index.md";
const ROADMAP_PATH: &str = "docs/features/ROADMAP.md";
const CLI_MAIN_PATH: &str = "crates/vcad-cli/src/main.rs";
const WORKSPACE_CARGO_TOML: &str = "Cargo.toml";

#[derive(Debug, thiserror::Error)]
pub enum VcadCrawlerError {
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
    #[error("missing package.name in {member_path}")]
    MissingPackageName { member_path: String },
    #[error("workspace members array not found in Cargo.toml")]
    MissingWorkspaceMembers,
}

pub type VcadCrawlerResult<T> = Result<T, VcadCrawlerError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VcadCapabilityInventory {
    pub manifest_version: u64,
    pub issue_id: String,
    pub crawler: String,
    pub vcad_commit: String,
    pub source_files: Vec<String>,
    pub docs: Vec<VcadDocCapability>,
    pub crates: Vec<VcadCrateCapability>,
    pub commands: Vec<VcadCommandCapability>,
    pub summary: VcadCapabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VcadDocCapability {
    pub source_file: String,
    pub section: String,
    pub capability: String,
    pub status: String,
    pub priority: Option<String>,
    pub effort: Option<String>,
    pub spec: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VcadCrateCapability {
    pub member_path: String,
    pub package_name: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VcadCommandCapability {
    pub variant: String,
    pub cli_command: String,
    pub description: String,
    pub cfg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VcadCapabilitySummary {
    pub docs_capability_count: usize,
    pub docs_status_counts: BTreeMap<String, usize>,
    pub crate_count: usize,
    pub command_count: usize,
}

pub fn crawl_vcad_capabilities(
    vcad_repo: &Path,
    commit: &str,
) -> VcadCrawlerResult<VcadCapabilityInventory> {
    let feature_index = git_show(vcad_repo, commit, FEATURE_INDEX_PATH)?;
    let roadmap = git_show(vcad_repo, commit, ROADMAP_PATH)?;
    let cli_main = git_show(vcad_repo, commit, CLI_MAIN_PATH)?;
    let workspace_cargo = git_show(vcad_repo, commit, WORKSPACE_CARGO_TOML)?;

    let mut docs = parse_markdown_capabilities(FEATURE_INDEX_PATH, &feature_index);
    docs.extend(parse_markdown_capabilities(ROADMAP_PATH, &roadmap));
    docs.sort_by(|left, right| {
        left.source_file
            .cmp(&right.source_file)
            .then_with(|| left.section.cmp(&right.section))
            .then_with(|| left.capability.cmp(&right.capability))
            .then_with(|| left.status.cmp(&right.status))
    });

    let member_paths = parse_workspace_members(&workspace_cargo)?;
    let mut crate_capabilities = Vec::with_capacity(member_paths.len());
    for member_path in member_paths {
        let cargo_toml_path = format!("{member_path}/Cargo.toml");
        let member_cargo = git_show(vcad_repo, commit, &cargo_toml_path)?;
        let package_name = parse_package_name(&member_cargo).ok_or_else(|| {
            VcadCrawlerError::MissingPackageName {
                member_path: member_path.clone(),
            }
        })?;
        crate_capabilities.push(VcadCrateCapability {
            member_path,
            category: categorize_package(&package_name).to_string(),
            package_name,
        });
    }
    crate_capabilities.sort_by(|left, right| {
        left.package_name
            .cmp(&right.package_name)
            .then_with(|| left.member_path.cmp(&right.member_path))
    });

    let mut command_capabilities = parse_cli_commands(&cli_main);
    command_capabilities.sort_by(|left, right| {
        left.cli_command
            .cmp(&right.cli_command)
            .then_with(|| left.variant.cmp(&right.variant))
    });

    let mut status_counts = BTreeMap::new();
    for doc in &docs {
        let entry = status_counts.entry(doc.status.clone()).or_insert(0);
        *entry += 1;
    }

    let summary = VcadCapabilitySummary {
        docs_capability_count: docs.len(),
        docs_status_counts: status_counts,
        crate_count: crate_capabilities.len(),
        command_count: command_capabilities.len(),
    };

    Ok(VcadCapabilityInventory {
        manifest_version: 1,
        issue_id: VCAD_PARITY_ISSUE_ID.to_string(),
        crawler: "openagents-cad::parity::vcad_crawler".to_string(),
        vcad_commit: commit.to_string(),
        source_files: vec![
            FEATURE_INDEX_PATH.to_string(),
            ROADMAP_PATH.to_string(),
            WORKSPACE_CARGO_TOML.to_string(),
            CLI_MAIN_PATH.to_string(),
        ],
        docs,
        crates: crate_capabilities,
        commands: command_capabilities,
        summary,
    })
}

fn git_show(repo: &Path, commit: &str, path: &str) -> VcadCrawlerResult<String> {
    let selector = format!("{commit}:{path}");
    let args = vec!["show".to_string(), selector];
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg(format!("{commit}:{path}"))
        .output()?;
    if !output.status.success() {
        return Err(VcadCrawlerError::GitFailed {
            repo: repo.display().to_string(),
            args,
            stderr: String::from_utf8(output.stderr)?,
        });
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn parse_markdown_capabilities(source_file: &str, markdown: &str) -> Vec<VcadDocCapability> {
    let mut rows = Vec::new();
    let mut headings: Vec<(usize, String)> = Vec::new();
    let mut table_header: Option<Vec<String>> = None;
    let mut waiting_for_separator = false;

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some((level, heading_text)) = parse_heading(trimmed) {
            while headings
                .last()
                .is_some_and(|(existing_level, _)| *existing_level >= level)
            {
                headings.pop();
            }
            headings.push((level, heading_text));
            table_header = None;
            waiting_for_separator = false;
            continue;
        }

        if !trimmed.starts_with('|') {
            table_header = None;
            waiting_for_separator = false;
            continue;
        }

        let cells = split_table_row(trimmed);
        if cells.is_empty() {
            table_header = None;
            waiting_for_separator = false;
            continue;
        }

        match (&table_header, waiting_for_separator) {
            (None, _) => {
                table_header = Some(cells);
                waiting_for_separator = true;
            }
            (Some(_), true) => {
                if is_separator_row(&cells) {
                    waiting_for_separator = false;
                } else {
                    table_header = None;
                }
            }
            (Some(header), false) => {
                if let Some(capability) =
                    map_capability_row(source_file, current_section(&headings), header, &cells)
                {
                    rows.push(capability);
                }
            }
        }
    }

    rows
}

fn parse_workspace_members(workspace_toml: &str) -> VcadCrawlerResult<Vec<String>> {
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
        return Err(VcadCrawlerError::MissingWorkspaceMembers);
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

fn categorize_package(package_name: &str) -> &'static str {
    if package_name == "vcad-kernel" || package_name.starts_with("vcad-kernel-") {
        return "kernel";
    }
    if package_name == "vcad-crdt" {
        return "collaboration";
    }
    if package_name.starts_with("vcad-ecad-") {
        return "ecad";
    }
    if package_name.starts_with("vcad-slicer") {
        return "slicer";
    }
    if package_name.starts_with("vcad-embroidery") {
        return "embroidery";
    }
    if package_name == "vcad-kernel-cam" || package_name == "vcad-kernel-stocksim" {
        return "cam";
    }
    if package_name == "vcad-sim" {
        return "simulation";
    }
    if package_name == "vcad-cli"
        || package_name == "vcad-app"
        || package_name == "vcad-desktop"
        || package_name == "termview"
    {
        return "app";
    }
    if package_name == "wasmosis"
        || package_name == "wasmosis-macro"
        || package_name == "stepperoni"
    {
        return "support";
    }
    "cad-core"
}

fn parse_cli_commands(main_rs: &str) -> Vec<VcadCommandCapability> {
    let mut commands = Vec::new();
    let mut in_enum = false;
    let mut enum_depth: usize = 0;
    let mut docs_buffer: Vec<String> = Vec::new();
    let mut pending_cfg: Option<String> = None;

    for line in main_rs.lines() {
        let trimmed = line.trim();
        if !in_enum {
            if trimmed.contains("enum Commands {") {
                in_enum = true;
                enum_depth = 1;
            }
            continue;
        }

        if enum_depth == 1 {
            if let Some(doc) = trimmed.strip_prefix("///") {
                docs_buffer.push(doc.trim().to_string());
            } else if trimmed.starts_with("#[cfg(") {
                pending_cfg = Some(trimmed.to_string());
            } else if let Some(variant) = parse_command_variant_name(trimmed) {
                let description = docs_buffer.join(" ");
                commands.push(VcadCommandCapability {
                    cli_command: to_kebab_case(&variant),
                    variant,
                    description,
                    cfg: pending_cfg.take(),
                });
                docs_buffer.clear();
            } else if !trimmed.is_empty() && !trimmed.starts_with("#[") {
                docs_buffer.clear();
                pending_cfg = None;
            }
        }

        enum_depth = adjust_brace_depth(enum_depth, line);
        if enum_depth == 0 {
            break;
        }
    }

    commands
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let mut level = 0;
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
    let heading_text = line[level..].trim();
    if heading_text.is_empty() {
        return None;
    }
    Some((level, heading_text.to_string()))
}

fn split_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn is_separator_row(cells: &[String]) -> bool {
    cells.iter().all(|cell| {
        !cell.is_empty()
            && cell
                .chars()
                .all(|char| char == '-' || char == ':' || char == ' ')
    })
}

fn current_section(headings: &[(usize, String)]) -> String {
    let labels: Vec<&str> = headings
        .iter()
        .filter_map(|(level, title)| {
            if *level >= 2 {
                Some(title.as_str())
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

fn map_capability_row(
    source_file: &str,
    section: String,
    header: &[String],
    row: &[String],
) -> Option<VcadDocCapability> {
    if header.len() != row.len() {
        return None;
    }

    let feature_idx = find_column(header, &["feature", "component"])?;
    let status_idx = find_column(header, &["status"])?;
    let priority_idx = find_column(header, &["priority"]);
    let effort_idx = find_column(header, &["effort"]);
    let spec_idx = find_column(header, &["spec"]);

    let feature_raw = row.get(feature_idx)?.as_str();
    let (feature_label, feature_link) = parse_markdown_link(feature_raw);
    let capability = normalize_markdown_cell(
        feature_label
            .as_deref()
            .unwrap_or(feature_raw)
            .to_string()
            .as_str(),
    );
    if capability.is_empty() {
        return None;
    }

    let status = normalize_markdown_cell(row.get(status_idx)?.as_str());
    let priority = priority_idx
        .and_then(|index| row.get(index))
        .map(|value| normalize_markdown_cell(value))
        .filter(|value| !value.is_empty());
    let effort = effort_idx
        .and_then(|index| row.get(index))
        .map(|value| normalize_markdown_cell(value))
        .filter(|value| !value.is_empty());
    let spec = spec_idx
        .and_then(|index| row.get(index))
        .map(|value| normalize_markdown_cell(value))
        .filter(|value| !value.is_empty())
        .or(feature_link);

    Some(VcadDocCapability {
        source_file: source_file.to_string(),
        section,
        capability,
        status,
        priority,
        effort,
        spec,
    })
}

fn normalize_markdown_cell(value: &str) -> String {
    value
        .trim()
        .trim_matches('`')
        .trim_matches('*')
        .replace("**", "")
        .replace('`', "")
        .trim()
        .to_string()
}

fn parse_markdown_link(value: &str) -> (Option<String>, Option<String>) {
    let trimmed = value.trim();
    let open_bracket = trimmed.find('[');
    let close_bracket = trimmed.find("](");
    let close_paren = trimmed.rfind(')');
    if let (Some(open), Some(close), Some(paren)) = (open_bracket, close_bracket, close_paren) {
        if open < close && close + 2 <= paren {
            let label = trimmed[open + 1..close].trim();
            let target = trimmed[close + 2..paren].trim();
            let label = if label.is_empty() {
                None
            } else {
                Some(label.to_string())
            };
            let target = if target.is_empty() {
                None
            } else {
                Some(target.to_string())
            };
            return (label, target);
        }
    }
    (None, None)
}

fn find_column(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|column| {
        let normalized = column.trim().to_ascii_lowercase();
        names.iter().any(|name| normalized == *name)
    })
}

fn parse_command_variant_name(line: &str) -> Option<String> {
    if line.is_empty() || line.starts_with('/') || line.starts_with('#') {
        return None;
    }
    let token = line
        .split(|char: char| char == '{' || char == ',' || char.is_whitespace())
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

fn to_kebab_case(name: &str) -> String {
    let mut output = String::new();
    let mut previous_lower_or_digit = false;
    for char in name.chars() {
        if char.is_ascii_uppercase() {
            if previous_lower_or_digit {
                output.push('-');
            }
            output.push(char.to_ascii_lowercase());
            previous_lower_or_digit = false;
        } else {
            output.push(char);
            previous_lower_or_digit = char.is_ascii_lowercase() || char.is_ascii_digit();
        }
    }
    output
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
        parse_cli_commands, parse_markdown_capabilities, parse_workspace_members, to_kebab_case,
    };

    #[test]
    fn parse_markdown_capabilities_extracts_feature_and_status() {
        let markdown = r#"
## Feature Index
### Core Modeling (Shipped)
| Feature | Status | Priority | Spec |
|---------|--------|----------|------|
| [Primitives](./primitives.md) | `shipped` | p0 | Box, cylinder |
| [Fillets](./fillets.md) | `partial` | p1 | Needs UI |
"#;

        let rows = parse_markdown_capabilities("docs/features/index.md", markdown);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].capability, "Primitives");
        assert_eq!(rows[0].status, "shipped");
        assert_eq!(rows[0].priority.as_deref(), Some("p0"));
        assert_eq!(rows[0].spec.as_deref(), Some("Box, cylinder"));
    }

    #[test]
    fn parse_workspace_members_reads_multiline_array() {
        let cargo = r#"
[workspace]
members = [
  "crates/vcad",
  "crates/vcad-cli",
]
"#;
        let members = parse_workspace_members(cargo).expect("members should parse");
        assert_eq!(members, vec!["crates/vcad", "crates/vcad-cli"]);
    }

    #[test]
    fn parse_cli_commands_extracts_variant_names_and_cfg() {
        let source = r#"
enum Commands {
    /// Open UI
    Tui {
        file: Option<String>,
    },
    /// Export scene
    Export {
        file: String,
    },
    #[cfg(feature = "print-server")]
    /// Start print relay
    PrintServer {
        port: u16,
    },
}
"#;
        let commands = parse_cli_commands(source);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].cli_command, "tui");
        assert_eq!(commands[1].cli_command, "export");
        assert_eq!(commands[2].cli_command, "print-server");
        assert_eq!(
            commands[2].cfg.as_deref(),
            Some("#[cfg(feature = \"print-server\")]")
        );
    }

    #[test]
    fn to_kebab_case_handles_camel_case() {
        assert_eq!(to_kebab_case("ImportUrdf"), "import-urdf");
        assert_eq!(to_kebab_case("Tui"), "tui");
    }
}
