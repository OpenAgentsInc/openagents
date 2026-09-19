//! Flattening structured request content into the text the model sees, and
//! the rewrite that keeps caller text from forging delimiter tokens.
//!
//! `render` and `option_text` mirror `kev/api.py` exactly: training data and
//! live requests go through one renderer, and the conformance fixtures in
//! `fixtures/requests/` pin the output.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// Matches `<|name|>` spans in caller text.
static SPECIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\|([A-Za-z0-9_]+)\|>").expect("delimiter regex"));

/// Rewrite caller-supplied text so it can never produce delimiter or control
/// tokens: `<|name|>` becomes the lookalike `<¦name¦>`, which tokenizes as
/// ordinary text. Option boundaries are unforgeable from user input.
pub fn sanitize(text: &str) -> Cow<'_, str> {
    SPECIAL_RE.replace_all(text, "<\u{a6}$1\u{a6}>")
}

/// Flatten `string | object | array | scalar | null` into labelled text.
/// Field names stay as labels; list items get `- ` bullets at their depth.
pub fn render(value: &Value) -> String {
    render_at(value, 0)
}

/// `render` at a given indentation: two spaces per level.
fn render_at(value: &Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => {
            if *b { "True".to_string() } else { "False".to_string() }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                let body = render_at(item, indent + 1);
                format!("{pad}- {}", body.trim_start())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => map
            .iter()
            .map(|(key, entry)| match entry {
                Value::Object(_) | Value::Array(_) => {
                    format!("{pad}{key}:\n{}", render_at(entry, indent + 1))
                }
                _ => format!("{pad}{key}: {}", render_at(entry, 0)),
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// One option's rendered text: the bare key when the description is null or
/// empty, `key: rendered description` otherwise.
pub fn option_text(name: &str, description: &Value) -> String {
    if description.is_null() || description.as_str() == Some("") {
        name.to_string()
    } else {
        format!("{name}: {}", render(description))
    }
}
