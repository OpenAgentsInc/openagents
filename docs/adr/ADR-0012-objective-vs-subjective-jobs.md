# ADR-0012: Objective vs Subjective Jobs and Settlement Rules

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents runs jobs over local/cloud/swarm providers. Some jobs are objectively verifiable (e.g., sandbox runs),
others are inherently subjective (e.g., summarization, reranking). Payment and trust must differ, or the system
will pay for garbage.

We need canonical rules for:
- verification modes,
- default settlement pattern,
- how receipts reflect verification.

## Decision

**Every job schema declares `verification_mode: objective | subjective`. Objective jobs default to pay-after-verify. Subjective jobs default to redundancy/adjudication before payment (or explicit reputation-only mode when allowed).**

### Normative rules

1. Job schemas MUST declare verification_mode.
2. **Objective jobs**
   - MUST include verifiable artifacts: exit codes and/or hashes sufficient to verify correctness.
   - Default settlement: **pay-after-verify**.
   - Receipt MUST record verification evidence and job_hash.

3. **Subjective jobs**
   - Must declare verification tier used:
     - `reputation_only` (explicitly allowed only under policy)
     - `redundancy_consensus` (best-of-N / majority)
     - `judge` (LLM judge)
     - `human_sampled` (optional)
   - Receipt MUST record tier, redundancy count (if applicable), and adjudication metadata.

### Canonical owner

- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) defines job schema surface + verification modes.
- `crates/protocol/` defines job typing and verification helpers.

## Scope

What this ADR covers:
- Verification taxonomy
- Default settlement patterns
- Minimum receipt metadata expectations for verification

What this ADR does NOT cover:
- Full payment rail implementations
- Provider reputation scoring details
- Dispute resolution protocol

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| verification_mode | Stable: `objective` or `subjective` |
| Objective settlement default | Stable: pay-after-verify |
| Receipt includes verification metadata | Stable requirement |

Backward compatibility:
- New verification tiers can be added (subjective), but existing tiers remain valid.
- Changing objective settlement default requires superseding ADR.

## Consequences

**Positive:**
- Prevents paying for unverifiable claims on objective work
- Makes trust model explicit and auditable

**Negative:**
- Subjective verification increases cost/latency when redundancy/judging is used

**Neutral:**
- Policy can still choose reputation-only for low-stakes work, but must be explicit

## Alternatives Considered

1. **Pay immediately for all jobs** — rejected (too easy to exploit).
2. **Always require human verification** — rejected (doesn't scale).
3. **Treat all jobs as subjective** — rejected (wastes the objective signal we have for code execution).

## References

- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md)
- [GLOSSARY.md](../../GLOSSARY.md) — objective vs subjective job terminology
- `crates/protocol/src/verification.rs` (or equivalent)
