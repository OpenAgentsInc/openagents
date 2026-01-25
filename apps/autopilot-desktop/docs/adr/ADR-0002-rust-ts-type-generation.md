# ADR-0002: Rust-to-TypeScript Contract Generation for Tauri IPC

## Status

**Accepted**

## Date

2026-01-25

## Context

Autopilot is a Tauri application with a Rust backend (`src-tauri/`) and a TypeScript
frontend that uses Effect. Frontend IPC calls previously routed through a generic
`invokeEffect(command, payload)` wrapper, while backend commands in
`src-tauri/src/codex.rs` and `src-tauri/src/agent/commands.rs` returned
`serde_json::Value`. The IPC boundary had no compile-time contract between Rust
and TypeScript, and drift was easy.

We need a lightweight, Tauri-friendly approach that provides:
- Rust as the source of truth for DTOs used in `#[tauri::command]` inputs/outputs
- Generated TypeScript types for frontend compile-time checks
- Compatibility with Effect (optional runtime validation)

## Decision

Adopt a **Rust-first DTO contract** using `ts-rs` for TypeScript type generation.

### Contract Layout

1. **Rust DTOs live in a dedicated module**:
   - `src-tauri/src/contracts/ipc.rs`
   - Only request/response types for Tauri commands (no business logic)
   - `serde::{Serialize, Deserialize}` + `ts_rs::TS` derives
   - `JsonValue` alias for opaque payloads (`unknown` in TS)

2. **TypeScript types are generated into the frontend**:
   - Output path: `src/gen/tauri-contracts.ts`
   - Generated file is committed and updated in CI/dev scripts

3. **Frontend IPC wrapper becomes typed**:
   - `src/components/unified-stream/api.ts` (pattern for future IPC modules)
   - Commands take typed args and return typed results from `src/gen/tauri-contracts.ts`
   - IPC responses are decoded through Effect Schema

4. **Effect validation is implemented for core IPC**:
   - Schemas live in `src/contracts/tauri.ts`
   - `UnifiedEvent` payloads are decoded before entering the queue
   - Typed responses are validated in `src/components/unified-stream/api.ts`

### Generation Mechanism

- A small Rust binary at `src-tauri/src/bin/gen_types.rs` writes a single
  `src/gen/tauri-contracts.ts` file by calling `export_ts` in
  `src-tauri/src/contracts/ipc.rs`.
- Add a script `bun run types:gen` that runs the generator via Cargo.
- Run `types:gen` in CI before TypeScript typecheck.

### Implementation Notes

- `JsonValue` is emitted as `unknown` in TypeScript to preserve opaque payloads
  returned from Codex app-server (`StartThreadResponse`, `ListModelsResponse`, etc.).
- `UnifiedEvent` numeric fields are mapped to `number` in TS (not `bigint`).
- `src/components/unified-stream/types.ts` re-exports `UnifiedEvent` from
  the generated contracts.

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
- `src-tauri/src/contracts/ipc.rs`
- `src-tauri/src/bin/gen_types.rs`
- `src/gen/tauri-contracts.ts`
- `src/contracts/tauri.ts`
- `src/components/unified-stream/api.ts`
