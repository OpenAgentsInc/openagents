# ADR-0011: Schema IDs are Canonical; Nostr Kind Numbers are Incidental

## Status

**Accepted**

## Date

2026-01-13

## Context

Docs and implementations have drifted by hardcoding Nostr kind numbers for job types. This is fragile:
- kind assignments can change or be interpreted differently,
- OpenAgents job types are versioned by schema ID (e.g., `oa.sandbox_run.v1`),
- kind numbers primarily describe envelope category (request/result/feedback), not job semantics.

We need a single rule so docs, providers, and clients don't rot.

## Decision

**Job type identity is the `schema_id`. Kind numbers MUST NOT be treated as the canonical identifier for job semantics.**

### Normative rules

1. Every NIP-90 job request/result MUST include `schema_id` in the payload/envelope.
2. Provider capability announcements MUST advertise supported job types by `schema_id`.
3. Docs MUST reference job types by `schema_id` (not by kind number).
4. Kind numbers may be mentioned only as:
   - envelope category (request vs result vs feedback), and/or
   - "current defaults per PROTOCOL_SURFACE.md".

### Canonical owner

- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) is the canonical mapping reference.
- `crates/protocol/` is the source of truth for envelope parsing and schema identifiers.

## Scope

What this ADR covers:
- Canonical identity for job types (`schema_id`)
- Documentation rules re: kind numbers
- Provider capability advertisement expectations

What this ADR does NOT cover:
- The specific kind mapping values (that lives in PROTOCOL_SURFACE/code)
- Job schema definitions (inputs/outputs)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Job type key | Stable: `schema_id` string |
| Versioning | Stable: `.vN` increments are meaningful |
| Docs | Must not hardcode kinds for job semantics |

Backward compatibility:
- If legacy payloads omit schema_id, clients/providers may shim temporarily, but new code must include schema_id.

## Consequences

**Positive:**
- Eliminates a major source of documentation rot
- Enables stable job typing across transport changes
- Makes multi-language implementations safer

**Negative:**
- Requires updating older docs/tests that key off kind numbers

**Neutral:**
- Kind numbers still exist and must be supported at transport level

## Alternatives Considered

1. **Kinds are canonical** — rejected (brittle, not versioned, ambiguous).
2. **Custom OpenAgents kinds per schema** — rejected (explodes kind space and increases drift).
3. **No schema IDs** — rejected (no versioned job identity).

## References

- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md)
- [GLOSSARY.md](../GLOSSARY.md) — schema ID terminology
- `crates/protocol/src/*` — schema registry / envelope parsing
