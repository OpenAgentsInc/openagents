# patch-check

Pure-computation guest plugin: a unified diff and a file's current content
in, a placement report out, no imports. Built on `openagents-pdk` (see
`../pdk/`), which owns the whole `packet-v0` ABI — this crate is one typed
`handle` function and a `plugin_entry!` invocation. The built artifact
`patch_check.wasm` and its digest pin in `manifest.json` are checked in so
the plugin runs without a Rust toolchain.

The check validates the diff against the content *before* anyone claims it
applies: each hunk is tried at its declared position (with the line shift
from previously applied hunks accounted for), then searched within `fuzz`
lines for a unique match. The report says where each hunk lands (`at_line`,
1-based in the current content), how far off it drifted (`drift_lines`,
signed), and why a hunk failed (`context_not_found` or `ambiguous`, quoting
the first mismatching context line). One file per call; a multi-file diff
is refused. `include_preview` returns the post-application content, elided
in the middle past `max_preview_chars`.

The placement logic is unit-tested in `src/tests.rs` (pure, no host); the
sandbox test in
`packages/openagents-cli/test/coder-plugin-patch-check.test.ts` proves the
same behavior through the real WASM boundary.

Rebuild from `plugins/` (then update `artifact.digest` in `manifest.json`):
see `../README.md`.
