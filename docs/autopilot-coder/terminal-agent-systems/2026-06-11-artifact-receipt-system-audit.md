# Artifact And Receipt System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #44 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent work should produce durable artifacts and receipts that can
prove what happened without exposing raw private material.

## Target

Build a unified artifact and receipt system for patches, diffs, test results,
build logs, previews, screenshots, PR drafts, closeouts, payment evidence,
assignment events, and settlement-related projections.

Artifacts are evidence. Receipts are claims about lifecycle transitions.
Neither should be free-form model prose.

## User-Visible Capability

Users should be able to:

- Inspect work artifacts grouped by run, mission, lane, and visibility.
- See which artifacts are private, team-visible, or public-safe.
- Understand which receipt proves admission, execution, verification,
  delivery, acceptance, rejection, payment, or settlement.
- Download or copy approved artifacts.
- Link public-safe receipts into issues, Forum posts, or workroom summaries.
- See when a claim is missing a required receipt.

## Artifact Model

Each artifact should include:

- Artifact ref.
- Kind.
- Run, mission, work order, or assignment ref.
- Digest.
- Size and media type.
- Visibility.
- Redaction class.
- Summary.
- Retention policy.
- Creation timestamp.
- Producer adapter ref.
- Related receipt refs.

Large payloads should be stored behind refs. The model and public projections
should receive summaries and digests, not raw unbounded data.

## Receipt Model

Each receipt should include:

- Receipt ref.
- Transition kind.
- Subject ref.
- Actor or service ref.
- Idempotency key.
- Input refs.
- Output refs.
- Policy refs.
- Generated timestamp.
- Public-safe projection status.
- Verification or caveat refs.

Receipts should be append-only except for redaction or revocation metadata.

## Bun/Effect Boundary

Use Effect services for:

- `ArtifactStoreService`: persist, fetch, classify, and delete artifacts.
- `ReceiptLedgerService`: append lifecycle receipts.
- `RedactionProjectionService`: derive public-safe artifact and receipt views.
- `ArtifactIndexService`: query by run, mission, work order, kind, and
  visibility.
- `ReceiptRequirementService`: checks whether a claim has required refs.

Use Schema for artifact kinds, receipt kinds, visibility, retention, and claim
requirements. Use Stream for large artifact ingestion. Use Scope for temporary
payload cleanup.

## Safety Rules

- Raw prompts, raw logs, private repo data, provider payloads, wallet material,
  and secrets never enter public artifacts.
- Public receipts link to refs and summaries, not private payloads.
- Acceptance and settlement are separate receipts.
- Payment evidence does not prove accepted work.
- PR draft creation does not prove merge, deploy, or customer acceptance.
- Artifact visibility can narrow automatically but cannot widen without policy.
- Public projections must carry freshness and caveat data.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has substantial artifact and receipt concepts in
Pylon, Autopilot, payment, product-promise, and labor-market surfaces. The
terminal-agent README does not yet include a general artifact/receipt system
audit.

Related open issue anchors:

- #4779 writeback symmetry through artifact/authority layers.
- #4785 settlement visibility law.
- #4778 mission/work-order unification.
- #4768 overnight proof smoke.
- #4770 spend-to-evidence join.

Do not claim a terminal result, payout, settlement, deploy, or public product
promise unless the matching receipt requirement is present.

## Tests

Minimum coverage:

- Store every supported artifact kind with digest and visibility.
- Reject public projection of unsafe payloads.
- Append idempotent receipts for lifecycle transitions.
- Enforce receipt requirements for public claims.
- Join artifacts and receipts by run and mission.
- Delete eligible private artifacts while preserving safe tombstones.
- Prevent payment receipts from satisfying acceptance requirements.
- Verify stale projections are labeled.

## Decision

Artifacts and receipts should be the evidence layer under the terminal agent.
Summaries may explain outcomes, but refs and typed transition receipts are what
make work auditable.

