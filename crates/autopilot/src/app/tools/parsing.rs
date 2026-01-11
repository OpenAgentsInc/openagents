use serde_json::Value;
use wgpui::components::atoms::ToolType;
use wgpui::components::organisms::{DiffLine, DiffLineKind, SearchMatch};

const TOOL_SEARCH_MATCH_LIMIT: usize = 200;

fn safe_truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn safe_suffix(s: &str, skip_bytes: usize) -> &str {
    if skip_bytes >= s.len() {
        return "";
    }
    let mut start = skip_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

pub(crate) fn format_tool_input(tool_name: &str, json_input: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(json_input) {
        match tool_name {
            "Glob" => {
                if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
                    return pattern.to_string();
                }
            }
            "Grep" => {
                if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
                    return pattern.to_string();
                }
            }
            "Read" => {
                if let Some(path) = value.get("file_path").and_then(|v| v.as_str()) {
                    if path.len() > 60 {
                        return format!("...{}", safe_suffix(path, path.len().saturating_sub(57)));
                    }
                    return path.to_string();
                }
            }
            "Bash" => {
                if let Some(cmd) = value.get("command").and_then(|v| v.as_str()) {
                    if cmd.len() > 80 {
                        return format!("{}...", safe_truncate(cmd, 77));
                    }
                    return cmd.to_string();
                }
            }
            "BashOutput" | "KillBash" => {
                if let Some(id) = value
                    .get("bash_id")
                    .or_else(|| value.get("shell_id"))
                    .and_then(|v| v.as_str())
                {
                    return format!("shell {}", id);
                }
            }
            "Edit" | "Write" => {
                if let Some(path) = value.get("file_path").and_then(|v| v.as_str()) {
                    if path.len() > 60 {
                        return format!("...{}", safe_suffix(path, path.len().saturating_sub(57)));
                    }
                    return path.to_string();
                }
            }
            "WebFetch" => {
                if let Some(url) = value.get("url").and_then(|v| v.as_str()) {
                    return url.to_string();
                }
            }
            "Task" => {
                if let Some(desc) = value.get("description").and_then(|v| v.as_str()) {
                    return desc.to_string();
                }
            }
            "AskUserQuestion" => {
                if let Some(questions) = value.get("questions").and_then(|v| v.as_array()) {
                    let question_texts: Vec<&str> = questions
                        .iter()
                        .filter_map(|q| q.get("question").and_then(|v| v.as_str()))
                        .collect();
                    if !question_texts.is_empty() {
                        let joined = question_texts.join(" | ");
                        if joined.len() > 80 {
                            return format!("{}...", safe_truncate(&joined, 77));
                        }
                        return joined;
                    }
                }
            }
            _ => {}
        }
        let s = json_input.replace('\n', " ");
        if s.len() > 80 {
            return format!("{}...", safe_truncate(&s, 77));
        }
        return s;
    }
    if json_input.len() > 80 {
        format!("{}...", safe_truncate(json_input, 77))
    } else {
        json_input.to_string()
    }
}

pub(crate) fn tool_type_for_name(name: &str) -> ToolType {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "read" => ToolType::Read,
        "write" | "todowrite" => ToolType::Write,
        "edit" | "notebookedit" | "diff" => ToolType::Edit,
        "bash" | "bashoutput" | "killbash" => ToolType::Bash,
        "glob" => ToolType::Glob,
        "grep" => ToolType::Grep,
        "search" => ToolType::Search,
        "list" => ToolType::List,
        "task" => ToolType::Task,
        "webfetch" | "web_fetch" | "fetch" => ToolType::WebFetch,
        _ => ToolType::Unknown,
    }
}

fn extract_exit_code(value: &Value) -> Option<i32> {
    let obj = value.as_object()?;
    let code = obj
        .get("exit_code")
        .or_else(|| obj.get("exitCode"))
        .or_else(|| obj.get("exitcode"))
        .and_then(|v| v.as_i64())?;
    Some(code as i32)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => serde_json::to_string_pretty(value).ok(),
    }
}

pub(crate) fn tool_result_output(
    content: &Value,
    tool_use_result: Option<&Value>,
) -> (String, Option<i32>, Option<Value>) {
    let mut output_value = tool_use_result.cloned();
    let mut exit_code = tool_use_result.and_then(extract_exit_code);
    if exit_code.is_none() {
        exit_code = extract_exit_code(content);
    }

    let mut output = value_to_string(content).unwrap_or_default();
    if output.trim().is_empty() {
        if let Some(result) = tool_use_result {
            if let Some(text) = result.get("output").and_then(|v| v.as_str()) {
                output = text.to_string();
            } else {
                let stdout = result.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                let stderr = result.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                if !stdout.is_empty() || !stderr.is_empty() {
                    if stdout.is_empty() {
                        output = stderr.to_string();
                    } else if stderr.is_empty() {
                        output = stdout.to_string();
                    } else {
                        output = format!("{}\n{}", stdout, stderr);
                    }
                }
            }
        }
    }

    if output.trim().is_empty() {
        if output_value.is_none() {
            if let Some(text) = content.as_str() {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    output_value = Some(parsed);
                }
            } else if !content.is_null() {
                output_value = Some(content.clone());
            }
        }
        if let Some(value) = output_value.as_ref() {
            output = serde_json::to_string_pretty(value).unwrap_or_default();
        }
    }

    let output = truncate_lines(&output, 200, 8_000);
    (output, exit_code, output_value)
}

