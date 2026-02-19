# ADR-0021: Rust-to-TypeScript Contract Generation for Tauri IPC

## Status

Deprecated

## Date

2026-01-25

## Deprecated Date

2026-02-19

## Replacement

- `apps/autopilot-desktop/` current Rust-native desktop architecture
- `crates/autopilot_app/src/lib.rs` and `crates/autopilot_ui/src/lib.rs` as current typed desktop event/action surfaces
- `docs/codex/unified-runtime-desktop-plan.md` for current desktop/runtime integration direction

## Context

This ADR captured a former Tauri-based desktop architecture
(`apps/autopilot-desktop/src-tauri/`) that is no longer present in this repo.

We need a lightweight, Tauri-friendly approach that provides:
- Rust as the source of truth for DTOs used in `#[tauri::command]` inputs/outputs
- Generated TypeScript types for frontend compile-time checks
- Compatibility with Effect (optional runtime validation)

## Decision

Adopt a **Rust-first DTO contract** using `ts-rs` for TypeScript type generation.

### Contract Layout

1. **Rust DTOs live in a dedicated module**:
   - `apps/autopilot-desktop/src-tauri/src/contracts/ipc.rs`
   - Only request/response types for Tauri commands (no business logic)
   - `serde::{Serialize, Deserialize}` + `ts_rs::TS` derives
   - `JsonValue` alias for opaque payloads (`unknown` in TS)

2. **TypeScript types are generated into the frontend**:
   - Output path: `apps/autopilot-desktop/src/gen/tauri-contracts.ts`
   - Generated file is committed and updated in CI/dev scripts

3. **Frontend IPC wrapper becomes typed**:
   - `apps/autopilot-desktop/src/ipc/unified.ts` (pattern for IPC modules)
   - `apps/autopilot-desktop/src/ipc/dsrs.ts` (signature registry IPC)
   - Commands take typed args and return typed results from `apps/autopilot-desktop/src/gen/tauri-contracts.ts`
   - IPC responses are decoded through Effect Schema

4. **Effect validation is implemented for core IPC**:
   - Schemas live in `apps/autopilot-desktop/src/contracts/tauri.ts`
   - `UnifiedEvent` payloads are decoded before entering the queue
   - Typed responses are validated in `src/ipc/unified.ts`

### Generation Mechanism

- A small Rust binary at `apps/autopilot-desktop/src-tauri/src/bin/gen_types.rs` writes a single
  `apps/autopilot-desktop/src/gen/tauri-contracts.ts` file by calling `export_ts` in
  `apps/autopilot-desktop/src-tauri/src/contracts/ipc.rs`.
- Add a script `bun run types:gen` that runs the generator via Cargo.
- Run `types:gen` in CI before TypeScript typecheck.

### Implementation Notes

- `JsonValue` is emitted as `unknown` in TypeScript to preserve opaque payloads
  returned from Codex app-server (`StartThreadResponse`, `ListModelsResponse`, etc.).
- `UnifiedEvent` numeric fields are mapped to `number` in TS (not `bigint`).
- Generated types are used directly in frontend IPC wrappers and schemas.

## Consequences

**Positive:**
- Single source of truth for IPC DTOs (Rust)
- Strong compile-time guarantees in the Effect frontend
- Minimal tooling overhead vs OpenAPI/Protobuf

**Negative:**
- Requires a generation step and discipline to keep output current
- DTOs need to be extracted from existing `serde_json::Value` returns

**Neutral:**
- Schemas are adopted for core paths now; additional IPC can follow the same pattern

## Alternatives Considered

1. **OpenAPI generation** (Rust → OpenAPI → TS client)
   - Rejected: heavier tooling and best suited for public HTTP APIs, not
     Tauri IPC.

2. **Protobuf/gRPC**
   - Rejected: high ceremony and a mismatch for Tauri `invoke` payloads.

3. **Manual TS types**
   - Rejected: error-prone and drifts from Rust over time.

## References

- `docs/adr/ADR-0001-adoption-of-adrs.md`
- `apps/autopilot-desktop/src-tauri/src/contracts/ipc.rs`
- `apps/autopilot-desktop/src-tauri/src/bin/gen_types.rs`
- `apps/autopilot-desktop/src/gen/tauri-contracts.ts`
- `apps/autopilot-desktop/src/contracts/tauri.ts`
- `apps/autopilot-desktop/src/ipc/unified.ts`
