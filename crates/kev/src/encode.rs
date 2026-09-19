//! Packing one [`Record`] into the token sequence the backbone runs, and the
//! block-causal mask that isolates question branches.
//!
//! Mirrors `kev/model.py::encode` and `branch_mask_batch`: `<|fim_prefix|>`
//! opens the state, each question is `<|fim_middle|>` instructions then
//! `<|box_start|> … <|box_end|>` option spans then `<|fim_suffix|>` as the
//! decision token. Segment ids mark the shared state as `0` and question `k`
//! as `k`; branch positions restart after the state. A token attends to
//! earlier tokens in the state or in its own question only.

use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use crate::api::Record;
use crate::error::{Error, Result};
use crate::render::sanitize;

/// The five Qwen special tokens Kev reuses as delimiters, in role order:
/// state, question, option open, option close, decide.
pub const SPECIAL: [&str; 5] = [
    "<|fim_prefix|>",
    "<|fim_middle|>",
    "<|box_start|>",
    "<|box_end|>",
    "<|fim_suffix|>",
];

/// State-token budget the reference trains on.
pub const MAX_STATE: usize = 384;
/// Per-question branch budget the reference trains on.
pub const MAX_BRANCH: usize = 1024;

/// `opt` value for state and instruction tokens: inside no option span.
pub const OPT_NONE: i64 = -1;
/// `opt` value for the `<|fim_suffix|>` decision token.
pub const OPT_DECIDE: i64 = -2;

/// One record packed for the backbone: the token ids plus the bookkeeping the
/// mask and the pointer head read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Encoding {
    /// The packed token ids.
    pub ids: Vec<u32>,
    /// Segment per token: `0` for the shared state, `k` for question `k`.
    pub seg: Vec<i64>,
    /// Position per token; each branch restarts after the state.
    pub pos: Vec<i64>,
    /// Option span per token: [`OPT_NONE`] outside options, `0..K` inside a
    /// span, [`OPT_DECIDE`] on the decision token.
    pub opt: Vec<i64>,
    /// Index of each question's decision token.
    pub decide_idx: Vec<usize>,
    /// Index of each option's closing delimiter, per question.
    pub opt_idx: Vec<Vec<usize>>,
    /// Whether every option span is its own sub-branch.
    pub option_isolation: bool,
    /// The labelled outcome per question.
    pub labels: Vec<usize>,
    /// Whether the state lost tokens to the `max_state` bound.
    pub state_truncated: bool,
}

/// Tokenize caller-supplied text so it can never produce a delimiter or
/// control token: `<|name|>` spans are rewritten before tokenization, and no
/// special tokens are added.
///
/// # Errors
///
/// Returns [`Error::Tokenize`] when the tokenizer rejects the text.
pub fn user_tokens(tokenizer: &Tokenizer, text: &str) -> Result<Vec<u32>> {
    let encoding = tokenizer
        .encode(sanitize(text).as_ref(), false)
        .map_err(|e| Error::Tokenize(e.to_string()))?;
    Ok(encoding.get_ids().to_vec())
}

/// The resolved id of one delimiter token.
fn special_id(tokenizer: &Tokenizer, name: &str) -> Result<u32> {
    tokenizer
        .token_to_id(name)
        .ok_or_else(|| Error::Tokenize(format!("tokenizer has no {name} token")))
}