fn truncate_lines(text: &str, max_lines: usize, max_chars: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }
    let mut result = lines.join("\n");
    if result.len() > max_chars {
        let mut truncate_at = max_chars;
        while truncate_at > 0 && !result.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        result.truncate(truncate_at);
        result.push_str("...");
    }
    result
}

pub(crate) fn parse_search_matches(output_value: Option<&Value>, output: &str) -> Vec<SearchMatch> {
    if let Some(value) = output_value {
        if let Some(matches) = parse_search_matches_from_value(value) {
            return matches;
        }
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(output) {
        if let Some(matches) = parse_search_matches_from_value(&parsed) {
            return matches;
        }
    }

    parse_search_matches_from_text(output)
}

fn parse_search_matches_from_value(value: &Value) -> Option<Vec<SearchMatch>> {
    let mut matches = Vec::new();
    if let Some(array) = value.as_array() {
        for entry in array {
            if let Some(path) = entry.as_str() {
                matches.push(SearchMatch {
                    file: path.to_string(),
                    line: 1,
                    content: String::new(),
                });
            }
            if matches.len() >= TOOL_SEARCH_MATCH_LIMIT {
                break;
            }
        }
    } else if let Some(obj) = value.as_object() {
        if let Some(array) = obj.get("matches").and_then(|v| v.as_array()) {
            for entry in array {
                if let Some(path) = entry.as_str() {
                    matches.push(SearchMatch {
                        file: path.to_string(),
                        line: 1,
                        content: String::new(),
                    });
                    continue;
                }
                if let Some(match_obj) = entry.as_object() {
                    let file = match_obj
                        .get("file")
                        .or_else(|| match_obj.get("file_path"))
                        .or_else(|| match_obj.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let line = match_obj
                        .get("line_number")
                        .or_else(|| match_obj.get("line"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as u32;
                    let content = match_obj
                        .get("line")
                        .or_else(|| match_obj.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    matches.push(SearchMatch {
                        file: file.to_string(),
                        line,
                        content,
                    });
                }
                if matches.len() >= TOOL_SEARCH_MATCH_LIMIT {
                    break;
                }
            }
        } else if let Some(array) = obj.get("files").and_then(|v| v.as_array()) {
            for entry in array {
                if let Some(path) = entry.as_str() {
                    matches.push(SearchMatch {
                        file: path.to_string(),
                        line: 1,
                        content: String::new(),
                    });
                }
                if matches.len() >= TOOL_SEARCH_MATCH_LIMIT {
                    break;
                }
            }
        } else if let Some(array) = obj.get("counts").and_then(|v| v.as_array()) {
            for entry in array {
                if let Some(match_obj) = entry.as_object() {
                    let file = match_obj
                        .get("file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let count = match_obj
                        .get("count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    matches.push(SearchMatch {
                        file: file.to_string(),
                        line: 1,
                        content: format!("{} matches", count),
                    });
                }
                if matches.len() >= TOOL_SEARCH_MATCH_LIMIT {
                    break;
                }
            }
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}

fn parse_search_matches_from_text(output: &str) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((file, rest)) = line.split_once(':') {
            if let Some((line_no, content)) = rest.split_once(':') {
                if let Ok(number) = line_no.trim().parse::<u32>() {
                    matches.push(SearchMatch {
                        file: file.trim().to_string(),
                        line: number,
                        content: content.trim().to_string(),
                    });
                    continue;
                }
            }
        }
        matches.push(SearchMatch {
            file: line.to_string(),
            line: 1,
            content: String::new(),
        });
        if matches.len() >= TOOL_SEARCH_MATCH_LIMIT {
            break;
        }
    }
    matches
}

pub(crate) fn parse_diff_lines(diff_text: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for line in diff_text.lines() {
        if line.starts_with("diff --git") || line.starts_with("index ") {
            continue;
        }
        let (kind, content) = if line.starts_with("+++ ") || line.starts_with("--- ") {
            (DiffLineKind::Header, line.to_string())
        } else if line.starts_with("@@") {
            (DiffLineKind::Header, line.to_string())
        } else if line.starts_with('+') {
            (DiffLineKind::Addition, line[1..].to_string())
        } else if line.starts_with('-') {
            (DiffLineKind::Deletion, line[1..].to_string())
        } else if line.starts_with(' ') {
            (DiffLineKind::Context, line[1..].to_string())
        } else {
            (DiffLineKind::Context, line.to_string())
        };
        lines.push(DiffLine {
            kind,
            content,
            old_line: None,
            new_line: None,
        });
        if lines.len() >= 200 {
            break;
        }
    }
    lines
}

pub(crate) fn build_simple_diff(old_text: &str, new_text: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    lines.push(DiffLine {
        kind: DiffLineKind::Header,
        content: "@@ -1 +1 @@".to_string(),
        old_line: None,
        new_line: None,
    });
    for line in old_text.lines() {
        lines.push(DiffLine {
            kind: DiffLineKind::Deletion,
            content: line.to_string(),
            old_line: None,
            new_line: None,
        });
    }
    for line in new_text.lines() {
        lines.push(DiffLine {
            kind: DiffLineKind::Addition,
            content: line.to_string(),
            old_line: None,
            new_line: None,
        });
    }
    lines
}
