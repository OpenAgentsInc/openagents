//! The Agent Trajectory Interchange Format: a session as ordered steps.
//!
//! The transcript a person reads is rows in a terminal, which is the wrong
//! shape for analysis: it says what a person saw, not what the agent did. A
//! session therefore keeps a second record of itself as ordered steps, and
//! this crate is that record — both the format, in [`document`], and the
//! file a running session appends to, in [`log`].
//!
//! The format is ATIF as the Harbor trajectory RFC defines it, reimplemented
//! here from the reference implementation in `~/work/coder`. It is a schema,
//! not service code: nothing in this crate opens a socket, and nothing in it
//! knows where a trace goes afterwards.
//!
//! # Why this format
//!
//! Because a decision-model call is first-class data in it. A shell command
//! and a question put to Jev, Kev, or Lev are both [`Call`]s, and
//! [`Call::extra`] carries what the host knows beyond the wire fields — which
//! door answered, what it was asked, what it said, and the digest of the
//! state it read. [`Decision`] builds one.
//!
//! That is the half of the picture the Gym's rows cannot supply. A row is
//! per-decision: one door, one state, one answer, comparable across doors. A
//! trace is per-episode: ordered, and holding what happened *next*, which is
//! what an outcome label is derived from.
//!
//! # What is not capped
//!
//! Nothing here truncates a tool result. Capping output is what makes a trace
//! useless for the analysis it exists for: a record of a command whose output
//! has been cut is a record that cannot answer whether the agent had what it
//! needed. A trace holds what the process held. Where the process itself
//! already bounded something — `coder`'s shell caps its capture, and shows its
//! judge less than it captured — the call's `extra` records that bound, so a
//! reader can tell what the agent saw from what the machine said.
//!
//! # Writing one
//!
//! ```no_run
//! use atif::{Log, Session, Source, Step, log};
//!
//! let dir = log::default_dir().expect("a home directory");
//! let session = Session::opening(
//!     &log::session_id(atif::now_ms()),
//!     "google/gemini-3.8-flash",
//!     "live",
//!     "/Users/someone/work/openagents",
//!     env!("CARGO_PKG_VERSION"),
//! );
//! let mut log = Log::create(&dir, &session)?;
//! log.append(&Step::said(Source::User, "what crates are here"))?;
//! log.finish(atif::log::ENDED)?;
//!
//! let document = atif::log::read(log.path())?.document();
//! # Ok::<(), std::io::Error>(())
//! ```

pub mod document;
pub mod log;

pub use document::{
    AGENT_NAME, Attempt, Call, DECISION_CALL_SCHEMA, Decision, EXPORTER, Outcome, SCHEMA_VERSION,
    Session, Source, Step, Usage, digest, document, intent, iso, now_ms, stamp,
};
pub use log::{Fault, FaultKind, Log, Recording};
