//! Retail cloud acceptance, funded-qualification planning, and the
//! fail-closed launch gate (`docs/cloud/retail-qualification.md`).
//!
//! - [`harness`]: an isolated world of fakes in a temporary directory.
//! - [`acceptance`]: the integrated fake-payment acceptance run (#10722).

pub mod acceptance;
pub mod harness;
