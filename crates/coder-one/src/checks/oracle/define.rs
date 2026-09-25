//! The task's stated definition, parameters, and boundary inputs, picked
//! by Jev from spans code lists.
//!
//! Code splits the instruction into sentences and lists every stated value
//! in them: a number, or a `name = value` span. Jev never writes text; it
//! answers one Noul per candidate, over the task's words alone:
//!
//! - [`DEFINES`]: does this sentence state how a correct result is
//!   computed or what it must satisfy?
//! - [`BOUNDARY`]: does this sentence name a boundary or edge input the
//!   result must handle?
//! - [`PARAMETER`]: is this value a setting the correct result depends on?
//!
//! Each question is general: none names a task, a file, or a value.
//! Candidates go [`BATCH`] to a request, with the task's words as state,
//! and every request is kept with its key so a run replays with no calls.

use std::collections::BTreeMap;

use jev::{Noul, NoulCriteria, Questions};
use serde_json::{Value, json};

use super::super::contract::clip;
use super::super::contract::extract::{self, Pristine};
use super::super::contract::host::Stat;
use super::{CaseSpec, InputHead, Parameter, Spec};
use crate::checks::acceptance::Covers;
use crate::component::jev::{Ask, JevMode, USD_PER_MILLION_INPUT, ask};
use crate::record::Recorder;

/// Candidates per request.
pub const BATCH: usize = 8;

/// The most sentences code offers Jev.
pub const MAX_SENTENCES: usize = 64;

/// The most stated values code offers Jev.
pub const MAX_VALUES: usize = 40;

/// The bound a Noul must reach. Not fitted: the midpoint.
pub const THRESHOLD: f64 = 0.5;

/// The most definition sentences, parameters, and boundaries kept, by
/// Jev's probability.
pub const KEEP_DEFINITION: usize = 16;
pub const KEEP_PARAMETERS: usize = 12;
pub const KEEP_BOUNDARIES: usize = 8;

/// The most characters of the task's words in each request's state.
pub const TASK_CHARS: usize = 7000;

/// The most characters of each input file's head.
pub const HEAD_CHARS: usize = 3000;

/// Does the sentence define a correct result?
pub const DEFINES: &str = "The sentence in `sentences[J]` comes from the task in `task`. Does \
it state how a correct result is computed or what a correct result must satisfy: a formula, a \
rule, an algorithm, a scoring definition, an ordering, or the exact relation between the input \
and the output?";

/// Does the sentence name a boundary input?
pub const BOUNDARY: &str = "The sentence in `sentences[J]` comes from the task in `task`. Does \
it name a boundary or edge case that a correct result must handle in a stated way, such as an \
empty or missing input, a tie, a value exactly at a threshold, the smallest or largest allowed \
input, or an invalid input that must be rejected?";

/// Is the value a parameter of the computation?
pub const PARAMETER: &str = "The value in `values[J].value` appears in the sentence \
`values[J].sentence` of the task in `task`. Is it a stated parameter of the computation: a \
setting such as a threshold, a window, a count, a rate, a tolerance, or a constant that the \
correct result depends on?";

fn defines(text: &str) -> Noul {
    Noul::with_criteria(
        text,
        NoulCriteria::new()
            .when_true(
                "The sentence gives the rule, formula, or relation a correct result follows.",
            )
            .when_false(
                "The sentence is about setup, file locations, tools, running time, background, \
                 or how the work is submitted.",
            ),
    )
}

fn boundary(text: &str) -> Noul {
    Noul::with_criteria(
        text,
        NoulCriteria::new()
            .when_true("The sentence names a specific edge input and what must happen with it.")
            .when_false("The sentence states only the general case, or nothing about inputs."),
    )
}

fn parameter(text: &str) -> Noul {
    Noul::with_criteria(
        text,
        NoulCriteria::new()
            .when_true("Changing the value would change what the correct result is.")
            .when_false(
                "The value is an example's output, part of a file name or a path, a version, a \
                 date, a count of provided files, or a limit on running time or resources.",
            ),
    )
}

