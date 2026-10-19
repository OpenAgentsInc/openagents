//! The desktop app's offline demo data, compiled from the client crate's own
//! files so the web page shows exactly what `--demo` shows on the desktop.
#![allow(dead_code, unused_imports)]

#[path = "../../../crates/client/src/demo/fixtures.rs"]
pub(crate) mod fixtures;
#[path = "../../../crates/client/src/demo/png.rs"]
pub(crate) mod png;
#[path = "../../../crates/client/src/demo/transcripts.rs"]
pub(crate) mod transcripts;
#[path = "../../../crates/client/src/demo/visuals.rs"]
pub(crate) mod visuals;
