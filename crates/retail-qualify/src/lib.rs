//! Retail cloud acceptance, funded-qualification planning, and the
//! fail-closed launch gate (`docs/cloud/retail-qualification.md`).
//!
//! - [`harness`]: an isolated world of fakes in a temporary directory.
//! - [`acceptance`]: the integrated fake-payment acceptance run (#10722).
//! - [`launch`]: the fail-closed launch gate and operator health (#10724).
//! - [`qualify`]: the bounded funded-qualification plan and runner (#10723).

pub mod acceptance;
pub mod harness;
pub mod launch;
pub mod qualify;