/// A stated value code found.
#[derive(Clone, Debug, PartialEq)]
pub struct StatedValue {
    pub name: String,
    pub value: String,
    pub sentence: String,
}

fn is_number(token: &str) -> bool {
    let t = token.trim_start_matches(['-', '+']);
    !t.is_empty()
        && t.chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '-' | '+' | '_' | '%'))
        && t.chars().any(|c| c.is_ascii_digit())
        && t.matches('.').count() <= 1
}

/// The stated values in `sentence`: `name = value` code spans, and plain
/// numbers with the three words before them as the name. A number inside
/// a path or a version isn't one.
#[must_use]
pub fn values_in(sentence: &str) -> Vec<StatedValue> {
    let mut out = Vec::new();
    for (_, span) in extract::spans(sentence) {
        if let Some((name, value)) = span.split_once('=')
            && extract::is_identifier(name.trim())
            && !value.trim().is_empty()
            && !value.contains('=')
        {
            out.push(StatedValue {
                name: name.trim().to_string(),
                value: value.trim().to_string(),
                sentence: sentence.to_string(),
            });
        }
    }
    let plain: String = {
        // Code spans are dropped: a number in a path or a command isn't a
        // stated value.
        let mut text = String::new();
        let mut code = false;
        for c in sentence.chars() {
            if c == '`' {
                code = !code;
                text.push(' ');
            } else if !code {
                text.push(c);
            }
        }
        text
    };
    let words: Vec<&str> = plain.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        let token = word.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '(' | ')' | '"'));
        let token = token.strip_suffix('.').unwrap_or(token);
        if !is_number(token) {
            continue;
        }
        let name = words[i.saturating_sub(3)..i]
            .iter()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if out
            .iter()
            .any(|v| v.value == token && v.sentence == sentence)
        {
            continue;
        }
        out.push(StatedValue {
            name,
            value: token.to_string(),
            sentence: sentence.to_string(),
        });
    }
    out
}

/// The sentences code offers Jev: units of four words or more.
#[must_use]
pub fn sentences(instruction: &str) -> Vec<String> {
    let (units, _) = extract::segment(instruction);
    let mut out: Vec<String> = Vec::new();
    for unit in units {
        if unit.text.split_whitespace().count() >= 4 && !out.contains(&unit.text) {
            out.push(unit.text);
        }
        if out.len() == MAX_SENTENCES {
            break;
        }
    }
    out
}

/// `sentence` with the fenced blocks that follow it in the instruction.
#[must_use]
pub fn with_blocks(instruction: &str, sentence: &str) -> String {
    let (units, blocks) = extract::segment(instruction);
    let mut text = sentence.to_string();
    if let Some(i) = units.iter().position(|u| u.text == sentence) {
        for block in blocks.iter().filter(|b| b.lead == Some(i)) {
            text.push_str(&format!(
                "\n```{}\n{}```",
                block.lang,
                clip(&block.body, 1200)
            ));
        }
    }
    text
}

/// Sentences that name a path, a command, or a file format: what the
/// writer reads as the input and output formats.
#[must_use]
pub fn formats(instruction: &str, workdir: &str) -> Vec<String> {
    const FORMAT_WORDS: &[&str] = &[
        "json",
        "csv",
        "tsv",
        "column",
        "columns",
        "header",
        "format",
        "line",
        "lines",
        "field",
        "fields",
        "key",
        "keys",
        "print",
        "prints",
        "stdout",
        "output",
        "argument",
        "arguments",
    ];
    let (units, blocks) = extract::segment(instruction);
    let mut out = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        let spans = extract::spans(&unit.text);
        let named = spans
            .iter()
            .any(|(_, s)| extract::as_path(s, workdir).is_some() || extract::is_command(s));
        let lower = unit.text.to_lowercase();
        let worded = FORMAT_WORDS
            .iter()
            .any(|w| lower.split(|c: char| !c.is_alphanumeric()).any(|x| x == *w));
        let led = blocks.iter().any(|b| b.lead == Some(i));
        if named || worded || led {
            let mut text = unit.text.clone();
            for block in blocks.iter().filter(|b| b.lead == Some(i)) {
                text.push_str(&format!(
                    "\n```{}\n{}```",
                    block.lang,
                    clip(&block.body, 1200)
                ));
            }
            out.push(clip(&text, 1600));
        }
        if out.len() == 30 {
            break;
        }
    }
    out
}

