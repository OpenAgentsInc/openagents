//! Jobs a linked Mac runs for its account (#11223).
//!
//! Some steps only run on macOS: Xcode and iOS builds, the iOS release gate
//! UI tests, TestFlight uploads, and desktop captures. A cloud environment,
//! or an agent in one, sends such a step to a Mac linked to the same
//! account as a typed job ([`Spec`]): a repository, a ref, and one named
//! [`Recipe`] with arguments from that recipe's allowlist. There is no
//! shell: each recipe is a fixed list of programs ([`plan`]), and the only
//! free text is the allowlisted arguments.
//!
//! The Mac reports what it can do ([`Capabilities`]): its macOS and Xcode
//! versions, its code-signing identities by name only, its simulators, and
//! whether an App Store Connect key is present (never the key). It pulls
//! the jobs the website holds for it, checks the ref out in its own
//! worktree, runs the recipe, streams the log lines ([`redact_line`]), and
//! uploads what the recipe made. Signing identities and App Store keys stay
//! on the Mac; a recipe that reaches outside ([`Spec::outward`], a store
//! upload) waits for the owner's approval first ([`UPLOAD_ABILITY`]).
//!
//! The website keeps the jobs (`openagents-web` `mac_jobs`), the Mac runs
//! them (`openagents mac serve`), and an environment submits them
//! (`openagents mac run RECIPE --ref REF`). `docs/cloud/linked-mac.md`
//! describes the whole flow.

pub mod actor;
mod capabilities;
mod plan;
mod spec;

pub use capabilities::{
    Capabilities, Simulator, asc_key_present, detect, parse_df_available_gb, parse_identities,
    parse_simctl, parse_sw_vers, parse_xcode_version,
};
pub use plan::{Collect, Places, Plan, Step, artifact_name, plan, summary_line};
pub use spec::{Kind, Recipe, Spec, valid_job_id};

/// The ability an outward-facing recipe asks the owner for, in the
/// approval policy (`coder_new::risk_policy`, #11170).
pub const UPLOAD_ABILITY: &str = "mac.upload";

/// The longest log line kept, in characters.
pub const LINE_CHARS: usize = 400;

/// A log line as it may leave the Mac: one line, control characters out,
/// at most [`LINE_CHARS`], and anything shaped like a credential redacted.
#[must_use]
pub fn redact_line(text: &str) -> String {
    let one: String = text
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    let trimmed = one.trim_end();
    let redacted = secret_screen::redact(trimmed);
    redacted.chars().take(LINE_CHARS).collect()
}

#[cfg(test)]
mod tests;
