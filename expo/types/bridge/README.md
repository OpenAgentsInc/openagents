# Bridge Type Definitions (Generated)

Source of truth
- Rust structs in `crates/oa-bridge/src/types.rs` and `crates/oa-bridge/src/ws.rs` (e.g., `ThreadSummaryTs`).
- These types derive `ts_rs::TS` and are configured to export into this folder when building the bridge crate.

What’s here
- TypeScript type files named after their Rust counterparts (e.g., `ThreadSummaryTs.ts`).
- The app imports these directly. Do not hand‑edit — regenerate from Rust when shapes change.

Generating
- Build the bridge crate (from repo root):
  - `cargo clean -p oa-bridge && cargo build`
- The `ts-rs` dependency includes the `format` feature to pretty‑print output.
- If files are not created, ensure the folder exists and rebuild; we keep a build script in the bridge to create this directory.

Conventions
- All fields are snake_case to match wire JSON.
- Prefer `last_message_ts` for timestamps in thread list rows (fallback to `updated_at`).
- Avoid mixed‑case probing in the app; import these types and use fields as‑is.

