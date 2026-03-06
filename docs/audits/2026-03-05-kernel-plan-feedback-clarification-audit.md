# Economy Kernel Plan Feedback Clarification Audit

Date: 2026-03-05
Author: Codex
Status: Complete

## Objective

Evaluate the provided feedback against current plan docs in `docs/plans/` and current desktop implementation, then identify what actually needs clarification versus what is already specified.

## Sources Reviewed

Plan/spec docs:
- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`
- `docs/plans/diagram.md`
- `docs/plans/hydra-liquidity-engine.md`
- `docs/plans/aegis.md`

Implementation context:
- `apps/autopilot-desktop/src/economy_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/job_inbox.rs`
- `docs/MVP.md`

## Executive Summary

The feedback is directionally strong. Most items are not architectural holes, but documentation precision gaps.

Triage result:
- Already covered well enough: 4 items
- Partially covered and should be clarified: 9 items
- Truly missing normative semantics worth adding now: 5 items

The highest-value clarifications are:
1. verifier assignment and verifier economics/liability posture,
2. explicit TreasuryRouter non-authoritative boundary,
3. evidence immutability/content-addressability contract,
4. deterministic snapshot ordering rule,
5. explicit handling of long-latency external ground-truth evidence.

## Feedback Triage Matrix

| Feedback item | Verdict | Evidence | Recommended action |
|---|---|---|---|
| Verifier incentive mechanism | Missing | `economy-kernel.md` defines tiers/capacity but no assignment/payment model; proto has `verification_fee` field only | Add `VerificationAssignment` semantics and receipts for assignment + verifier payout |
| Verifier slashing/liability | Missing | Bonds cover worker/underwriter/dispute in `economy-kernel.md`; no verifier liability statement | Add explicit policy choice: `verifier liability lane optional` or `verifiers non-liable` |
| Oracle/ground-truth resolution for long-latency | Partial | Outcome resolution invariant exists (`objective/adjudication/human`) but no explicit external oracle evidence shape | Add `GroundTruthEvidence` requirements (source, digest, confidence, received_at_ms) |
| Worker matching model | Partial (intentional scope gap) | Kernel spec defines contract/settlement, not discovery; MVP defines NIP-90 demand paths | Add one explicit line: kernel is post-match execution and does not define discovery/matching protocol |
| TreasuryRouter authority boundary | Partial | Kernel says TreasuryRouter "decides what should happen" and kernel executes; not explicit enough that TR is non-authoritative | Add explicit non-authoritative planner statement in Section 3.2 |
| Evidence store immutability | Partial | Receipts immutable and evidence digests required; evidence object immutability not explicit | Add rule: evidence referenced by receipts MUST be immutable content-addressed artifacts |
| Outcome registry privacy | Partial | `/stats` and exports have redaction rules; outcome registry public/restricted view split not explicit | Add redaction profile semantics for `OutcomeRegistryEntry` |
| Simulation scenario generation governance | Partial | GroundTruthCase linkage exists; scenario generation actor/validation path unspecified | Add required generator role + validation receipt linkage |
| Reputation decay rules | Partial and likely intentionally non-normative | Reputation constraints exist; no decay/weighting contract | Add explicit statement: formula is policy-defined, but decision receipts must include measurement window IDs |
| Cross-contract worker correlation | Partial | Checker correlation explicit; proto provenance includes producer lineage/correlation groups | Clarify in kernel spec that correlation controls apply to producer and checker lineages |
| Snapshot determinism ordering | Missing in spec, present in code | Spec has minute boundary only; implementation sorts by `created_at_ms` then `receipt_id` | Promote implementation ordering rule to normative spec |
| Evidence size growth controls | Partial | EvidenceRef model exists but no size/embedding guidance | Add guidance: large evidence SHOULD be external objects by digest; receipts store refs only |
| Policy complexity bounds | Missing operational guardrail | Deterministic precedence exists; no max complexity or eval SLO | Add non-functional limits: max rules, evaluation timeout, deterministic fallback |
| Identity trust authority and revocation model | Partial | CredentialRef exists; issuer trust/revocation semantics unspecified | Add out-of-scope note + required trust profile/issuer registry reference in policy |
| "Who decides truth" conceptual question | Covered but can be made more explicit | Outcome resolution invariant + adjudicator role already defined | Add one short restatement in dispute section to remove interpretation ambiguity |

## What Is Already Strong (No Structural Rework Needed)

1. Authority boundaries are already robust.
- HTTP-only authority mutations, required caller identity, policy bundle binding, and idempotency are explicit.

2. Truth-resolution path is already normative.
- `economy-kernel.md` already requires objective harness, declared adjudication policy, or human underwriting before outcomes become settled truth.

3. Incident/GroundTruth append-only lifecycle is already explicit.
- Plan and proto plan both require append-only incident revisions and immutable taxonomy code meanings.

4. Public/restricted safety and export posture already exists.
- `/stats` redaction rules and export redaction tiers are already described; this needs refinement for specific objects, not a redesign.

## Codebase Reality Check: Clarifications Needed To Avoid Spec/Impl Drift

The implementation already chooses behaviors that the spec should codify:

1. Snapshot ordering is deterministic in code.
- `economy_snapshot.rs` sorts receipts by `created_at_ms` then `receipt_id` before aggregation.
- The spec currently requires deterministic minute boundaries but does not define tie-break ordering.

2. Idempotency scope is concrete in code.
- `earn_kernel_receipts.rs` scopes idempotency by `(receipt_type, policy.approved_by)`.
- The spec currently defines endpoint + caller scope. This is close but not textually aligned with implementation naming.

3. Incident export redaction is implemented with explicit public/restricted behavior.
- Public tier redacts summaries, linkage IDs, and evidence URIs.
- Plan docs should mirror this object-level redaction expectation for outcome registry and scenario exports too.

4. Market matching is product-lane behavior today.
- `job_inbox.rs` and MVP docs show NIP-90 inbox intake and explicit accept/reject workflow.
- This reinforces that kernel should state it is post-match execution unless optional market modules are enabled.

## Recommended Clarification Patch Set (Priority Order)

### P0 Clarifications (add now)

1. Verifier assignment + payment semantics
- Add a normative subsection under verification module:
  - assignment modes,
  - assignment receipts,
  - verifier payment trigger and linkage.

2. TreasuryRouter non-authoritative boundary
- In Section 3.2, add explicit line:
  - TreasuryRouter proposes/plans; only kernel authority endpoints mutate state.

3. Evidence immutability contract
- In Section 5, add:
  - evidence refs in receipts MUST resolve to immutable content-addressed artifacts.

4. Snapshot ordering rule
- In Section 7.1, add:
  - receipt ordering for snapshot derivation MUST be `created_at_ms` then `receipt_id` (or exact equivalent deterministic total order).

5. Long-latency ground-truth evidence
- In Section 6.10, add required fields for externally arrived ground truth evidence and how it links into claim/incident resolution receipts.

### P1 Clarifications (next pass)

1. Verifier liability posture
- Either specify optional verifier bond/slash lane or explicitly declare verifier non-liability.

2. Outcome registry privacy profile
- Define required public vs restricted fields for outcome registry entries.

3. Reputation semantics boundaries
- Declare formula non-normative, require declared windows/weights in policy notes for decisions that depend on reputation.

4. Identity trust-profile boundary
- Add "trusted issuer and revocation verification profile" requirement to policy, or explicitly mark external trust verification as out-of-kernel.

5. Policy complexity guardrails
- Add implementation limits and deterministic failure behavior when policy exceeds limits.

### P2 Clarifications (optional but useful)

1. Explicit "kernel does not define worker discovery" sentence in market section.
2. Evidence payload sizing guidance for large artifacts.
3. Correlation language broadened from checker-only to producer+checker lineages.

## Suggested Minimal Text Additions

If you want a minimal edit set with high leverage, add these five statements verbatim into `economy-kernel.md`:

1. "TreasuryRouter is non-authoritative planning logic; it MUST NOT directly mutate economic state."
2. "Evidence referenced by receipts MUST be immutable, content-addressed artifacts; URIs are transport pointers, not authority."
3. "Snapshot derivation MUST apply a deterministic total order over receipts; default order is `created_at_ms` then `receipt_id`."
4. "Verification MUST include deterministic assignment semantics, including assignment receipts and payout linkage for verifier work."
5. "For long-latency outcomes, externally arrived ground-truth evidence MUST be linked as hash-bound evidence with source, confidence, and receipt lineage."

## Final Assessment

The architecture is already strong and largely complete. The remaining risk is not missing components; it is implementer divergence where semantics are implied but not pinned down.

This is a clarification pass, not a redesign pass.
