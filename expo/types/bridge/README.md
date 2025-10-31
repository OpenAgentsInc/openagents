# Bridge Type Definitions (Generated)

Source of truth
- Rust structs in `crates/oa-bridge/src/types.rs` and `crates/oa-bridge/src/ws.rs` (e.g., `ThreadSummaryTs`).
- These types derive `ts_rs::TS` and are configured to export into this folder when building the bridge crate.

What’s here
- TypeScript type files named after their Rust counterparts (e.g., `ThreadSummaryTs.ts`).
- The app imports these directly. Do not hand‑edit — regenerate from Rust when shapes change.

Generating
- Run bridge tests (exports happen on `cargo test` with `#[ts(export)]`):
  - `cargo clean -p oa-bridge && cargo test -p oa-bridge`
- The `ts-rs` dependency includes the `format` feature to pretty‑print output.
- The bridge build script ensures this directory exists before tests run.

Conventions
- All fields are snake_case to match wire JSON.
- Prefer `last_message_ts` for timestamps in thread list rows (fallback to `updated_at`).
- Avoid mixed‑case probing in the app; import these types and use fields as‑is.
