# ADR-0006: Deterministic Hashing and Canonicalization Rules

## Status

**Proposed**

## Date

2026-01-13

## Context

OpenAgents relies on hashes for:
- Tool receipts (`params_hash`, `output_hash`)
- Replay events (linkability + audit)
- Job envelopes (`job_hash`)
- Artifact integrity (receipt ↔ replay ↔ verification)

We need a canonical hashing approach to ensure receipts and replay logs are verifiable across crates, languages, and time.

## Decision

**All OpenAgents hashes use SHA-256 over a deterministic canonical representation, and all hashes are computed over full (untruncated) data.**

### Canonical Authority

Canonical hashing rules are defined and implemented in:
- `crates/protocol/` (canonical JSON + job hashing)
- `docs/PROTOCOL_SURFACE.md` (protocol-level hashing rules)
- `crates/dsrs/docs/REPLAY.md` + `crates/dsrs/docs/ARTIFACTS.md` (artifact/replay hash requirements)

This ADR defines **cross-system invariants** and forbids divergent "local" hashing schemes.

### Rules (Normative)

1. **Algorithm**
   - SHA-256
   - Output format: `sha256:<lowercase_hex>`

2. **Canonicalization**
   - Deterministic JSON canonicalization (JCS-like):
     - Object keys sorted lexicographically
     - Arrays preserve order
     - No whitespace
   - Canonicalization function must be shared/reused where possible (prefer `crates/protocol` helpers).

3. **No truncation for hashing**
   - Hashes MUST be computed from the **full** value.
   - Truncation is permitted only for *display previews* and must never affect hashes.

4. **Avoid floats in hashed payloads**
   - Prefer integers or strings for values that must be hashed.
   - If floats are unavoidable, they must have a single canonical encoding rule (documented in `crates/protocol`).

## Scope

What this ADR covers:
- Hash algorithm + output format
- Canonicalization invariants
- "Full output" requirement
- Where hashing rules are defined as canonical

What this ADR does NOT cover:
- Receipt schema details (ADR-0002 + ARTIFACTS.md)
- Replay event schema details (ADR-0003 + REPLAY.md)
- Specific job schema contents (PROTOCOL_SURFACE.md)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Hash prefix | Stable: `sha256:` |
| Hex casing | Stable: lowercase |
| Tool params hash | Stable: computed on canonical JSON params |
| Tool output hash | Stable: computed on full output (never preview) |
| Job hash | Stable: computed on canonical JSON job payload |
| Canonicalization | Stable: key-sorted objects, ordered arrays |

Backward compatibility:
- Adding new optional fields is allowed (hash inputs change only if included in hashed structure by spec).
- Changing canonicalization or hash rules requires a superseding ADR + migration plan + test vectors.

## Consequences

**Positive:**
- Receipts and replay logs become verifiable across crates and languages
- Prevents "hash drift" bugs and audit failures

**Negative:**
- Requires discipline to reuse canonicalization helpers instead of ad-hoc serde_json formatting

**Neutral:**
- May require adding test vectors and cross-impl tests (Rust/TS) to enforce invariants

## Alternatives Considered

1. **Serde JSON string hashing** — rejected (non-deterministic key ordering).
2. **RFC 8785 strict JCS library everywhere** — viable, but would require adopting a single library and migrating existing code.
3. **Binary formats (protobuf)** — rejected (harder to audit and share as plain artifacts).

## References

- [docs/PROTOCOL_SURFACE.md](../PROTOCOL_SURFACE.md) — hashing rules
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — replay hashing invariants
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — receipt hashing invariants
- `crates/protocol/src/*` — canonical hashing helpers (source of truth)
