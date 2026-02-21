# openagents-proto

Generated Rust wire contracts for OpenAgents proto packages.

## Purpose

This crate owns proto-generated wire types for all boundary-crossing contracts under `proto/openagents/*/v1/*`.

Boundary rule:
1. Use `openagents_proto::wire::*` for cross-process/client-server payloads.
2. Use domain structs in service/app crates for business logic.
3. Convert explicitly at the boundary (`TryFrom` / `From`).

## Regeneration Workflow

Generation is build-driven via `build.rs`, and verified alongside Buf Rust generation (`buf.gen.yaml` -> `target/buf/rust`).

Canonical verification command:

```bash
scripts/verify-rust-proto-crate.sh
```

This script verifies:
1. `buf generate` produces non-empty Rust output deterministically.
2. `build.rs` proto output is deterministic across repeated builds.
3. crate contract tests pass.

## Usage

```rust
use openagents_proto::wire::openagents::sync::v1::KhalaFrame;

let frame = KhalaFrame {
    topic: "runtime.codex_worker_events".into(),
    seq: 1,
    kind: 2,
    payload_bytes: vec![],
    schema_version: 1,
};
```
