---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_27.idempotent_share_create_reconciliation.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "eb15ce99c54af497874a998192b1afbb2fa8268b"
claim_revision: "46b95cc49ab7a2202f8b2470394936a495df2238"
implementation_revision: "4ef8dc7858aad3e07c81d4c2707257ecb28c5076"
proof_rung: "idempotent_share_create_reconciliation"
observed_at: "2026-07-17T20:58:37Z"
---

# FF-D1-27 idempotent share-create reconciliation receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-27 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-26 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, dependencies, and active worktrees were
reconciled before mutation.

The tested implementation tree was landed on current `origin/main` at
`4ef8dc7858aad3e07c81d4c2707257ecb28c5076`. This documentation-only release
annotation records that remote fact; it does not extend scope.

No open bug issue claims this gap. Two July 4 worktrees containing unrelated
share-route edits had no status or process evidence for more than 90 minutes
and were audited stale under the claim protocol. Active Desktop `main.ts`,
renderer, T3 UI, and teardown work remained outside this packet. AssuranceSpec
inventory remains proposed proof design rather than a provider-owned verdict.

## Implemented packet

- added a bounded visible-ASCII `Idempotency-Key` contract for authenticated
  `POST /api/share` creation;
- derived an RFC 9562 UUIDv5 share identity from a product-owned namespace,
  the authenticated owner identity, and the exact key, keeping different
  owners isolated while preserving the existing random-ID path when absent;
- ran ordinary source authorization and redacted projection construction, then
  replayed only an active, unrevoked, unexpired record whose owner, canonical
  URL, source, audience, title, redaction policy, and expiry matched exactly;
- returned `200` plus `Idempotency-Replayed: true` for a semantic replay,
  preserved `201` for first creation, and refused conflicting reuse with a
  bounded `409 idempotency_conflict` response;
- reconciled a concurrent unique-create race by rereading and accepting only
  the same exact semantics; and
- added no migration, shared response-schema widening, credential exposure,
  content upload, audience expansion, deployment, or release action.

## Proof

| Check | Result |
| --- | --- |
| Focused idempotency/share route/projection tests | PASS — 32/32 |
| API package typecheck | PASS; two pre-existing Effect advisories only |
| Fast Follow package checks | PASS — 13/13 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 104/104 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS |
| `pnpm run check:fast` | PASS with task-local `GIT_WORK_TREE`; shared `core.bare=true` collision preserved |
| Targeted AssuranceSpec suite | BASELINE FAIL — 188/190; digest drift plus distribution timeout |
| Root Fast Follow coverage | BASELINE FAIL — 6/7; teardown seed owned separately |

The AssuranceSpec compiler reproduced the known environment-profile digest
snapshot drift. The offline distribution proof also exceeded its fixed 30
second test bound twice on this machine (33.5 and 46.1 seconds) without touching
this packet's paths. Root Fast Follow coverage still lacks only the separately
owned mobile-component teardown seed. A concurrent task changed the shared Git
configuration to `core.bare=true`; repository checks passed with a process-
local work-tree override, and this packet did not mutate that shared setting.

## Honest boundary and next packet

This receipt closes only server-side ambiguous-create reconciliation. It does
not make the existing Desktop transports retry with an idempotency key, add
named-group authority or publication, supply canonical export evidence,
compose resources in `main.ts`, connect renderer commands, render pixels, or
prove an installed runtime journey. Those residuals, owner acceptance, and Day
1 completion remain unclaimed.
