---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_28.desktop_public_share_idempotent_retry.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "20f352f33213e4aa9c908f468391787cca496c74"
claim_revision: "4c4510bf6c564e354f1df7f633c3dd184a7b7fcb"
implementation_revision: "f2d88980e7ada9b732802468904cde0d8ba60d48"
proof_rung: "desktop_public_share_idempotent_retry"
observed_at: "2026-07-17T21:18:53Z"
---

# FF-D1-28 Desktop public-share idempotent retry receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-28 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-27 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, dependencies, and active worktrees were
reconciled before mutation.

No open bug issue or active worktree claims the public-publication transport.
Active Desktop `main.ts`, renderer, Full Auto, T3, and teardown work remained
outside this packet. Named-group membership and canonical export evidence
remain unavailable authorities. AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- derived one bounded visible-ASCII SHA-256 publication key from the exact
  decoded disclosure receipt identity, never from raw transcript or local
  content;
- read the host-custodied access token only after complete request validation,
  then sent at most two byte-identical ref-only `POST /api/share` requests with
  the same authorization-independent idempotency key and body;
- used a typed Effect failure plus `Schedule.recurs(1)` to retry only transport
  failures, ambiguous/retryable HTTP outcomes, unreadable bodies, or malformed
  and unsafe success evidence;
- accepted only FF-D1-27's exact `201`/`Idempotency-Replayed: false` first-
  creation evidence or `200`/`Idempotency-Replayed: true` replay evidence,
  followed by the existing bounded active same-origin response decoder;
- kept `400`, `401`, `403`, `409`, and `422` refusals definitive and one-shot,
  while exhausted ambiguity remains `publication_outcome_unknown`; and
- left workspace-members retry, server routes, schemas, migrations, host/UI
  composition, credentials, and content boundaries unchanged.

## Proof

| Check | Result |
| --- | --- |
| Focused public/disclosure/authorization/server-contract tests | PASS — 35/35 |
| Desktop package typecheck | PASS |
| Fast Follow package checks | PASS — 13/13 |
| Root Fast Follow coverage | PASS — 7/7 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 104/104 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS |
| `pnpm run check:fast` | PASS |
| Targeted AssuranceSpec suite | BASELINE FAIL — 189/190; environment digest |

The AssuranceSpec compiler reproduced only the known environment-profile
digest snapshot drift. Its two repository-inventory tests initially inherited
the task-local `GIT_WORK_TREE` override required by the separately mutated
shared `core.bare=true` configuration; rerunning those fixture-owning tests
without that override passed 2/2, leaving the same 189/190 baseline. This
packet did not mutate shared Git configuration or absorb any unrelated repair.

## Honest boundary and next packet

This receipt closes only bounded public-share client retry and replay
reconciliation. It does not add the same retry to workspace-members
publication, add named-group authority or publication, supply canonical export
evidence, compose resources in `main.ts`, connect renderer commands, render
pixels, or prove an installed runtime journey. Those residuals, owner
acceptance, and Day 1 completion remain unclaimed.

The exact tested implementation tree landed on `origin/main` at
`f2d88980e7ada9b732802468904cde0d8ba60d48` before this documentation-only
claim release.
