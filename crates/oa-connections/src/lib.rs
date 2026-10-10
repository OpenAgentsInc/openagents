//! Connections: third-party software a person connects to OpenAgents
//! (#11238), starting with Google Drive.
//!
//! The model is harvested from UsefulSoftwareCo/executor (MIT; its
//! `vision.md`), rebuilt here in Rust with no runtime dependency on it:
//!
//! - An [`core::Integration`] is one API surface and its tools. It is
//!   produced from a spec ([`core::SpecKind`]: Google Discovery today,
//!   OpenAPI next) or declared first-party, and it says how it
//!   authenticates ([`core::AuthTemplate`]).
//! - A [`core::Connection`] is a named credential for one integration,
//!   placed in one [`core::Scope`] (an account, a workspace, or a
//!   project). It holds a [`core::SecretRef`], a pointer the server
//!   resolves at call time; the secret never reaches a model, a tool's
//!   arguments or result, or a page.
//! - A [`core::Policy`] gates each tool: allow, require approval, or
//!   block. The default comes from the HTTP method (reads are allowed,
//!   writes need approval); rules attached to an integration, a
//!   connection, or one tool override it, an inner scope can't weaken an
//!   outer scope's rule ([`core::effective_policy`]).
//! - A tool is addressed `<integration>.<scope>.<connection>.<tool>`
//!   ([`core::ToolAddress`]).
//!
//! [`google`] holds the first integration: OAuth with incremental consent
//! ([`google::oauth`]), tools derived from a Discovery document
//! ([`discovery`]), and the curated read-only Drive tools
//! ([`google::drive`]): `drive.search`, `drive.list_folder`, and
//! `drive.read` (Docs as text, Sheets as CSV, PDFs as text through a
//! [`google::drive::PdfText`] the host provides).

pub mod core;
pub mod discovery;
#[cfg(feature = "fake")]
pub mod fake;
pub mod google;

pub use core::{
    AuthTemplate, Connection, Integration, Policy, PolicyRule, PolicyTarget, Scope, ScopeKind,
    SecretRef, SpecKind, ToolAddress, ToolSpec, effective_policy,
};
