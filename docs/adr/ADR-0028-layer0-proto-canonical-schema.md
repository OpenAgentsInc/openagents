# ADR-0028: Layer-0 Proto Contracts Are Canonical in `proto/`

## Status

Accepted

## Date

2026-02-19

## Context

OpenAgents has multiple active language/runtime surfaces (Elixir runtime, Laravel control plane, TypeScript clients, Rust desktop/tooling). Without a neutral contract source, schema ownership drifts between language-specific DTOs and docs during refactors.

The current strategy direction (including notes in `docs/local/convo.md`) calls for a language-neutral, codegen-first Layer-0 schema so protocol contracts survive future implementation shifts.

The repository already contains:

- canonical proto packages under `proto/openagents/protocol/v1/*`,
- Buf config (`buf.yaml`, `buf.gen.yaml`),
- generation verification (`scripts/verify-proto-generate.sh`),
- Layer-0 mapping guidance (`docs/protocol/LAYER0_PROTOBUF_MAPPING.md`).

What was missing was an explicit ADR making this authoritative across plans and implementation work.

## Decision

`proto/` is the canonical Layer-0 universal schema source for shared contracts across runtime, control plane, and clients.

Normative rules:

1. Shared cross-surface contracts MUST be defined first in `proto/openagents/protocol/v1/*`.
2. Language-specific DTOs (Elixir/PHP/TS/Rust) MUST be generated from or explicitly mapped to proto definitions; they are not authoritative on their own.
3. Wire transport may remain JSON/SSE, but payload shapes MUST remain proto-compatible per `docs/protocol/LAYER0_PROTOBUF_MAPPING.md`.
4. Contract evolution in `v1` is additive-only; breaking changes require versioned package bump (for example `v2`).
5. Buf lint and breaking checks are required for proto contract changes.
6. Generated artifacts under `generated/` are not committed; CI verifies generation viability.

### Schema / Spec Authority

- `proto/README.md` — canonical Layer-0 package layout and generation policy.
- `docs/protocol/LAYER0_PROTOBUF_MAPPING.md` — proto to JSON/SSE mapping rules.
- `docs/protocol/PROTOCOL_SURFACE.md` — field semantics and protocol-level behavior.

## Scope

What this ADR covers:

- Layer-0 schema authority for shared contracts.
- Evolution and compatibility policy for proto-defined contracts.
- Relationship between proto contracts and JSON/SSE transport.

What this ADR does NOT cover:

- Replacing JSON/SSE transport with binary protobuf transport.
- UI-only/internal-only view models that never cross shared boundaries.
- Business-policy semantics beyond schema and compatibility ownership.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Canonical schema root | Stable: `proto/` |
| Canonical package root | Stable: `proto/openagents/protocol/v1/*` |
| Contract evolution | Stable: additive-only within `v1` |
| Breaking guard | Stable: Buf breaking checks required |
| JSON/SSE compatibility | Stable: proto-compatible mapping required |

Backward compatibility expectations:

- New fields and enum values are additive.
- Field numbers are never reused.
- Removed fields/names are reserved.
- Existing consumers remain valid until explicit package version bump.

## Consequences

**Positive:**

- Prevents source-of-truth drift across Elixir/PHP/TS/Rust.
- Makes language switches/refactors lower-risk.
- Strengthens replay/audit consistency via shared schema governance.

**Negative:**

- Adds schema-change ceremony (Buf checks + mapping updates).
- Requires discipline to keep adapters/mappers aligned with proto.

**Neutral:**

- Existing JSON/SSE contracts continue; only authority and governance are clarified.

## Alternatives Considered

1. **Language-owned DTOs as canonical** — rejected (drift-prone across stacks).
2. **OpenAPI-only as canonical** — rejected (HTTP-focused; weaker fit for event envelopes and generated multi-runtime DTOs).
3. **JSON Schema-only as canonical** — rejected (good for config/manifests, weaker for typed multi-language event contracts).
4. **Binary protobuf on all wires now** — rejected for now (unnecessary transport change; JSON/SSE mapping is sufficient).

## References

- `proto/README.md`
- `buf.yaml`
- `buf.gen.yaml`
- `scripts/verify-proto-generate.sh`
- `docs/protocol/LAYER0_PROTOBUF_MAPPING.md`
- `docs/local/convo.md`
