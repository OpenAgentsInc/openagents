//! File statistics over a read-only mount, as a `packet-v0` guest plugin.
//!
//! This is the proof plugin for the host's first capability import: the
//! manifest declares one read-only mount, so the host exposes
//! `openagents.read_file` to this module and confines every path it asks
//! for — relative paths only, no `..` escape, no symlinks, a per-file size
//! bound. The plugin itself holds no authority: it hands the host a path
//! and gets back bytes or a typed refusal, which the PDK surfaces as a
//! plain `Result`.

use openagents_pdk::{plugin_entry, read_mounted_file, Refusal};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    /// Path relative to a declared mount root.
    path: String,
}

#[derive(Serialize)]
struct Output {
    path: String,
    bytes: usize,
    utf8: bool,
    /// Line count when the file is UTF-8 text; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<usize>,
}

fn handle(input: Input) -> Result<Output, Refusal> {
    let bytes = read_mounted_file(&input.path)?;
    let lines = std::str::from_utf8(&bytes).ok().map(|text| {
        if text.is_empty() {
            0
        } else {
            text.lines().count()
        }
    });
    Ok(Output {
        path: input.path,
        bytes: bytes.len(),
        utf8: lines.is_some(),
        lines,
    })
}

plugin_entry!(handle);
