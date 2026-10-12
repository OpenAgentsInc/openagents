//! Split a workflow script into its meta literal and its body.

use serde_json::{Map, Value};

use crate::cut;

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub name: String,
    pub description: String,
    pub phases: Vec<DeclaredPhase>,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredPhase {
    pub title: String,
    pub detail: Option<String>,
    pub model: Option<String>,
}

/// The script must begin with `export const meta = { ... }`, a literal.
pub fn parse(source: &str) -> Result<Script, String> {
    let source = trim_start_js(source);
    let Some(after_keyword) = source.strip_prefix("export const meta") else {
        return Err("Start the script with export const meta.".into());
    };
    let after_keyword = trim_start_js(after_keyword);
    let Some(after_eq) = after_keyword.strip_prefix('=') else {
        return Err("The meta object has to be a literal.".into());
    };
    let after_eq = trim_start_js(after_eq);
    let (object, rest) = take_object(after_eq)?;
    let rest = trim_start_js(rest);
    let body = rest.strip_prefix(';').map(trim_start_js).unwrap_or(rest);
    let meta = eval_meta(object)?;
    fields(&meta, body)
}

fn fields(meta: &Value, body: &str) -> Result<Script, String> {
    let Some(object) = meta.as_object() else {
        return Err("The meta object has to be a literal.".into());
    };
    let name = required_text(object, "name")?;
    let description = required_text(object, "description")?;
    if name.is_empty() || description.is_empty() {
        return Err("The script's meta needs a name and a description.".into());
    }
    let phases = phases(object.get("phases"))?;
    Ok(Script {
        name: cut(&name, 64),
        description: cut(&description, 256),
        phases,
        body: body.to_string(),
    })
}

fn required_text(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    match object.get(key) {
        Some(Value::String(text)) => Ok(text.trim().to_string()),
        Some(_) => Err("The script's meta needs a name and a description.".into()),
        None => Err("The script's meta needs a name and a description.".into()),
    }
}

fn phases(value: Option<&Value>) -> Result<Vec<DeclaredPhase>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err("phases has to be a list.".into());
    };
    let mut phases = Vec::with_capacity(items.len());
    for item in items {
        let Some(object) = item.as_object() else {
            return Err("Each phase needs a title.".into());
        };
        let Some(Value::String(title)) = object.get("title") else {
            return Err("Each phase needs a title.".into());
        };
        let title = cut(title.trim(), 64);
        if title.is_empty() {
            return Err("Each phase needs a title.".into());
        }
        let detail = optional_text(object, "detail")?.map(|text| cut(&text, 256));
        let model = optional_text(object, "model")?;
        phases.push(DeclaredPhase {
            title,
            detail,
            model,
        });
    }
    Ok(phases)
}

fn optional_text(object: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.trim().to_string())),
        Some(_) => Err(format!("{key} has to be text.")),
    }
}

fn eval_meta(object: &str) -> Result<Value, String> {
    let mut context = boa_engine::Context::default();
    let source = format!("({object})");
    let value = context
        .eval(boa_engine::Source::from_bytes(&source))
        .map_err(|_| "The meta object has to be a literal.".to_string())?;
    value
        .to_json(&mut context)
        .map_err(|_| "The meta object has to be a literal.".to_string())?
        .ok_or_else(|| "The meta object has to be a literal.".to_string())
}

fn trim_start_js(source: &str) -> &str {
    let mut rest = source;
    loop {
        rest = rest.trim_start();
        if let Some(line) = rest.strip_prefix("//") {
            rest = line.split_once('\n').map(|(_, tail)| tail).unwrap_or("");
            continue;
        }
        if let Some(block) = rest.strip_prefix("/*") {
            let Some((_, tail)) = block.split_once("*/") else {
                return "";
            };
            rest = tail;
            continue;
        }
        return rest;
    }
}

/// The meta object, and the rest of the script. Rejects calls, spreads, and
/// template strings so the object stays a literal.
fn take_object(source: &str) -> Result<(&str, &str), String> {
    if !source.starts_with('{') {
        return Err("The meta object has to be a literal.".into());
    }
    let chars: Vec<(usize, char)> = source.char_indices().collect();
    let mut depth = 0;
    let mut in_string: Option<char> = None;
    let mut escape = false;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut index = 0;
    while index < chars.len() {
        let (byte, ch) = chars[index];
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if ch == '*' && next_char(&chars, index) == Some('/') {
                block_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(quote) = in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == quote {
                in_string = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next_char(&chars, index) == Some('/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if ch == '/' && next_char(&chars, index) == Some('*') {
            block_comment = true;
            index += 2;
            continue;
        }
        if ch == '`' {
            return Err("The meta object has to be a literal.".into());
        }
        if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            index += 1;
            continue;
        }
        if ch == '(' {
            return Err("The meta object has to be a literal.".into());
        }
        if ch == '.'
            && next_char(&chars, index) == Some('.')
            && nth_char(&chars, index, 2) == Some('.')
        {
            return Err("The meta object has to be a literal.".into());
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let end = byte + ch.len_utf8();
                return Ok((&source[..end], &source[end..]));
            }
        }
        index += 1;
    }
    Err("The meta object is not closed.".into())
}

fn next_char(chars: &[(usize, char)], index: usize) -> Option<char> {
    nth_char(chars, index, 1)
}

fn nth_char(chars: &[(usize, char)], index: usize, offset: usize) -> Option<char> {
    chars.get(index + offset).map(|(_, ch)| *ch)
}
