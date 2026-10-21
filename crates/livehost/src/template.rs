//! A small template language the host interprets at run time, so a
//! template or copy change is data, not code.
//!
//! - `{{ name }}` inserts a value from the page's data, HTML-escaped;
//!   `{{{ name }}}` inserts it raw. Dotted paths reach into objects.
//! - `{{#each items}}...{{/each}}` repeats for each item, which the body
//!   reads as `this` or `this.field`.
//! - `{{#if name}}...{{else}}...{{/if}}` tests a value.
//! - `{{> name }}` includes another template from the manifest.
//! - `{{asset static/style.css}}` and `{{module name}}` insert the
//!   content-addressed URL of an object or a browser module.

use anyhow::{Result, bail};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Text(String),
    Var {
        path: String,
        raw: bool,
    },
    Partial(String),
    Asset(String),
    Module(String),
    Each {
        path: String,
        body: Vec<Node>,
    },
    If {
        path: String,
        then: Vec<Node>,
        otherwise: Vec<Node>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    nodes: Vec<Node>,
}

/// What a render needs from the manifest it is pinned to.
pub trait Resolve {
    fn partial(&self, name: &str) -> Option<&Template>;
    fn asset(&self, path: &str) -> Option<String>;
    fn module(&self, name: &str) -> Option<String>;
}

enum Token {
    Text(String),
    Tag { body: String, raw: bool },
}

const MAX_DEPTH: usize = 16;

impl Template {
    pub fn parse(source: &str) -> Result<Self> {
        let tokens = tokenize(source)?;
        let mut position = 0;
        let (nodes, end) = parse_nodes(&tokens, &mut position)?;
        if let Some(end) = end {
            bail!("unexpected {{{{{end}}}}}");
        }
        Ok(Template { nodes })
    }

    pub fn render(&self, data: &Value, resolve: &dyn Resolve) -> Result<String> {
        let mut out = String::new();
        let mut scopes = vec![data];
        render_nodes(&self.nodes, &mut scopes, resolve, &mut out, 0)?;
        Ok(out)
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("{{") {
        if start > 0 {
            tokens.push(Token::Text(rest[..start].to_owned()));
        }
        let raw = rest[start..].starts_with("{{{");
        let (open, close) = if raw { (3, "}}}") } else { (2, "}}") };
        let after = &rest[start + open..];
        let Some(end) = after.find(close) else {
            bail!("unclosed tag");
        };
        tokens.push(Token::Tag {
            body: after[..end].trim().to_owned(),
            raw,
        });
        rest = &after[end + close.len()..];
    }
    if !rest.is_empty() {
        tokens.push(Token::Text(rest.to_owned()));
    }
    Ok(tokens)
}

/// Parses until the end of input or a closing tag, which it returns.
fn parse_nodes(tokens: &[Token], position: &mut usize) -> Result<(Vec<Node>, Option<String>)> {
    let mut nodes = Vec::new();
    while let Some(token) = tokens.get(*position) {
        *position += 1;
        let (body, raw) = match token {
            Token::Text(text) => {
                nodes.push(Node::Text(text.clone()));
                continue;
            }
            Token::Tag { body, raw } => (body.as_str(), *raw),
        };
        if raw {
            nodes.push(Node::Var {
                path: body.to_owned(),
                raw: true,
            });
        } else if body.starts_with('/') || body == "else" {
            return Ok((nodes, Some(body.to_owned())));
        } else if let Some(name) = body.strip_prefix('>') {
            nodes.push(Node::Partial(name.trim().to_owned()));
        } else if let Some(path) = body.strip_prefix("asset ") {
            nodes.push(Node::Asset(path.trim().to_owned()));
        } else if let Some(name) = body.strip_prefix("module ") {
            nodes.push(Node::Module(name.trim().to_owned()));
        } else if let Some(path) = body.strip_prefix("#each ") {
            let (body, end) = parse_nodes(tokens, position)?;
            if end.as_deref() != Some("/each") {
                bail!("{{{{#each}}}} needs {{{{/each}}}}");
            }
            nodes.push(Node::Each {
                path: path.trim().to_owned(),
                body,
            });
        } else if let Some(path) = body.strip_prefix("#if ") {
            let (then, mut end) = parse_nodes(tokens, position)?;
            let mut otherwise = Vec::new();
            if end.as_deref() == Some("else") {
                (otherwise, end) = parse_nodes(tokens, position)?;
            }
            if end.as_deref() != Some("/if") {
                bail!("{{{{#if}}}} needs {{{{/if}}}}");
            }
            nodes.push(Node::If {
                path: path.trim().to_owned(),
                then,
                otherwise,
            });
        } else if body.starts_with('#') {
            bail!("unknown block {{{{{body}}}}}");
        } else {
            nodes.push(Node::Var {
                path: body.to_owned(),
                raw: false,
            });
        }
    }
    Ok((nodes, None))
}

fn render_nodes<'a>(
    nodes: &'a [Node],
    scopes: &mut Vec<&'a Value>,
    resolve: &'a dyn Resolve,
    out: &mut String,
    depth: usize,
) -> Result<()> {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Var { path, raw } => {
                let text = display(lookup(scopes, path));
                if *raw {
                    out.push_str(&text);
                } else {
                    escape_into(&text, out);
                }
            }
            Node::Partial(name) => {
                if depth >= MAX_DEPTH {
                    bail!("templates include each other more than {MAX_DEPTH} deep");
                }
                let Some(partial) = resolve.partial(name) else {
                    bail!("no template named {name}");
                };
                render_nodes(&partial.nodes, scopes, resolve, out, depth + 1)?;
            }
            Node::Asset(path) => match resolve.asset(path) {
                Some(url) => out.push_str(&url),
                None => bail!("no object at {path}"),
            },
            Node::Module(name) => match resolve.module(name) {
                Some(url) => out.push_str(&url),
                None => bail!("no module named {name}"),
            },
            Node::Each { path, body } => {
                if let Some(Value::Array(items)) = lookup(scopes, path) {
                    for item in items {
                        scopes.push(item);
                        let result = render_nodes(body, scopes, resolve, out, depth);
                        scopes.pop();
                        result?;
                    }
                }
            }
            Node::If {
                path,
                then,
                otherwise,
            } => {
                let branch = if truthy(lookup(scopes, path)) {
                    then
                } else {
                    otherwise
                };
                render_nodes(branch, scopes, resolve, out, depth)?;
            }
        }
    }
    Ok(())
}

