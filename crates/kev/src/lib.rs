//! A decision model: one state and a map of typed questions in, one
//! probability distribution per question out, in a single prefill pass with
//! no decoding.
//!
//! This crate is a Rust port of the mechanism in
//! [`jaredpalmer/kev`](https://github.com/jaredpalmer/kev): a frozen causal
//! backbone plus a LoRA adapter and a small pointer head, run under a
//! block-causal mask that gives every question branch access to the shared
//! state and to nothing else. The wire contract is TypeSafe's
//! `POST /v1/systemone`, the same contract the `jev` client crate speaks.
//!
//! Conformance fixtures under `fixtures/` are generated from the Python
//! reference by `fixtures/gen_fixtures.py`; see `docs/kev/` for the
//! architecture, the measurements, and the port roadmap.

pub mod api;
pub mod error;
pub mod render;

pub use api::{
    Answer, ChoiceAnswer, Meta, NoulAnswer, Question, Record, RecordQuestion, ScoreAnswer,
    SystemOneRequest, choice_confidence, r2, score_confidence, to_answers, to_record,
};
pub use error::{Error, MAX_OPTIONS, Result};
pub use render::{option_text, render, sanitize};
