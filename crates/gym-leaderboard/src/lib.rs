//! The Gym's published benchmark results.
//!
//! This crate turns committed Terminal-Bench evidence (an experiment's
//! `attempts.json`, a study's round report, and the retained traces) into
//! the two files the Verse Gym and any other reader load:
//!
//! - `leaderboard.v1.json` ([`contract::Leaderboard`]): every board with
//!   its tallies, per-task rows, per-attempt rows, labels, caveats, and the
//!   digest of every evidence file it read.
//! - `traces/<board>/<attempt>.json` ([`contract::TraceBundle`]): one
//!   attempt's instruction, Jev decision, briefing, timed delegate steps,
//!   and verifier result, scrubbed and bounded for a phone.
//!
//! No number is typed by hand: each is computed from the evidence, and
//! each source adapter recomputes the study's verdicts and refuses to
//! build when they disagree with what the study recorded. The output is
//! deterministic, so `check` can regenerate it and compare bytes.
//!
//! Read `docs/verse/gym-leaderboard.md` before changing the contract.

//!
//! Features: `generate` (on by default, and required by the binary) builds
//! and checks the publication and pulls in `gym`, `knowledge`, and
//! `regex`. Without it the crate is the contract, digest verification
//! ([`verify`]), and the view model, small enough for a phone; `client`
//! adds the fetch-verify-cache client ([`client`]).

pub mod contract;
pub mod evidence;
pub mod verify;
pub mod view;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "generate")]
pub mod bundle;
#[cfg(feature = "generate")]
mod generator;
#[cfg(feature = "generate")]
pub mod microcoder;
#[cfg(feature = "generate")]
pub mod reference_boards;
#[cfg(feature = "generate")]
pub mod scrub;
#[cfg(feature = "generate")]
pub mod study;
#[cfg(feature = "generate")]
pub mod tb21_oos;
#[cfg(feature = "generate")]
pub mod tb4_delegate;
#[cfg(feature = "generate")]
pub mod tb4_delegate_dev;
#[cfg(feature = "generate")]
pub mod tb4_microcoder_kb;
#[cfg(feature = "generate")]
pub mod tb4_oos;

#[cfg(feature = "generate")]
pub use generator::{Output, REVIEWS, REVIEWS_SCHEMA, Review, check, generate, write};

/// A beat whose cost or time margin is under this fraction is labeled
/// [`contract::Label::ThinMargin`].
pub const THIN_MARGIN: f64 = 0.05;

/// The leaderboard's serialized size bound. A phone fetches it on entry.
pub const MAX_LEADERBOARD_BYTES: usize = 512 * 1024;

/// A trace bundle's serialized size bound. A phone fetches one on demand.
pub const MAX_BUNDLE_BYTES: usize = 256 * 1024;

/// Where the publication lives, repository-relative.
pub const PUBLISHED: &str = "bench/terminal-bench/published";

/// The leaderboard's file name inside the publication.
pub const LEADERBOARD_FILE: &str = "leaderboard.v1.json";

/// The index's file name inside the publication.
pub const INDEX_FILE: &str = "index.json";

/// The path a board's attempt's bundle is published at.
#[must_use]
pub fn trace_path(board: &str, attempt: &str) -> String {
    format!("traces/{board}/{attempt}.json")
}

/// A fraction as a percentage: one decimal under 10%, none above, and no
/// trailing `.0`.
#[must_use]
pub fn pct(fraction: f64) -> String {
    let p = fraction * 100.0;
    let text = if p.abs() < 10.0 {
        format!("{p:.1}")
    } else {
        format!("{p:.0}")
    };
    format!("{}%", text.strip_suffix(".0").unwrap_or(&text))
}

/// Dollars: four decimals under $1, two above.
#[must_use]
pub fn usd(amount: f64) -> String {
    if amount.abs() < 1.0 {
        format!("${amount:.4}")
    } else {
        format!("${amount:.2}")
    }
}

/// The median of `values`, or `None` when empty.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[mid]
    } else {
        f64::midpoint(v[mid - 1], v[mid])
    })
}
