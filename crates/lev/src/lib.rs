//! The System One contract, answered by Apple's on-device foundation model.
//!
//! One state and a map of typed questions go in; one typed answer per
//! question comes out. The contract is the same one `crates/jev` sends to
//! TypeSafe and `crates/kev` serves from open weights, so a caller reaches
//! any of the three with a `base_url` change.
//!
//! What makes this implementation different is what Apple's framework
//! withholds. It returns text and typed structured values, and it exposes no
//! logits, no log-probabilities, and no hidden states. Kev's pointer head has
//! no position to read here and no head to run. So Lev derives a distribution
//! from behavior it can observe — a seeded sample ensemble, or a constrained
//! certainty band — and then earns the word "probability" by fitting a
//! calibration map against labelled outcomes. Without a fitted map for the
//! question family in hand, a Lev door refuses rather than reporting a number.
//!
//! See `docs/lev/` for the architecture, the measured behavior of the
//! runtime, and the calibration rule.

pub mod api;
pub mod bridge;
pub mod calibrate;
pub mod error;
pub mod estimator;
pub mod render;
pub mod schema;
pub mod suite;
#[cfg(feature = "serve")]
pub mod serve;

pub use api::{Answer, Extensions, NoulCriteria, Question, SystemOneRequest, SystemOneResponse, Usage};
pub use calibrate::{Map, Metrics, Observation, Record};
pub use bridge::{Availability, Bridge, Call, Outcome, Sampling};
pub use estimator::{Estimator, Raw, confidence};
pub use error::{Refusal, RefusalCode, Result};
pub use render::render;
pub use schema::{BANDS, Compiled, Kind, compile};
