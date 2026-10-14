//! The `graph_*` tools: their definitions, guidance and execution.

use std::path::Path;

use serde_json::{Value, json};

use crate::model::Kind;
use crate::query::DEFAULT_LIMIT;
use crate::store::{self, Origin};

pub const CALLERS: &str = "graph_callers";
pub const CALLEES: &str = "graph_callees";
pub const IMPACT: &str = "graph_impact";
pub const PATH: &str = "graph_path";
pub const SYMBOL: &str = "graph_symbol";
pub const MODULE: &str = "graph_module";

/// Every tool's name.
pub const NAMES: [&str; 6] = [SYMBOL, CALLERS, CALLEES, IMPACT, PATH, MODULE];

/// What a model is told about the tools.
pub const INSTRUCTIONS: &str = "The graph_* tools answer structure questions about this repository's Rust from a prebuilt code graph, in short file:line lists: where a symbol is defined (graph_symbol), who calls or uses it (graph_callers), what it calls (graph_callees), what depends on it if it changes (graph_impact), how one symbol reaches another (graph_path), and a module's public items (graph_module). For structure and impact questions, query the graph before opening source, then Read only the lines it points to. Names can be qualified (crate::module::Type::method), partial (Type::method), a file path, or file.rs:LINE; an ambiguous name returns candidates to choose from. Edges marked ? were matched by name. The graph covers Rust code only.\n";

#[must_use]
pub fn is_tool(name: &str) -> bool {
    NAMES.contains(&name)
}

fn function(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function","function":{
        "name": name,
        "description": description,
        "parameters": {
            "type":"object",
            "additionalProperties": false,
            "required": required,
            "properties": properties
        }
    }})
}

/// The tools' definitions, in the chat-completions function shape.
#[must_use]
pub fn definitions() -> Vec<Value> {
    let symbol = json!({"type":"string","description":"crate::module::Item, Type::method, a name, a file path, or file.rs:LINE"});
    let limit =
        json!({"type":"integer","minimum":1,"maximum":200,"description":"most rows (default 40)"});
    let kinds: Vec<&str> = Kind::ALL.iter().map(|kind| kind.code()).collect();
    vec![
        function(
            SYMBOL,
            "Where a Rust symbol is defined: file:line, signature, parent, caller and callee counts, and a type's impls. No source.",
            json!({"symbol": symbol, "kind": {"type":"string","enum": kinds}}),
            &["symbol"],
        ),
        function(
            CALLERS,
            "Who calls, names, or imports a Rust symbol, as file:line rows.",
            json!({"symbol": symbol, "limit": limit}),
            &["symbol"],
        ),
        function(
            CALLEES,
            "What a Rust function calls and which types it names, as file:line rows.",
            json!({"symbol": symbol, "limit": limit}),
            &["symbol"],
        ),
        function(
            IMPACT,
            "What may break if a Rust symbol changes: its transitive callers and dependents, nearest first, with file:line.",
            json!({"symbol": symbol, "depth": {"type":"integer","minimum":1,"maximum":8,"description":"steps back (default 3)"}, "limit": limit, "exact_only": {"type":"boolean","description":"skip edges matched only by name"}}),
            &["symbol"],
        ),
        function(
            PATH,
            "The shortest chain of calls and type references from one Rust symbol to another.",
            json!({"from": symbol, "to": symbol, "max_depth": {"type":"integer","minimum":1,"maximum":12}}),
            &["from", "to"],
        ),
        function(
            MODULE,
            "A Rust module's public surface: its public items with signatures and line numbers, and its submodules.",
            json!({"module": {"type":"string","description":"crate::module or a file path"}, "limit": limit}),
            &["module"],
        ),
    ]
}

fn text<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{key} is required."))
}

fn number(arguments: &Value, key: &str, default: usize, max: usize) -> usize {
    arguments
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |n| usize::try_from(n).unwrap_or(max))
        .clamp(1, max)
}

/// Run one tool against the graph of the checkout `cwd` is in.
pub fn execute(name: &str, arguments: &Value, cwd: &Path) -> Result<Value, String> {
    if !is_tool(name) {
        return Err(format!("{name} is not a graph tool."));
    }
    let opened = store::open(cwd)?;
    let index = &opened.index;
    let limit = number(arguments, "limit", DEFAULT_LIMIT, 200);
    let answer = match name {
        SYMBOL => {
            let kind = arguments
                .get("kind")
                .and_then(Value::as_str)
                .and_then(Kind::parse);
            index.symbol(text(arguments, "symbol")?, kind)
        }
        CALLERS => index.callers(text(arguments, "symbol")?, limit),
        CALLEES => index.callees(text(arguments, "symbol")?, limit),
        IMPACT => index.impact(
            text(arguments, "symbol")?,
            number(arguments, "depth", 3, 8),
            limit,
            arguments
                .get("exact_only")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        PATH => index.path(
            text(arguments, "from")?,
            text(arguments, "to")?,
            number(arguments, "max_depth", 8, 12),
        ),
        _ => index.module(text(arguments, "module")?, limit),
    };
    let short: String = opened.commit.chars().take(10).collect();
    let graph = format!(
        "{short}{}{}",
        if opened.dirty {
            " with uncommitted changes"
        } else {
            ""
        },
        match opened.origin {
            Origin::Built { millis } => format!(", built in {millis} ms"),
            _ => String::new(),
        }
    );
    // An ambiguous or unknown name is an answer, not a failure: the model
    // picks a candidate and asks again.
    let answer = match answer {
        Ok(answer) | Err(answer) => answer,
    };
    Ok(json!({"graph": graph, "answer": answer}))
}
