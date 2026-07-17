---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_29.desktop_workspace_share_idempotent_retry.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "6631bcba080ec4005030f9ad1d5bcfee4d890a18"
claim_revision: "4398a7765be0e338fb9426e4c1b51c38bec340fd"
implementation_revision: "9c492766148371f4dc27000bc2695e11f028a726"
proof_rung: "desktop_workspace_share_idempotent_retry"
observed_at: "2026-07-17T21:32:01Z"
---

# FF-D1-29 Desktop workspace-share idempotent retry receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-29 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-28 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, dependencies, and active worktrees were
reconciled before mutation.

No open bug issue or active worktree claims the workspace-publication
transport. Active Desktop `main.ts`, renderer, Full Auto, T3, and teardown work
remained outside this packet. Named-group membership and canonical export
evidence remain unavailable authorities. AssuranceSpec inventory remains
proposed proof design rather than a provider-owned verdict.

## Implemented packet

- derived one bounded visible-ASCII SHA-256 publication key from the exact
  decoded disclosure receipt identity, never from team names, local content,
  credentials, paths, or provider data;
- kept complete receipt, authorization, exact team scope/source, bounded team
  name, and origin validation before the single host-custodied credential read;
- sent at most two byte-identical ref-only `POST /api/share` requests with the
  same idempotency key, authorization, exact TeamMembers audience, and body;
- used a typed Effect failure plus `Schedule.recurs(1)` to retry only transport
  failures, ambiguous or retryable HTTP outcomes, unreadable bodies, or
  malformed, unsafe, and wrong-audience success evidence;
- accepted only FF-D1-27's exact `201`/`Idempotency-Replayed: false` first-
  creation evidence or `200`/`Idempotency-Replayed: true` replay evidence,
  followed by the existing bounded active same-origin and exact-audience
  response decoder; and
- kept `400`, `401`, `403`, `409`, and `422` refusals definitive and one-shot,
  while exhausted ambiguity remains `publication_outcome_unknown`.

## Proof

| Check | Result |
| --- | --- |
| Focused workspace/disclosure/authorization/Sync/server-contract tests | PASS — 43/43 |
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
packet did not mutate shared Git configuration or absorb unrelated repair.

## Honest boundary and next packet

This receipt closes only bounded workspace-members share client retry and
replay reconciliation. It does not add named-group authority or publication,
supply canonical export evidence, compose resources in `main.ts`, connect
renderer commands, render pixels, or prove an installed runtime journey.
Those residuals, owner acceptance, and Day 1 completion remain unclaimed.

The exact tested implementation tree landed on `origin/main` at
`9c492766148371f4dc27000bc2695e11f028a726` before this documentation-only
claim release.