/// Heads of the input files the instruction names that exist in the
/// untouched workspace and aren't programs.
#[must_use]
pub fn input_heads(pristine: &Pristine) -> Vec<InputHead> {
    const PROGRAM: &[&str] = &[
        ".py", ".sh", ".rs", ".c", ".cc", ".cpp", ".h", ".go", ".js", ".ts", ".java", ".rb", ".pl",
        ".v", ".md",
    ];
    pristine
        .entries
        .iter()
        .filter(|(p, e)| {
            matches!(e.stat, Stat::File(_))
                && e.text.is_some()
                && !PROGRAM.iter().any(|x| p.to_lowercase().ends_with(x))
                && !p.to_lowercase().contains("readme")
        })
        .take(8)
        .map(|(p, e)| {
            let text = e.text.as_deref().unwrap_or_default();
            let head: String = text.lines().take(60).collect::<Vec<_>>().join("\n");
            InputHead {
                path: p.clone(),
                head: clip(&head, HEAD_CHARS),
            }
        })
        .collect()
}

struct Request {
    state: Value,
    questions: Questions,
    /// (kind, index into sentences or values) per question ID.
    ids: Vec<(String, &'static str, usize)>,
}

fn requests(task: &str, sentences: &[String], values: &[StatedValue]) -> Vec<Request> {
    let mut out = Vec::new();
    for (n, chunk) in sentences.chunks(BATCH).enumerate() {
        let mut questions = Questions::new();
        let mut ids = Vec::new();
        for j in 0..chunk.len() {
            let d = format!("defines_{j}");
            let b = format!("boundary_{j}");
            questions = questions
                .with(d.clone(), defines_at(j))
                .with(b.clone(), boundary_at(j));
            ids.push((d, "defines", n * BATCH + j));
            ids.push((b, "boundary", n * BATCH + j));
        }
        out.push(Request {
            state: json!({ "task": task, "sentences": chunk }),
            questions,
            ids,
        });
    }
    for (n, chunk) in values.chunks(BATCH).enumerate() {
        let mut questions = Questions::new();
        let mut ids = Vec::new();
        for j in 0..chunk.len() {
            let p = format!("parameter_{j}");
            questions = questions.with(p.clone(), parameter_at(j));
            ids.push((p, "parameter", n * BATCH + j));
        }
        out.push(Request {
            state: json!({
                "task": task,
                "values": chunk.iter().map(|v| json!({
                    "value": v.value,
                    "sentence": clip(&v.sentence, 600),
                })).collect::<Vec<_>>(),
            }),
            questions,
            ids,
        });
    }
    out
}

fn indexed(template: &str, j: usize) -> String {
    template.replace("[J]", &format!("[{j}]"))
}

fn defines_at(j: usize) -> Noul {
    defines(&indexed(DEFINES, j))
}

fn boundary_at(j: usize) -> Noul {
    boundary(&indexed(BOUNDARY, j))
}

fn parameter_at(j: usize) -> Noul {
    parameter(&indexed(PARAMETER, j))
}

/// What defining cost and asked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Asked {
    pub requests: usize,
    pub input_tokens: u64,
    pub usd: f64,
    pub calls: Vec<Value>,
}

