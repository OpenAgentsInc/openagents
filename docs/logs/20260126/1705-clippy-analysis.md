# Clippy Analysis (autopilot-desktop focus)

Run: `cargo clippy -p autopilot-desktop --all-targets` (log captured at `/tmp/clippy_autopilot_desktop.log`).

## Autopilot Desktop (direct warnings)
1) `apps/autopilot-desktop/src-tauri/build.rs`
   - `clippy::semicolon_if_nothing_returned`: add `;` to `tauri_build::build()`.
   - Fix: `tauri_build::build();`

## Blocking errors (must fix before clippy can pass)
### `crates/ai-server/src/lib.rs`
Clippy errors are hard failures due to stricter rules:

**Print to stdout/stderr (disallowed)**
- `clippy::print-stdout` errors at lines emitting `println!`:
  - Already running on port
  - Starting server
  - Server started
  - Stopping server
  - Server stopped
  - Restarting server
  - Installing dependencies
  - Dependencies installed
  - Waiting for ready
  - Server is ready
- `clippy::print-stderr` errors at lines emitting `eprintln!`:
  - Failed to kill server process
  - Failed to wait for process

**`unwrap()` on `Mutex::lock()` (disallowed)**
- `clippy::unwrap-used` at all `GLOBAL_AI_SERVER.lock().unwrap()` calls.

**Recommended fix path (ai-server)**
1) Replace `println!`/`eprintln!` with `tracing::{info,warn}` or similar logging.
2) Replace `lock().unwrap()` with error handling (e.g., `lock().map_err(|e| anyhow!(...))?`) or `expect` with a specific message if truly unrecoverable.
3) Optional cleanups (warnings, not fatal):
   - Remove redundant `use reqwest;`
   - Change `args(&[...])` to `args([...])`
   - Refactor `find_ai_server_path` to a free function if `self` isn’t used
   - Replace manual `Option` mapping with `.as_ref().map(...)`
   - Replace `once_cell::Lazy` with `std::sync::LazyLock` if you want to satisfy `clippy::non_std_lazy_statics`

## Secondary warnings (non-blocking, but will remain after ai-server fixes)
### `crates/openagents-utils/src/filenames.rs`
- `clippy::collapsible_str_replace`: replace chained `replace` with `replace([':', '/'], "_")`.

### `crates/codex-client/src/client.rs`
- `clippy::derivable_impls`: derive `Default` for `AppServerConfig` instead of manual impl.

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

### `apps/autopilot-desktop/src-tauri/build.rs`
- One `semicolon_if_nothing_returned` warning (fix immediately, see top section).

## Recommended cleanup order
1) **Autopilot Desktop**: fix `build.rs` warning (single-line change).
2) **ai-server**: fix `print_*` and `unwrap_used` errors (blocking).
3) **openagents-utils**: quick replace chain fix.
4) **codex-client**: derive `Default`.
5) **lm-router/gpt-oss**: small style fixes.
6) **nostr/core**: large batch cleanups (optionally defer or apply `clippy::allow` if not urgent).

## Notes
- The clippy run is scoped to `autopilot-desktop` but still checks dependent crates, so dependency lint errors block the run.
- Full log is in `/tmp/clippy_autopilot_desktop.log` for exact line references.

## Work log (2026-01-27)
- Fixed clippy errors across `autopilot-desktop`, `ai-server`, `lm-router`, `gpt-oss`, `openagents-utils`, and `codex-client` (eprintln/println removal, `LazyLock` usage, `derivable_impls`, and related nits).
- Removed `#[allow(...)]` usage in `autopilot-desktop` (clippy deny) and replaced with `#[expect(...)]` where needed.
- Updated `tauri` build flow to use `tauri_build::try_build` with codegen context and added scoped lint expectations for generated context.
- Applied large batch of `nostr` clippy fixes (`collapsible_if`, `get_first`, `inherent_to_string`, `FromStr`, and range helpers) plus test updates.
- Verified with `cargo clippy -p autopilot-desktop --all-targets` (exit 0; warnings remain but no clippy errors).
