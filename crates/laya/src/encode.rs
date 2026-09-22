//! Flattening a request into the token sequences the model scores: the
//! Python `json.dumps` state serialization, the per-type option rendering,
//! `build_sequence`, and the padded batch the forward consumes.
//!
//! Every function mirrors `rl_common.py` line for line where the output is
//! observable; the conformance fixtures under `fixtures/` pin the parity.

use serde_json::Value;
use tokenizers::Tokenizer;

use crate::config::Specials;
use crate::error::{Error, Result};

/// The token cap the reference applies to each rendered option before the
/// head budget shrinks them further.
pub const OPTION_TOKENS: usize = 48;
/// The option-token slack the head region always keeps for instructions.
pub const HEAD_SLACK: usize = 16;
/// The smallest instruction budget `head_ids` is truncated to.
pub const MIN_HEAD: usize = 8;
/// The smallest per-option token count the even-shrink pass leaves; the
/// marker is the first token, so it always survives.
pub const MIN_OPTION: usize = 4;

/// A question normalized to the reference's internal form: a type, its
/// instruction text, and the criteria in the shape `render_options` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct InternalQuestion {
    /// The `QTYPES` index: `choice` 0, `score` 1, `noul` 2.
    pub qtype: usize,
    /// The instruction text; a non-string `instructions` is serialized.
    pub instructions: String,
    /// `choice` criteria: ordered `(name, description)` pairs.
    pub choice: Option<Vec<(String, Value)>>,
    /// `score` criteria: the ordered level descriptions.
    pub score: Option<Vec<Value>>,
    /// `noul` criteria: optional `false`/`true` descriptions.
    pub noul: Option<(Option<Value>, Option<Value>)>,
}

/// `serialize_state`: a string state is itself; any other JSON value is
/// `json.dumps(state, ensure_ascii=False)` — comma-space and colon-space
/// separators, insertion-order keys (the workspace's `serde_json` keeps
/// them), and Python's string escaping.
#[must_use]
pub fn serialize_state(state: &Value) -> String {
    match state {
        Value::String(s) => s.clone(),
        other => json_dumps(other),
    }
}

/// `json.dumps(value, ensure_ascii=False)` with Python's default
/// separators. Float formatting follows Python `repr` (`1.0`, `1e+20`,
/// `1e-05`).
#[must_use]
pub fn json_dumps(value: &Value) -> String {
    let mut out = String::new();
    dumps_at(value, &mut out);
    out
}

fn dumps_at(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&python_number(n)),
        Value::String(s) => dumps_str(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dumps_at(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (key, entry)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dumps_str(key, out);
                out.push_str(": ");
                dumps_at(entry, out);
            }
            out.push('}');
        }
    }
}

/// Python's `ensure_ascii=False` string escaping: `"` and `\` escaped,
/// `\b` `\f` `\n` `\r` `\t` short escapes, other control characters
/// `\u00XX`, everything else literal including DEL and non-ASCII.
fn dumps_str(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A JSON number in Python's repr: integers plain, floats via the
/// shortest-roundtrip digits Rust and Python share, with Python's
/// `e+XX`/`e-XX` exponent spelling.
fn python_number(n: &serde_json::Number) -> String {
    if n.is_f64() {
        python_float(n.as_f64().unwrap_or_default())
    } else {
        n.to_string()
    }
}

/// Python `repr(float)`: `1.0` keeps its point, exponents read `e+20` /
/// `e-05` with at least two digits.
fn python_float(x: f64) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let repr = format!("{x:?}");
    if let Some(at) = repr.find('e') {
        let (mantissa, exp) = repr.split_at(at);
        let exp = &exp[1..];
        let (sign, digits) = match exp.strip_prefix('-') {
            Some(d) => ("-", d),
            None => ("+", exp.strip_prefix('+').unwrap_or(exp)),
        };
        format!("{mantissa}e{sign}{digits:0>2}")
    } else if repr.contains('.') {
        repr
    } else {
        format!("{repr}.0")
    }
}

/// Python `str(value)` for the values a criteria entry or an instruction
/// can carry: `True`/`False`/`None` spellings, Python floats, strings as
/// themselves. Arrays and objects serialize through [`json_dumps`]; the
/// reference's own `str()` would emit single-quoted reprs there, a
/// documented deviation that only affects inputs upstream renders
/// unreadably anyway.
#[must_use]
pub fn py_str(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) => python_number(n),
        Value::String(s) => s.clone(),
        other => json_dumps(other),
    }
}