/// Resolves `this`, `this.a.b`, or `a.b` against the innermost scope, then
/// the page's data.
fn lookup<'a>(scopes: &[&'a Value], path: &str) -> Option<&'a Value> {
    let inner = *scopes.last()?;
    if path == "this" {
        return Some(inner);
    }
    if let Some(rest) = path.strip_prefix("this.") {
        return walk(inner, rest);
    }
    scopes.iter().rev().find_map(|scope| walk(scope, path))
}

fn walk<'a>(mut value: &'a Value, path: &str) -> Option<&'a Value> {
    for key in path.split('.') {
        value = match value {
            Value::Object(map) => map.get(key)?,
            Value::Array(items) => items.get(key.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
        _ => true,
    }
}

fn display(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

pub fn escape_into(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    struct Fixture(HashMap<String, Template>);

    impl Resolve for Fixture {
        fn partial(&self, name: &str) -> Option<&Template> {
            self.0.get(name)
        }
        fn asset(&self, path: &str) -> Option<String> {
            (path == "static/style.css").then(|| "/_o/abc.css".into())
        }
        fn module(&self, name: &str) -> Option<String> {
            (name == "greet").then(|| "/_o/def.wasm".into())
        }
    }

    #[test]
    fn renders_values_blocks_partials_and_urls() {
        let header = Template::parse("<h1>{{ title }}</h1>").unwrap();
        let fixture = Fixture(HashMap::from([("header".to_owned(), header)]));
        let page = Template::parse(
            "{{> header}}<link href=\"{{asset static/style.css}}\">{{module greet}}\
             {{#each items}}<li>{{this.name}}</li>{{/each}}\
             {{#if missing}}yes{{else}}no{{/if}}{{{ html }}}",
        )
        .unwrap();
        let data = json!({
            "title": "A & B",
            "items": [{"name": "<one>"}, {"name": "two"}],
            "html": "<b>raw</b>",
        });
        assert_eq!(
            page.render(&data, &fixture).unwrap(),
            "<h1>A &amp; B</h1><link href=\"/_o/abc.css\">/_o/def.wasm\
             <li>&lt;one&gt;</li><li>two</li>no<b>raw</b>"
        );
    }

    #[test]
    fn rejects_malformed_templates_and_missing_references() {
        assert!(Template::parse("{{#each items}}x").is_err());
        assert!(Template::parse("{{ open").is_err());
        assert!(Template::parse("{{/if}}").is_err());
        let fixture = Fixture(HashMap::new());
        let missing = Template::parse("{{> nope}}").unwrap();
        assert!(missing.render(&json!({}), &fixture).is_err());
        let looped = Template::parse("{{> self}}").unwrap();
        let fixture = Fixture(HashMap::from([("self".to_owned(), looped.clone())]));
        assert!(looped.render(&json!({}), &fixture).is_err());
    }
}
