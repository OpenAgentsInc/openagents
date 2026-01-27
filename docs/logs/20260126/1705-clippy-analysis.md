# Clippy Analysis (autopilot-desktop focus)

Run: `cargo clippy -p autopilot-desktop --all-targets` (log captured at `/tmp/clippy_autopilot_desktop.log`).

## Update (2026-01-27)
Several items listed below were already addressed in-tree after this log was captured:
- `apps/autopilot-desktop/src-tauri/build.rs` now uses `tauri_build::try_build` with a codegen context.
- `crates/ai-server/src/lib.rs` uses `tracing` (no `println!`/`eprintln!`) and handles `Mutex::lock` errors.
- `crates/codex-client/src/client.rs` derives `Default` for `AppServerConfig`.
- `crates/openagents-utils/src/filenames.rs` already uses the collapsible replace form for char sets.

Re-run clippy to refresh the exact remaining warnings before large cleanup passes.

## Previously blocking errors (now addressed in-tree)
### `crates/ai-server/src/lib.rs`
The original failures were from `println!`/`eprintln!` and `lock().unwrap()`. Those have since been replaced with tracing + explicit error handling.

## Secondary warnings (non-blocking, likely still present)
### `crates/nostr/core/*` (large volume)
Common lint categories:
- `clippy::collapsible_if`
- `clippy::should_implement_trait` for custom `from_str`
- `clippy::inherent_to_string`
- `clippy::get_first`
- `clippy::map_clone`
- `clippy::needless_borrows_for_generic_args`
- `clippy::len_zero`

Given volume, recommend deferring until blocking errors are resolved.

### `crates/lm-router` / `crates/gpt-oss`
- Mostly `collapsible_if`, `explicit_auto_deref`, `needless_borrows_for_generic_args`.

## Recommended cleanup order
1) **Re-run clippy** to confirm current error surface.
2) **lm-router/gpt-oss**: small style fixes.
3) **nostr/core**: large batch cleanups (optionally defer or apply `clippy::allow` if not urgent).

## Notes
- The clippy run is scoped to `autopilot-desktop` but still checks dependent crates, so dependency lint errors block the run.
- Full log is in `/tmp/clippy_autopilot_desktop.log` for exact line references.

## Work log (2026-01-27)
- This log predates several fixes; see the 2026-01-27 update note above.
- Clippy has not been re-run as part of this doc update; re-run to refresh the exact warnings list.