/// Python's truthiness on a JSON value: `null`, `false`, `0`, `""`, and
/// empty containers are falsy.
#[must_use]
pub fn py_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// `render_options`: the option texts in label order. Noul is always
/// `[false, true]` so `p[1]` is the noul probability.
#[must_use]
pub fn render_options(q: &InternalQuestion) -> Vec<String> {
    if let Some(choice) = &q.choice {
        return choice
            .iter()
            .map(|(name, description)| {
                if py_falsy(description) {
                    name.clone()
                } else {
                    format!("{}: {}", name, py_str(description))
                }
            })
            .collect();
    }
    if let Some(score) = &q.score {
        return score
            .iter()
            .enumerate()
            .map(|(i, level)| format!("level {i}: {}", py_str(level)))
            .collect();
    }
    let (no, yes) = q.noul.clone().unwrap_or_default();
    vec![
        format!(
            "false: {}",
            no.as_ref()
                .filter(|v| !py_falsy(v))
                .map_or_else(|| "no, the statement does not hold".to_string(), py_str)
        ),
        format!(
            "true: {}",
            yes.as_ref()
                .filter(|v| !py_falsy(v))
                .map_or_else(|| "yes, the statement holds".to_string(), py_str)
        ),
    ]
}

/// The number of options a question carries, for the marker-fit check and
/// the temperature bucket.
#[must_use]
pub fn option_count(q: &InternalQuestion) -> usize {
    if let Some(choice) = &q.choice {
        choice.len()
    } else if let Some(score) = &q.score {
        score.len()
    } else {
        2
    }
}

/// One encoded question row: its token ids and the positions of the
/// per-option markers.
#[derive(Clone, Debug, PartialEq)]
pub struct Encoding {
    /// `[CLS] head [SEP] (marker option)* [SEP] state [SEP]`, right-padded
    /// later by [`collate`].
    pub ids: Vec<u32>,
    /// The positions of the option markers, in option order. Each points
    /// at a mask id inside `ids`.
    pub markers: Vec<usize>,
}

/// Encode `text` with no added specials, the call shape the reference
/// uses for every span.
fn tokenize(tokenizer: &Tokenizer, text: &str) -> Result<Vec<u32>> {
    tokenizer
        .encode(text, false)
        .map(|e| e.get_ids().to_vec())
        .map_err(|e| Error::Tokenize(e.to_string()))
}

/// `build_sequence`: `[CLS] <type> instructions [SEP] [MASK] opt0 [MASK]
/// opt1 ... [SEP] state [SEP]`, bounded by `head_max_len` for the question
/// half and `max_len` overall.
///
/// The argument list mirrors the reference signature rather than a
/// struct, so each site lines up with `rl_common.build_sequence`.
///
/// `truncate_left` mirrors the episode path: when set and the state fits
/// no room, the reference keeps the whole state and lets the final cap
/// drop its tail — preserved here for parity even though serving always
/// truncates right.
///
/// # Errors
///
/// Returns [`Error::Tokenize`] when the tokenizer fails.
#[allow(clippy::too_many_arguments)]
pub fn build_sequence(
    tokenizer: &Tokenizer,
    specials: &Specials,
    state: &str,
    q: &InternalQuestion,
    option_order: Option<&[usize]>,
    max_len: usize,
    head_max_len: usize,
    truncate_left: bool,
) -> Result<Encoding> {
    let opts = render_options(q);
    let default_order: Vec<usize> = (0..opts.len()).collect();
    let order = option_order.unwrap_or(&default_order);
    let ins = q.instructions.replace(&specials.mask_token, " ");
    let mut head_ids = tokenize(
        tokenizer,
        &format!("{} question: {ins}", qtype_label(q.qtype)),
    )?;
    let mut opt_ids: Vec<Vec<u32>> = Vec::with_capacity(order.len());
    for &i in order {
        let text = opts[i].replace(&specials.mask_token, " ");
        let mut ids = vec![specials.mask_id];
        let mut text_ids = tokenize(tokenizer, &format!(" {text}"))?;
        text_ids.truncate(OPTION_TOKENS);
        ids.extend(text_ids);
        opt_ids.push(ids);
    }
    let mut opt_budget = head_max_len.saturating_sub(opt_ids.iter().map(Vec::len).sum::<usize>());
    if opt_budget < HEAD_SLACK {
        let per = (head_max_len.saturating_sub(HEAD_SLACK) / opt_ids.len().max(1)).max(MIN_OPTION);
        for ids in &mut opt_ids {
            ids.truncate(per);
        }
        opt_budget = head_max_len.saturating_sub(opt_ids.iter().map(Vec::len).sum::<usize>());
    }
    head_ids.truncate(opt_budget.max(MIN_HEAD));
    let mut ids = vec![specials.cls_id];
    ids.extend(&head_ids);
    ids.push(specials.sep_id);
    let mut markers = Vec::with_capacity(opt_ids.len());
    for opt in &opt_ids {
        markers.push(ids.len());
        ids.extend(opt);
    }
    ids.push(specials.sep_id);
    let room = max_len.saturating_sub(ids.len() + 1);
    let mut st = tokenize(tokenizer, &state.replace(&specials.mask_token, " "))?;
    if truncate_left {
        // `st[-room:]` in the reference: `room == 0` selects the whole
        // tail, and the final `ids[:max_len]` does the cutting.
        if room > 0 && st.len() > room {
            st.drain(..st.len() - room);
        }
    } else {
        st.truncate(room);
    }
    ids.extend(&st);
    ids.push(specials.sep_id);
    ids.truncate(max_len);
    markers.retain(|m| *m < max_len);
    Ok(Encoding { ids, markers })
}