/// Pack `record` exactly as `kev.model.encode` does.
///
/// With `option_isolation`, every option span becomes its own sub-branch that
/// sees the state, the instructions, and itself only; all spans share position
/// ids and the decision token sits one position past the longest span, which
/// makes option representations permutation-invariant by construction.
///
/// # Errors
///
/// Returns [`Error::StateTooLong`] under `strict` when the state exceeds
/// `max_state`, [`Error::BranchTooLong`] when a question branch plus the state
/// exceeds `max_branch`, and [`Error::Tokenize`] on tokenizer failures.
pub fn encode(
    tokenizer: &Tokenizer,
    record: &Record,
    max_state: usize,
    max_branch: usize,
    strict: bool,
    option_isolation: bool,
) -> Result<Encoding> {
    let state_tokens = user_tokens(tokenizer, &record.state)?;
    if strict && state_tokens.len() + 1 > max_state {
        return Err(Error::StateTooLong {
            tokens: state_tokens.len() + 1,
            max: max_state,
        });
    }
    let s_id = special_id(tokenizer, SPECIAL[0])?;
    let mut ids = vec![s_id];
    ids.extend_from_slice(&state_tokens[..state_tokens.len().min(max_state - 1)]);
    let state_len = ids.len();
    let mut seg = vec![0i64; state_len];
    let mut pos: Vec<i64> = (0..state_len as i64).collect();
    let mut opt = vec![OPT_NONE; state_len];
    let (q_id, o_id, c_id, d_id) = (
        special_id(tokenizer, SPECIAL[1])?,
        special_id(tokenizer, SPECIAL[2])?,
        special_id(tokenizer, SPECIAL[3])?,
        special_id(tokenizer, SPECIAL[4])?,
    );
    let mut decide_idx = Vec::with_capacity(record.questions.len());
    let mut opt_idx = Vec::with_capacity(record.questions.len());
    for (k, q) in record.questions.iter().enumerate() {
        let k = k as i64 + 1;
        let mut branch = vec![q_id];
        branch.extend(user_tokens(tokenizer, &q.instr)?);
        let instr_len = branch.len();
        let mut spans = Vec::with_capacity(q.options.len());
        for option in &q.options {
            let mut span = vec![o_id];
            span.extend(user_tokens(tokenizer, option)?);
            span.push(c_id);
            spans.push(span);
        }
        let mut branch_opt = vec![OPT_NONE; instr_len];
        let mut branch_pos: Vec<i64> = (state_len as i64..state_len as i64 + instr_len as i64)
            .collect();
        let mut ends = Vec::with_capacity(spans.len());
        let mut cursor = instr_len;
        let longest = spans.iter().map(Vec::len).max().unwrap_or(0);
        for (j, span) in spans.iter().enumerate() {
            branch.extend_from_slice(span);
            branch_opt.extend(std::iter::repeat_n(j as i64, span.len()));
            if option_isolation {
                branch_pos.extend(
                    (0..span.len() as i64).map(|i| state_len as i64 + instr_len as i64 + i),
                );
            } else {
                branch_pos.extend(
                    (0..span.len() as i64)
                        .map(|i| state_len as i64 + cursor as i64 + i),
                );
            }
            cursor += span.len();
            ends.push(cursor - 1);
        }
        branch.push(d_id);
        branch_opt.push(OPT_DECIDE);
        if option_isolation {
            branch_pos.push(state_len as i64 + instr_len as i64 + longest as i64);
        } else {
            branch_pos.push(state_len as i64 + cursor as i64);
        }
        let budget = max_branch.saturating_sub(state_len);
        if branch.len() > budget {
            return Err(Error::BranchTooLong {
                tokens: branch.len(),
                max: budget,
            });
        }
        let base = ids.len();
        decide_idx.push(base + branch.len() - 1);
        opt_idx.push(ends.iter().map(|e| base + e).collect());
        ids.extend_from_slice(&branch);
        seg.extend(std::iter::repeat_n(k, branch.len()));
        pos.extend_from_slice(&branch_pos);
        opt.extend_from_slice(&branch_opt);
    }
    Ok(Encoding {
        ids,
        seg,
        pos,
        opt,
        decide_idx,
        opt_idx,
        option_isolation,
        labels: record.questions.iter().map(|q| q.label).collect(),
        state_truncated: state_tokens.len() + 1 > max_state,
    })
}

/// The block-causal mask for a batch of encodings, right-padded to the
/// longest sequence: `allow[b][i][j]` is true when query `i` may attend to
/// key `j`.
///
/// A token attends to earlier tokens in the state (`seg 0`) or in its own
/// segment. Padded keys are masked for every query; padded query rows keep
/// their diagonal so no row is fully masked. With `opts`, option-span tokens
/// additionally see only the state, their question's instructions, and their
/// own span; the decision token sees its whole question.
#[must_use]
pub fn branch_mask(segs: &[Vec<i64>], opts: Option<&[Vec<i64>]>) -> Vec<Vec<Vec<bool>>> {
    let len = segs.iter().map(Vec::len).max().unwrap_or(0);
    let batch = segs.len();
    let mut allow = vec![vec![vec![false; len]; len]; batch];
    for (b, seg) in segs.iter().enumerate() {
        let opt_row: Vec<i64> = opts.map_or_else(
            || vec![OPT_NONE; seg.len()],
            |o| o[b].clone(),
        );
        for i in 0..len {
            let seg_i = if i < seg.len() { seg[i] } else { -1 };
            let opt_i = if i < opt_row.len() { opt_row[i] } else { OPT_NONE };
            for j in 0..=i.min(len - 1) {
                if j >= seg.len() {
                    break;
                }
                let seg_j = seg[j];
                let opt_j = opt_row[j];
                let mut yes = seg_j == 0 || seg_j == seg_i;
                if yes && opts.is_some() && opt_j >= 0 {
                    yes = opt_i == OPT_DECIDE || opt_j == opt_i;
                }
                allow[b][i][j] = yes;
            }
            // Diagonal survives padding so no row is fully masked.
            if i < len {
                allow[b][i][i] = true;
            }
        }
    }
    allow
}
