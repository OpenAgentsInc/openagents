//! Directory listing over a read-only mount, as a `packet-v0` guest plugin.
//!
//! The proof plugin for the host's second capability import,
//! `openagents.list_dir`: the manifest declares one read-only mount, the
//! guest asks for one listing by mount index and relative path, and the
//! host confines the path exactly as it confines reads — no absolute
//! paths, no `..` escapes, no symlinks — and bounds the entries per
//! listing. The escape tests in
//! `packages/openagents-cli/test/coder-plugin-list-dir.test.ts` drive this
//! module through the real boundary.

use openagents_pdk::{list_mounted_dir, plugin_entry, MountDirListing, Refusal};
use serde::Deserialize;

#[derive(Deserialize)]
struct Input {
    /// Which declared mount to list, by manifest order. Defaults to 0.
    #[serde(default)]
    mount_index: u32,
    /// Directory path relative to that mount's root; empty lists the root.
    path: String,
}

fn handle(input: Input) -> Result<MountDirListing, Refusal> {
    list_mounted_dir(input.mount_index, &input.path)
}

plugin_entry!(handle);
