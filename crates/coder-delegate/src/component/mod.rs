//! The components a terminal turn's judge and briefing use: Jev calls
//! and their records ([`jev`]), the evidence Jev reads ([`evidence`]), and
//! the briefing packer's record ([`pack`]). Coder One's `component` module
//! re-exports them beside the components only its episodes run.

pub mod evidence;
pub mod jev;
pub mod pack;
