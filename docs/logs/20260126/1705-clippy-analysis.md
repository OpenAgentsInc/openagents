# Clippy Status (workspace)

Latest run (2026-01-27): `cargo clippy` (full workspace). The run completes with warnings only; no hard errors.

Previous run context: `cargo clippy -p autopilot-desktop --all-targets` (log captured at `/tmp/clippy_autopilot_desktop.log`). That log is still useful for historical line references.

## What changed since the last log
### Unblocked hard errors (now fixed)
- `crates/issues/src/cache.rs`: removed `RwLock` unwraps via safe lock helpers.
- `crates/issues/src/directive.rs`: replaced `eprintln!` with `tracing::warn`.
- `crates/ws-test/src/main.rs`: removed `expect` usages; added non-panicking fallback response and HMAC init guard.
- `crates/autopilot-desktop-runner/src/main.rs`: replaced `println!/eprintln!` with `tracing::warn`; added tracing init.
- `crates/autopilot-desktop-runner/Cargo.toml`: added `tracing` + `tracing-subscriber`.
- `crates/compute/src/domain/job.rs`: imported `FromStr` for `InputType`.
- `crates/runtime/src/dvm.rs`: imported `FromStr`; replaced `#[allow(dead_code)]` with `#[expect(dead_code)]`.
- `crates/runtime/src/containers/providers/dvm.rs` + `crates/runtime/src/compute/providers/dvm.rs`: replaced `#[allow(dead_code)]` with `#[expect(dead_code)]`.
- `crates/pylon/src/db/jobs.rs`: fixed `JobStatus::from_str` handling.
- `crates/autopilot-core`:
  - Removed `unwrap()` in hot paths (`dspy_optimization.rs`, `logger.rs`, `startup.rs`, `replay.rs`).
  - Replaced `#[allow(dead_code)]` with `#[expect(dead_code)]`.
  - Marked test modules with `#[expect(clippy::unwrap_used)]`.

Note: `Cargo.lock` updated because of the new `tracing-subscriber` dependency.

## Current status (warnings only)
Clippy now finishes without errors, but there is still a large warning surface. Key buckets from the latest run:
- `crates/rlm`: `manual_strip`, `double_ended_iterator_last`, `too_many_arguments`.
- `crates/frlm`: `collapsible_if`.
- `crates/spark`: `single_match_else`, `assigning_clones`, `cast_lossless`, `implicit_clone`, plus **unfulfilled** `#[expect(dead_code)]` on `SparkWallet::sdk`.
- `crates/compute`: `collapsible_if`, `should_implement_trait` (custom `from_str`), `derivable_impls`, `redundant_closure`, `manual_contains`, `unnecessary_lazy_evaluations`, `needless_borrows_for_generic_args`.
- `crates/autopilot-core`: `collapsible_if`, `map_unwrap_or`, `manual_range_contains`, `manual_string_new`, `unused_self`, plus **unfulfilled** `#[expect(dead_code)]` on `StartupPhase` + `LogLine.timestamp`.
- `crates/autopilot`: large volume of `collapsible_if`, `single_match`, `implicit_saturating_sub`, `single_char_add_str`, etc.
- `crates/wgpui`, `crates/vim`, `crates/gateway`, `crates/relay`: mostly style warnings (`collapsible_if`, `derivable_impls`, etc.).

## Cleanup plan (recommended order)
1) **Remove unfulfilled `#[expect(dead_code)]`**  
   - These were added to satisfy `clippy::allow_attributes`, but the lint no longer fires, so `expect` itself becomes noisy.  
   - Targets: `crates/autopilot-core/src/startup.rs`, `crates/autopilot-core/src/startup/types.rs`, `crates/spark/src/wallet.rs`.

2) **Low-risk auto-fix passes by crate**  
   - `cargo clippy --fix --lib -p rlm`  
   - `cargo clippy --fix --lib -p frlm`  
   - `cargo clippy --fix --lib -p compute`  
   - `cargo clippy --fix --lib -p autopilot-core`  
   These will address most `collapsible_if`, `map_unwrap_or`, `manual_range_contains`, etc.

3) **Trait/constructor normalization**  
   - Convert custom `from_str` helpers to `impl FromStr` or rename to `parse_*` in `compute`, `issues`, and related crates.

4) **Large warning surfaces (defer or batch)**  
   - `autopilot`, `wgpui`, `vim`, and `nostr` are mostly stylistic. Consider batching, or selectively allowing noisy lints if they do not impact correctness.

## Notes
- The workspace denies `allow_attributes`, so suppression should use `#[expect(...)]` and only where the lint actually fires.
- The latest `cargo clippy` run completed successfully; remaining work is cleanup/consistency, not blocking.
