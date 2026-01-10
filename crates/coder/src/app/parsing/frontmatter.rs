use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct Frontmatter {
    pub(crate) scalars: HashMap<String, String>,
    pub(crate) lists: HashMap<String, Vec<String>>,
}

fn normalize_frontmatter_key(key: &str) -> String {
    key.trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_")
}

fn strip_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0] as char;
        let last = bytes[bytes.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn is_list_key(key: &str) -> bool {
    matches!(
        key,
        "tools"
            | "allowed_tools"
            | "disallowed_tools"
            | "tags"
            | "categories"
            | "capabilities"
            | "skills"
    )
}

fn parse_list_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| strip_quotes(item).trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_inline_list(value: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        return Some(parse_list_values(inner));
    }
    None
}

pub(crate) fn parse_frontmatter(contents: &str) -> (Frontmatter, String) {
    let mut frontmatter = Frontmatter::default();
    let mut lines = contents.lines();
    let Some(first) = lines.next() else {
        return (frontmatter, contents.to_string());
    };
    if first.trim() != "---" {
        return (frontmatter, contents.to_string());
    }

    let mut frontmatter_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut in_frontmatter = true;

    for line in lines {
        if in_frontmatter && line.trim() == "---" {
            in_frontmatter = false;
            continue;
        }
        if in_frontmatter {
            frontmatter_lines.push(line);
        } else {
            body_lines.push(line);
        }
    }

    if in_frontmatter {
        return (Frontmatter::default(), contents.to_string());
    }

    let mut current_list_key: Option<String> = None;
    for raw_line in frontmatter_lines {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let stripped = line.trim_start();
        if stripped.starts_with('-') {
            if let Some(key) = current_list_key.as_ref() {
                let item = stripped.trim_start_matches('-').trim();
                if !item.is_empty() {
                    frontmatter
                        .lists
                        .entry(key.clone())
                        .or_default()
                        .push(strip_quotes(item));
                }
                continue;
            }
        }
        current_list_key = None;
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = normalize_frontmatter_key(raw_key);
        let value = raw_value.trim();
        if value.is_empty() {
            current_list_key = Some(key.clone());
            frontmatter.lists.entry(key).or_default();
            continue;
        }
        if let Some(list) = parse_inline_list(value) {
            frontmatter.lists.insert(key, list);
            continue;
        }
        if is_list_key(&key) {
            frontmatter.lists.insert(key, parse_list_values(value));
        } else {
            frontmatter
                .scalars
                .insert(key, strip_quotes(value));
        }
    }

    (frontmatter, body_lines.join("\n"))
}

pub(crate) fn frontmatter_scalar(frontmatter: &Frontmatter, key: &str) -> Option<String> {
    let normalized = normalize_frontmatter_key(key);
    frontmatter.scalars.get(&normalized).cloned()
}

pub(crate) fn frontmatter_list(frontmatter: &Frontmatter, key: &str) -> Option<Vec<String>> {
    let normalized = normalize_frontmatter_key(key);
    frontmatter.lists.get(&normalized).cloned()
}

pub(crate) fn first_nonempty_line(text: &str) -> Option<String> {
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}
