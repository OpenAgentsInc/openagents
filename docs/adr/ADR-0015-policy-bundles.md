# ADR-0015: Policy Bundles — Format, Pin/Rollback, and Rollout States

## Status

**Proposed**

## Date

2026-01-13

## Context

OpenAgents relies on DSPy-style compilation (dsrs) to improve policies over time. Without a rigorous "policy bundle" contract:
- behavior changes are not attributable ("what changed?"),
- rollouts are unsafe (no canary/shadow discipline),
- regressions can't be rolled back cleanly,
- training data cannot be tied to the exact policy used.

We need a single policy artifact that:
- is versioned and immutable once created,
- can be pinned/rolled back,
- supports staged rollout states,
- is recorded in session artifacts (`policy_bundle_id`).

## Decision

**Policy bundles are versioned, immutable artifacts. Every autonomous session MUST record `policy_bundle_id`. Policy bundles MUST support pin/rollback and staged rollout states.**

### Canonical terms (normative)

- Canonical identifier: **`policy_bundle_id`** (never `policy_version`).
- `policy_version` may exist only as display metadata derived from bundle manifest.

### Canonical owner (where the details live)

- Bundle format + manifests: `crates/dsrs/docs/OPTIMIZERS.md` (policy bundles) and `crates/dsrs/docs/COMPILER-CONTRACT.md` (compiled manifests).
- Session linkage: `crates/dsrs/docs/ARTIFACTS.md` (RECEIPT.json fields) and `crates/dsrs/docs/REPLAY.md` (replay header/session start fields).
- Terminology: [GLOSSARY.md](../../GLOSSARY.md)

This ADR defines *what is canonical* and *stability/rollout requirements*, not full schema duplication.

### Bundle contents (normative, minimum)

A policy bundle MUST contain:
- a bundle manifest (metadata + references to compiled modules)
- per-signature compiled artifacts (instruction text + optional demos)
- optimizer metadata (what produced it, datasets/hashes, scorecards)
- compatibility requirements (tools/lanes/privacy modes if applicable)

Bundles SHOULD be content-addressed where practical (hash-based IDs), but the only required stable handle is `policy_bundle_id`.

### Rollout states (normative)

Policy bundles progress through rollout states:

- **Candidate** — compiled, not yet trusted
- **Staged** — passed cheap/proxy checks (format/schema/contract)
- **Shadow** — evaluated in parallel; does not control behavior
- **Promoted** — default for new sessions
- **RolledBack** — removed from default due to regression (kept for audit)

Promotion gates are defined by the evaluation system (proxy + truth metrics + shadow comparisons). The *state* must be recorded in the bundle manifest as `rollout_state`.

### rollout_state field (normative)

- `rollout_state` is **required** in all new bundles created after this ADR.
- If `rollout_state` is missing in older bundles, treat as `Candidate` (conservative default).
- Valid values: `Candidate`, `Staged`, `Shadow`, `Promoted`, `RolledBack`.

### Pin / rollback (normative)

- The runtime MUST allow selecting an explicit `policy_bundle_id` for a session ("pin").
- There MUST be a mechanism to revert the default bundle to the last known-good promoted bundle ("rollback").
- Pinning must not mutate the pinned bundle; it only changes selection.

### Session attribution (normative)

Every session artifact set MUST record `policy_bundle_id`:
- `RECEIPT.json.policy_bundle_id`
- `REPLAY.jsonl` header/session start includes `policy_bundle_id`
- any external publication (Forge adapters) must preserve this attribution

## Scope

What this ADR covers:
- What a policy bundle is (artifact + immutability)
- Required rollout states and selection semantics
- Pin/rollback requirements
- Session attribution requirements

What this ADR does NOT cover:
- Exact on-disk directory layout (covered by ADR-0008 "Session storage layout + artifact paths")
- Optimizer algorithms and scoring formulas (covered by dsrs docs)
- UI/UX for policy management commands

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Identifier | `policy_bundle_id` is the canonical bundle handle |
| Immutability | Bundles are append-only; existing bundles are not rewritten |
| Attribution | Sessions record `policy_bundle_id` in receipt + replay |
| Rollout states | Candidate → Staged → Shadow → Promoted (+ RolledBack) |
| `rollout_state` field | Required in new bundles; missing = `Candidate` |

Backward compatibility expectations:
- Adding optional metadata fields to bundle manifests is allowed.
- Removing/renaming core fields or changing state semantics requires a superseding ADR + migration.

## Consequences

**Positive:**
- Every behavior change is attributable and reversible
- Enables safe self-improvement rollout (shadow/canary discipline)
- Enables consistent fleet metrics and regression tracking by bundle ID

**Negative:**
- Requires storage/retention of old bundles and their eval history
- Slightly more operational complexity (promotion/rollback workflow)

**Neutral:**
- Bundle IDs can be hash-based or semver-like; `policy_bundle_id` is the stable handle either way

## Alternatives Considered

1. **No bundles; edit prompts in place** — rejected (not reproducible, not auditable).
2. **One global "current prompt set"** — rejected (no rollback or attribution).
3. **Fine-tuning weights instead of bundles** — rejected for MVP (heavier operational burden; doesn't remove need for attribution).

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `policy_bundle_id`, terminology
- [crates/dsrs/docs/OPTIMIZERS.md](../../crates/dsrs/docs/OPTIMIZERS.md) — policy bundle format (canonical)
- [crates/dsrs/docs/COMPILER-CONTRACT.md](../../crates/dsrs/docs/COMPILER-CONTRACT.md) — compiled manifests
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — RECEIPT.json attribution
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — replay attribution
- [ROADMAP.md](../../ROADMAP.md) — pin/rollback + rollout gating as MVP items
