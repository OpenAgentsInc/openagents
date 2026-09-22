//! A decision model: one state and a map of typed questions in, one
//! probability distribution per question out, in a single encoder pass
//! with no decoding.
//!
//! This crate is a Rust port of the laya mechanism described in
//! `docs/decision-models/others/2026-09-19-laya.md`: a frozen ModernBERT
//! encoder plus a from-scratch decision head that scores a `[MASK]`
//! marker per option. The wire contract is TypeSafe's
//! `POST /v1/systemone`, the same contract the `jev` client crate speaks.
//!
//! Conformance fixtures under `fixtures/` are generated from the Python
//! reference by `fixtures/gen_fixtures.py`; see `docs/laya/` for the
//! architecture, the measurements, and the artifact layout.

pub mod api;
pub mod artifacts;
pub mod config;
pub mod decision;
pub mod encode;
pub mod error;
pub mod head;
pub mod model;
#[cfg(feature = "serve")]
pub mod serve;

pub use api::{
    Answer, ChoiceAnswer, ChoiceCriteria, Meta, NoulAnswer, Question, RlAgent, ScoreAnswer,
    SystemOneRequest, confidence, r4, to_answers,
};
pub use artifacts::ArtifactIdentity;
pub use config::{
    EncoderConfig, HeadConfig, QTYPE_CHOICE, QTYPE_NOUL, QTYPE_SCORE, Specials, qtype_name,
    temp_bucket,
};
pub use decision::DecisionModel;
pub use encode::{
    Batch, Encoding, InternalQuestion, Item, build_sequence, collate, json_dumps, option_count,
    py_str, render_options, serialize_state,
};
pub use error::{Bound, Error, MAX_OPTIONS, RefusalCode, Result};
pub use head::Head;
pub use model::Backbone;