/// Makes the spec for `task`: code lists the candidates, Jev picks among
/// them, and the picks become the definition, the parameters, the
/// boundaries, and the cases. `replay`, when given, answers first.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn spec(
    task: &str,
    instruction: &str,
    workdir: &str,
    pristine: &Pristine,
    references: Vec<String>,
    mode: &JevMode,
    replay: Option<&JevMode>,
    recorder: &Recorder,
) -> (Spec, Asked) {
    let words = clip(extract::strip_comments(instruction).trim(), TASK_CHARS);
    let offered = sentences(instruction);
    let mut values: Vec<StatedValue> = offered.iter().flat_map(|s| values_in(s)).collect();
    values.truncate(MAX_VALUES);
    let mut asked = Asked::default();
    let mut nouls: BTreeMap<(&'static str, usize), f64> = BTreeMap::new();
    for (n, request) in requests(&words, &offered, &values).into_iter().enumerate() {
        let one = |mode| {
            ask(
                mode,
                recorder,
                Ask {
                    component: "checks.oracle",
                    name: "jev_oracle_define",
                    id: format!("oracle-define-{task}-{n}"),
                    state: request.state.clone(),
                    questions: request.questions.clone(),
                    parent: None,
                    deadline: None,
                },
            )
        };
        let mut answer = match replay {
            Some(replay) => one(replay).await,
            None => one(mode).await,
        };
        if !answer.answered() && replay.is_some() && !matches!(mode, JevMode::Off) {
            answer = one(mode).await;
        }
        asked.requests += 1;
        if answer.how == "live" {
            let tokens = answer.input_tokens.unwrap_or(0);
            asked.input_tokens += tokens;
            #[allow(clippy::cast_precision_loss)]
            {
                asked.usd += tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0;
            }
        }
        for (id, kind, index) in &request.ids {
            if let Some(p) = answer.noul(id) {
                nouls.insert((kind, *index), p);
            }
        }
        asked.calls.push(json!({
            "key": answer.key,
            "how": answer.how,
            "questions": request.questions,
            "answers": answer.answers,
            "error": answer.error,
            "input_tokens": answer.input_tokens,
            "output_tokens": answer.output_tokens,
            "milliseconds": answer.milliseconds,
        }));
    }
    let picked = |kind: &'static str, count: usize, keep: usize| -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = (0..count)
            .filter_map(|i| nouls.get(&(kind, i)).map(|p| (i, *p)))
            .filter(|(_, p)| *p >= THRESHOLD)
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out.truncate(keep);
        out.sort_by_key(|(i, _)| *i);
        out
    };
    let definition: Vec<String> = picked("defines", offered.len(), KEEP_DEFINITION)
        .into_iter()
        .map(|(i, _)| with_blocks(instruction, &offered[i]))
        .collect();
    let boundaries: Vec<String> = picked("boundary", offered.len(), KEEP_BOUNDARIES)
        .into_iter()
        .map(|(i, _)| offered[i].clone())
        .collect();
    let parameters: Vec<Parameter> = picked("parameter", values.len(), KEEP_PARAMETERS)
        .into_iter()
        .map(|(i, p)| Parameter {
            name: values[i].name.clone(),
            value: values[i].value.clone(),
            sentence: clip(&values[i].sentence, 400),
            noul: p,
        })
        .collect();
    let mut cases = vec![CaseSpec {
        id: "O1".to_string(),
        covers: Covers {
            input: Some("the task's own inputs, as the task states them".to_string()),
            from: Some("stated".to_string()),
            ..Covers::default()
        },
    }];
    for (n, p) in parameters.iter().enumerate() {
        cases.push(CaseSpec {
            id: format!("P{}", n + 1),
            covers: Covers {
                parameter: Some(p.name.clone()),
                value: Some(p.value.clone()),
                input: None,
                from: Some("stated".to_string()),
            },
        });
    }
    for (n, b) in boundaries.iter().enumerate() {
        cases.push(CaseSpec {
            id: format!("B{}", n + 1),
            covers: Covers {
                input: Some(clip(b, 400)),
                from: Some("boundary".to_string()),
                ..Covers::default()
            },
        });
    }
    let spec = Spec {
        schema: String::new(),
        task: task.to_string(),
        workdir: workdir.to_string(),
        instruction: crate::accept::sha256(instruction.as_bytes()),
        definition,
        formats: formats(instruction, workdir),
        parameters,
        boundaries,
        inputs: input_heads(pristine),
        references,
        cases,
        jev: asked.calls.clone(),
        digest: String::new(),
    }
    .sealed();
    (spec, asked)
}
