//! The interactive-answer format benchmark (#11113,
//! `docs/research/2026-10-09-ui-format-benchmark.md`): the OpenUI Lang
//! subset against minified nested JSON, on our models and our catalog.
//!
//! Both formats are scored by the same validator. A JSON reply is turned
//! into one OpenUI Lang statement ([`json_to_lang`]) and parsed by
//! [`openui_lang::parse`], so the only difference measured is what the
//! model writes: validity, blank screens, length, and how soon a streaming
//! reply can draw something.
//!
//! The `ui-format-bench` binary sends the prompts in
//! `bench/ui-format/prompts-v1.json`; this module is everything it does
//! that needs no network.

use openui_lang::catalog::{CATALOG, Kind};
use openui_lang::{Expr, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The fence a JSON reply's block opens with.
pub const JSON_LANG: &str = "ui-json";

/// The prompts file's schema.
pub const SCHEMA: &str = "openagents.ui-format-bench.v1";

/// The checked-in prompts.
pub const PROMPTS: &str = include_str!("../../../bench/ui-format/prompts-v1.json");

/// A wire format under test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// The OpenUI Lang subset in an ```` ```openui-lang ```` block.
    Lang,
    /// Minified nested JSON in a ```` ```ui-json ```` block.
    Json,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Lang, Format::Json];

    /// The word reports use.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Format::Lang => "openui-lang",
            Format::Json => "json",
        }
    }

    /// The model's instructions: the same lead rule, then the format's
    /// own description of the same catalog.
    #[must_use]
    pub fn instructions(self) -> String {
        let lead = "Answer with a one-line lead, then the interface. ";
        match self {
            Format::Lang => format!("{lead}{}", openui_lang::prompt()),
            Format::Json => format!("{lead}{}", json_prompt()),
        }
    }
}

/// The prompts file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Prompts {
    pub schema: String,
    pub prompts: Vec<Prompt>,
}

/// One prompt.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Prompt {
    pub id: String,
    pub prompt: String,
}

impl Prompts {
    /// Parses a prompts file.
    ///
    /// # Errors
    ///
    /// The JSON error, a schema that isn't [`SCHEMA`], or no prompts.
    pub fn parse(json: &str) -> Result<Self, String> {
        let prompts: Prompts = serde_json::from_str(json).map_err(|e| format!("prompts: {e}"))?;
        if prompts.schema != SCHEMA {
            return Err(format!("prompts: schema {}, not {SCHEMA}", prompts.schema));
        }
        if prompts.prompts.is_empty() {
            return Err("prompts: none".into());
        }
        Ok(prompts)
    }
}

/// The catalog described for the JSON format: the same components and
/// arguments as [`openui_lang::prompt`], written as nested objects.
#[must_use]
pub fn json_prompt() -> String {
    let mut out = format!(
        "To show interactive parts with an answer, add one fenced ```{JSON_LANG} block after a \
         one-line lead holding one minified JSON object, the root component. Each component is an \
         object whose \"type\" is its name and whose other keys are its arguments; a list of \
         components is an array of such objects. Links are https:// URLs or site paths such as \
         /download. Use only these components:\n"
    );
    for component in CATALOG {
        let args: Vec<String> = component
            .props
            .iter()
            .map(|p| {
                let kind = match p.kind {
                    Kind::Text | Kind::Href => "string".to_owned(),
                    Kind::Choice(words) => words
                        .iter()
                        .map(|w| format!("\"{w}\""))
                        .collect::<Vec<_>>()
                        .join("|"),
                    Kind::Children => "[component]".to_owned(),
                    Kind::Items(item) => format!("[{item}]"),
                };
                let optional = if p.required { "" } else { "?" };
                format!("\"{}\"{optional}: {kind}", p.name)
            })
            .collect();
        out.push_str(&format!(
            "- {{\"type\": \"{}\", {}}}: {}\n",
            component.name,
            args.join(", "),
            component.about
        ));
    }
    out
}

/// One JSON component tree as an OpenUI Lang `root` statement, so the same
/// validator scores both formats.
///
/// # Errors
///
/// A value no OpenUI Lang expression can hold (a non-finite number).
pub fn json_to_lang(value: &Value) -> Result<String, String> {
    Ok(Statement {
        name: "root".into(),
        expr: expr(value)?,
    }
    .to_string())
}