fn qtype_label(qtype: usize) -> &'static str {
    crate::config::qtype_name(qtype)
}

/// One row of a padded batch: the encoding plus its type index.
pub struct Item {
    /// The encoded sequence.
    pub encoding: Encoding,
    /// The `QTYPES` index.
    pub qtype: usize,
}

/// `collate_items` for one request: right-pad to the longest row, gather
/// marker positions into a fixed-width grid, and count real tokens.
pub struct Batch {
    /// `[rows, longest]` padded ids.
    pub input_ids: Vec<Vec<u32>>,
    /// `1` on real tokens, `0` on padding.
    pub attention_mask: Vec<Vec<u32>>,
    /// `[rows, max options]` marker positions (zeroed under mask).
    pub marker_pos: Vec<Vec<u32>>,
    /// `true` where `marker_pos` names a real option.
    pub marker_mask: Vec<Vec<bool>>,
    /// Per-row question type.
    pub qtype: Vec<usize>,
    /// Total non-padding tokens; the `usage.input_tokens` figure.
    pub n_tokens: usize,
}

/// Pad the rows of one request into the batch the model consumes.
#[must_use]
pub fn collate(items: &[Item], pad_id: u32) -> Batch {
    let n = items.len();
    let longest = items
        .iter()
        .map(|it| it.encoding.ids.len())
        .max()
        .unwrap_or(0);
    let kmax = items
        .iter()
        .map(|it| it.encoding.markers.len())
        .max()
        .unwrap_or(0);
    let mut input_ids = vec![vec![pad_id; longest]; n];
    let mut attention_mask = vec![vec![0u32; longest]; n];
    let mut marker_pos = vec![vec![0u32; kmax]; n];
    let mut marker_mask = vec![vec![false; kmax]; n];
    let mut qtype = Vec::with_capacity(n);
    let mut n_tokens = 0;
    for (i, item) in items.iter().enumerate() {
        let len = item.encoding.ids.len();
        input_ids[i][..len].copy_from_slice(&item.encoding.ids);
        for slot in attention_mask[i].iter_mut().take(len) {
            *slot = 1;
        }
        n_tokens += len;
        for (j, &m) in item.encoding.markers.iter().enumerate() {
            marker_pos[i][j] = m as u32;
            marker_mask[i][j] = true;
        }
        qtype.push(item.qtype);
    }
    Batch {
        input_ids,
        attention_mask,
        marker_pos,
        marker_mask,
        qtype,
        n_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn internal(kind: &str, ins: &str, crit: Value) -> InternalQuestion {
        match kind {
            "choice" => {
                let obj = crit.as_object().expect("choice criteria object");
                InternalQuestion {
                    qtype: crate::config::QTYPE_CHOICE,
                    instructions: ins.to_string(),
                    choice: Some(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
                    score: None,
                    noul: None,
                }
            }
            "score" => InternalQuestion {
                qtype: crate::config::QTYPE_SCORE,
                instructions: ins.to_string(),
                choice: None,
                score: Some(crit.as_array().expect("score criteria array").clone()),
                noul: None,
            },
            _ => InternalQuestion {
                qtype: crate::config::QTYPE_NOUL,
                instructions: ins.to_string(),
                choice: None,
                score: None,
                noul: Some((crit.get("false").cloned(), crit.get("true").cloned())),
            },
        }
    }

    #[test]
    fn serialize_state_matches_python_json_dumps() {
        // Python: json.dumps({"a": 1, "b": [1.5, "x"], "c": True},
        //   ensure_ascii=False) == '{"a": 1, "b": [1.5, "x"], "c": true}'
        assert_eq!(
            serialize_state(&json!({"a": 1, "b": [1.5, "x"], "c": true})),
            r#"{"a": 1, "b": [1.5, "x"], "c": true}"#
        );
        assert_eq!(serialize_state(&json!("already text")), "already text");
        assert_eq!(
            serialize_state(&json!([1, "two", null])),
            r#"[1, "two", null]"#
        );
        assert_eq!(serialize_state(&json!({"n": 1.0})), r#"{"n": 1.0}"#);
        assert_eq!(
            serialize_state(&json!({"uni": "héllo"})),
            r#"{"uni": "héllo"}"#
        );
        assert_eq!(
            serialize_state(&json!({"esc": "a\nb\"c"})),
            r#"{"esc": "a\nb\"c"}"#
        );
        assert_eq!(serialize_state(&json!(1e20)), "1e+20");
        assert_eq!(serialize_state(&json!(1e-5)), "1e-05");
        assert_eq!(serialize_state(&json!(-0.5)), "-0.5");
    }

    #[test]
    fn py_str_spells_python_scalars() {
        assert_eq!(py_str(&json!(true)), "True");
        assert_eq!(py_str(&json!(false)), "False");
        assert_eq!(py_str(&Value::Null), "None");
        assert_eq!(py_str(&json!(3)), "3");
        assert_eq!(py_str(&json!(2.5)), "2.5");
        assert_eq!(py_str(&json!("text")), "text");
    }

    #[test]
    fn render_options_follows_type_conventions() {
        let noul = internal("noul", "q?", json!({}));
        assert_eq!(
            render_options(&noul),
            vec![
                "false: no, the statement does not hold",
                "true: yes, the statement holds"
            ]
        );
        let noul_desc = internal("noul", "q?", json!({"true": "refund asked"}));
        assert_eq!(
            render_options(&noul_desc),
            vec![
                "false: no, the statement does not hold",
                "true: refund asked"
            ]
        );
        let choice = internal(
            "choice",
            "q?",
            json!({"billing": "money questions", "shipping": null}),
        );
        assert_eq!(
            render_options(&choice),
            vec!["billing: money questions", "shipping"]
        );
        // Python `or` treats "" as falsy: empty description -> bare key.
        let empty_desc = internal("choice", "q?", json!({"a": ""}));
        assert_eq!(render_options(&empty_desc), vec!["a"]);
        let score = internal("score", "q?", json!(["low", "mid", "high"]));
        assert_eq!(
            render_options(&score),
            vec!["level 0: low", "level 1: mid", "level 2: high"]
        );
    }

    #[test]
    fn collate_pads_rows_and_counts_real_tokens() {
        let items = vec![
            Item {
                encoding: Encoding {
                    ids: vec![1, 2, 3, 4, 5],
                    markers: vec![2, 4],
                },
                qtype: 0,
            },
            Item {
                encoding: Encoding {
                    ids: vec![9, 8],
                    markers: vec![1],
                },
                qtype: 2,
            },
        ];
        let batch = collate(&items, 0);
        assert_eq!(
            batch.input_ids,
            vec![vec![1, 2, 3, 4, 5], vec![9, 8, 0, 0, 0]]
        );
        assert_eq!(
            batch.attention_mask,
            vec![vec![1, 1, 1, 1, 1], vec![1, 1, 0, 0, 0]]
        );
        assert_eq!(batch.marker_pos, vec![vec![2, 4], vec![1, 0]]);
        assert_eq!(batch.marker_mask, vec![vec![true, true], vec![true, false]]);
        assert_eq!(batch.qtype, vec![0, 2]);
        assert_eq!(batch.n_tokens, 7);
    }
}
