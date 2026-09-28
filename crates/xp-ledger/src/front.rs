//! A small reader for the YAML front matter entries use: scalars, quoted
//! scalars, folded (`>`) and literal (`|`) blocks, flow lists (`[a, b]`),
//! block lists (`- a`), and nested maps. Anchors, multi-document streams,
//! and the rest of YAML are left out.

/// One value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    List(Vec<String>),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// The value as text, when it's a scalar.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The value as a list. An empty scalar is an empty list.
    #[must_use]
    pub fn list(&self) -> Option<Vec<String>> {
        match self {
            Value::List(items) => Some(items.clone()),
            Value::Text(text) if text.is_empty() => Some(Vec::new()),
            _ => None,
        }
    }
}

/// Splits a document into its front matter and its body.
///
/// # Errors
///
/// When the document doesn't open with a `---` line or never closes it.
pub fn split(text: &str) -> Result<(&str, &str), String> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or("the file doesn't start with a --- line")?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Ok((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    Err("the front matter has no closing --- line".to_string())
}

/// Parses front matter into its top-level keys, in order.
///
/// # Errors
///
/// A line that isn't `key: value`, or a key that appears twice.
pub fn parse(text: &str) -> Result<Vec<(String, Value)>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut pos = 0;
    map(&lines, &mut pos, 0)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn blank(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn map(lines: &[&str], pos: &mut usize, level: usize) -> Result<Vec<(String, Value)>, String> {
    let mut out: Vec<(String, Value)> = Vec::new();
    while *pos < lines.len() {
        let line = lines[*pos];
        if blank(line) {
            *pos += 1;
            continue;
        }
        if indent(line) < level {
            break;
        }
        if indent(line) > level {
            return Err(format!("line {}: unexpected indentation", *pos + 1));
        }
        let (key, rest) = line
            .trim()
            .split_once(':')
            .ok_or(format!("line {}: expected `key: value`", *pos + 1))?;
        let key = key.trim().to_string();
        if out.iter().any(|(k, _)| *k == key) {
            return Err(format!("line {}: `{key}` appears twice", *pos + 1));
        }
        let rest = rest.trim();
        *pos += 1;
        let value = if rest.starts_with('>') || rest.starts_with('|') {
            Value::Text(block(lines, pos, level, rest))
        } else if rest.starts_with('[') {
            Value::List(flow(rest).map_err(|e| format!("`{key}`: {e}"))?)
        } else if rest.is_empty() || rest.starts_with('#') {
            nested(lines, pos, level)?
        } else {
            Value::Text(scalar(rest))
        };
        out.push((key, value));
    }
    Ok(out)
}

/// The indented lines under a key with no value: a block list or a map.
fn nested(lines: &[&str], pos: &mut usize, level: usize) -> Result<Value, String> {
    let next = lines[*pos..].iter().position(|l| !blank(l));
    let Some(next) = next.map(|n| n + *pos) else {
        return Ok(Value::Text(String::new()));
    };
    let deeper = indent(lines[next]);
    if deeper <= level {
        return Ok(Value::Text(String::new()));
    }
    if lines[next].trim_start().starts_with("- ") || lines[next].trim() == "-" {
        let mut items = Vec::new();
        while *pos < lines.len() {
            let line = lines[*pos];
            if blank(line) {
                *pos += 1;
                continue;
            }
            if indent(line) < deeper {
                break;
            }
            let item = line
                .trim()
                .strip_prefix('-')
                .ok_or(format!("line {}: expected a `- item`", *pos + 1))?;
            items.push(scalar(item.trim()));
            *pos += 1;
        }
        Ok(Value::List(items))
    } else {
        Ok(Value::Map(map(lines, pos, deeper)?))
    }
}

/// A `>` or `|` block: the indented lines that follow, folded or kept.
fn block(lines: &[&str], pos: &mut usize, level: usize, marker: &str) -> String {
    let fold = marker.starts_with('>');
    let mut taken: Vec<&str> = Vec::new();
    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.trim().is_empty() && indent(line) <= level {
            break;
        }
        taken.push(line);
        *pos += 1;
    }
    let cut = taken
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| indent(l))
        .min()
        .unwrap_or(0);
    let body: Vec<&str> = taken
        .iter()
        .map(|l| if l.len() >= cut { &l[cut..] } else { "" })
        .collect();
    let text = if fold {
        let mut out = String::new();
        for line in body {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push(' ');
                }
                out.push_str(line.trim_end());
            }
        }
        out
    } else {
        body.join("\n")
    };
    text.trim_end().to_string()
}

/// A plain or quoted scalar, without a trailing comment.
fn scalar(text: &str) -> String {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = text.strip_prefix(quote)
            && let Some(end) = inner.find(quote)
        {
            return inner[..end].to_string();
        }
    }
    match text.find(" #") {
        Some(at) => text[..at].trim_end().to_string(),
        None => text.to_string(),
    }
}

/// A `[a, "b, c", d]` list.
fn flow(text: &str) -> Result<Vec<String>, String> {
    let inner = text
        .trim()
        .strip_prefix('[')
        .and_then(|t| t.split(" #").next())
        .map(str::trim_end)
        .and_then(|t| t.strip_suffix(']'))
        .ok_or("a list opened with [ must close with ] on the same line")?;
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => quote = Some(c),
            (None, ',') => {
                items.push(current.trim().to_string());
                current.clear();
            }
            (None, c) => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_form_an_entry_uses_parses() {
        let text = "id: a.b\nversion: 1\nkind: method            # a comment\nsummary: >-\n  one\n  two\n\n  three\ntags: [x, \"y, z\"]\nprovenance:\n  written_from: [reference]\n  cites:\n    - \"Book, 2nd ed.\"\n    - Paper\nevidence: []\n";
        let parsed = parse(text).unwrap();
        let get = |k: &str| parsed.iter().find(|(key, _)| key == k).unwrap().1.clone();
        assert_eq!(get("kind"), Value::Text("method".to_string()));
        assert_eq!(get("summary"), Value::Text("one two\nthree".to_string()));
        assert_eq!(get("tags").list().unwrap(), ["x", "y, z"]);
        assert_eq!(get("evidence").list().unwrap(), Vec::<String>::new());
        let Value::Map(provenance) = get("provenance") else {
            panic!("provenance is a map");
        };
        assert_eq!(provenance[1].1.list().unwrap(), ["Book, 2nd ed.", "Paper"]);
    }

    #[test]
    fn a_repeated_key_is_refused() {
        assert!(parse("a: 1\na: 2\n").unwrap_err().contains("twice"));
    }

    #[test]
    fn the_body_follows_the_closing_line() {
        let (front, body) = split("---\nid: a\n---\n\n## Body\n").unwrap();
        assert_eq!(front, "id: a\n");
        assert_eq!(body, "\n## Body\n");
        assert!(split("id: a\n").is_err());
    }
}