fn expr(value: &Value) -> Result<Expr, String> {
    Ok(match value {
        Value::Null => Expr::Null,
        Value::Bool(b) => Expr::Bool(*b),
        Value::Number(n) => Expr::Num(
            n.as_f64()
                .filter(|f| f.is_finite())
                .ok_or_else(|| format!("the number {n} is not finite"))?,
        ),
        Value::String(s) => Expr::Str(s.clone()),
        Value::Array(items) => Expr::Array(items.iter().map(expr).collect::<Result<_, _>>()?),
        Value::Object(fields) => match fields.get("type").and_then(Value::as_str) {
            Some(name) => Expr::Call {
                name: name.to_owned(),
                args: Vec::new(),
                named: fields
                    .iter()
                    .filter(|(key, _)| key.as_str() != "type")
                    .map(|(key, value)| Ok((key.clone(), expr(value)?)))
                    .collect::<Result<_, String>>()?,
            },
            None => Expr::Object(
                fields
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), expr(value)?)))
                    .collect::<Result<_, String>>()?,
            ),
        },
    })
}

/// The last block fenced with `lang` in `text`, and whether it is closed.
#[must_use]
pub fn fenced<'a>(text: &'a str, lang: &str) -> Option<(&'a str, bool)> {
    let mut found = None;
    let mut body: Option<usize> = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let end = at + line.len();
        let trimmed = line.trim();
        match body {
            None => {
                if trimmed
                    .strip_prefix("```")
                    .is_some_and(|info| info.trim() == lang)
                {
                    body = Some(end);
                }
            }
            Some(start) => {
                if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '`') {
                    found = Some((&text[start..at], true));
                    body = None;
                }
            }
        }
        at = end;
    }
    match body {
        Some(start) => Some((&text[start.min(text.len())..], false)),
        None => found,
    }
}

/// How one reply scored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    /// A closed block that draws something with nothing fixed or dropped.
    pub valid: bool,
    /// Nothing to draw: no block, a block that does not parse, or no root.
    pub blank: bool,
    /// What the validator fixed or dropped.
    pub diagnostics: usize,
    /// The block's length in characters.
    pub block_chars: usize,
}

/// Scores a whole reply in `format`.
#[must_use]
pub fn score(format: Format, reply: &str) -> Score {
    let Some((program, block_chars, closed)) = program(format, reply) else {
        return Score {
            blank: true,
            ..Score::default()
        };
    };
    let document = openui_lang::parse(&program);
    let blank = document.root.is_none();
    Score {
        valid: closed && !blank && document.diagnostics.is_empty(),
        blank,
        diagnostics: document.diagnostics.len(),
        block_chars,
    }
}

/// Whether a reply still streaming in could draw something yet: the OpenUI
/// Lang block with the streaming rules, or the JSON block once it parses
/// whole (nested JSON has no finished parts before it closes).
#[must_use]
pub fn renders(format: Format, partial: &str) -> bool {
    match format {
        Format::Lang => fenced(partial, openui_lang::LANG)
            .is_some_and(|(source, _)| openui_lang::parse_partial(source).root.is_some()),
        Format::Json => program(format, partial)
            .is_some_and(|(program, _, _)| openui_lang::parse_partial(&program).root.is_some()),
    }
}

/// The reply's block as an OpenUI Lang program, its length, and whether
/// its fence closed; `None` when there is no block, or a JSON block does
/// not parse.
fn program(format: Format, reply: &str) -> Option<(String, usize, bool)> {
    match format {
        Format::Lang => {
            let (source, closed) = fenced(reply, openui_lang::LANG)?;
            Some((source.to_owned(), source.chars().count(), closed))
        }
        Format::Json => {
            let (source, closed) = fenced(reply, JSON_LANG)?;
            let value: Value = serde_json::from_str(source.trim()).ok()?;
            Some((json_to_lang(&value).ok()?, source.chars().count(), closed))
        }
    }
}

#[cfg(test)]
mod tests;
