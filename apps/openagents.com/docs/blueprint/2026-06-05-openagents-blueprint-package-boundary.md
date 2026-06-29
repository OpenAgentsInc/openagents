# OpenAgents product surface Blueprint Package Boundary

Issue #222 adds the first OpenAgents product surface-owned Blueprint kernel source boundary.

The source boundary is `workers/api/src/blueprint`.

## Source Layout

- `boundary.ts`: boundary manifest, ownership policy, module catalog, and
  authority modes.
- `index.ts`: public exports for the current Blueprint kernel boundary.
- `README.md`: source-level rules for future Blueprint kernel modules.

Future modules should live under:

- `schemas/`
- `repositories/`
- `services/`
- `projections/`
- `exports/`
- `fixtures/`

## Authority Modes

The initial authority modes are:

- `evidence_only`
- `approval_gated`
- `export_only`

The boundary intentionally does not grant runtime authority. Program Runs and
most kernel records are evidence-only. Write-side services must be
approval-gated. Contract exports are export-only.

## Study Packet Marketplace Boundary

StudyBench study packets can cite Blueprint program signatures, source
authority refs, context packs, release gates, and Action Submission evidence,
but they remain evidence-only until a separate package lane passes product and
marketplace gates.

For `autopilot.repo_study_packets.v1`, the allowed public claim is internal
OpenAgents dogfood over the public `openagents` repo. A study packet is not
customer repo studying, not a trained repo expert, not a marketplace package,
not payout eligibility, and not paid work. Marketplace package work requires
separate customer-data privacy, held-out evaluation, usage metering, pricing,
payout, and settlement receipts.

## Ownership

The boundary manifest declares:

- `kernelRef = openagents.blueprint.kernel.v1`
- `ownerRef = openagents`
- `deprecatedDependencyAllowed = false`

This keeps the deprecated Blueprint workspace as reference material only.

## Next Steps

The following Epic Q issues should add concrete modules inside this boundary:

- Objective and Outcome schemas.
- Program Type and Program Signature schemas.
- Module Version schema.
- Program Run repository.
- Program Run evidence-only guards.
- Action Submission approval-gated write path.
- Source Authority and Context Pack.
- Release Gate and fixtures.
- Contract exports.
