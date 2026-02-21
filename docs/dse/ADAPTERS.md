# Adapters (Serialization/Parsing Contract)

Adapters are responsible for **formatting** provider requests and **parsing** provider responses.

Normative contract (see `docs/plans/archived/adr-legacy-2026-02-21/ADR-0007-tool-execution-contract.md`):
- Adapters MUST serialize/parse only.
- Adapters MUST NOT validate tool params against schemas.
- Adapters MUST NOT execute tools.
- Adapters MUST NOT implement retry policies (retries belong to predictors/runtime).

Why:
- Centralizes enforcement (schemas, retries, receipts) in the runtime.
- Keeps provider-specific formatting logic separate from safety and accounting.

Where this matters in the repo:
- Provider adapters live in DSE/compiler surfaces (see `packages/dse/`).
- Tool execution and receipts live in runtime/control-plane surfaces (`apps/runtime/`, `apps/openagents.com/`, `packages/effuse/`, and desktop Rust surfaces under `crates/`).
