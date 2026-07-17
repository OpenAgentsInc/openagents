---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_05.desktop_control_outcome_replay.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "28517a9777d6538b5832561eb1d2b666cba6cc08"
proof_rung: "desktop_exact_control_ack_replay"
observed_at: "2026-07-17T12:52:51Z"
---

# FF-D1-05 Desktop control-outcome replay receipt

## Authority and obligation reconciliation

The owner-accepted program and bounded FF-D1-05 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-04 was recovered, verified, receipted, and
landed. Current `origin/main` also included the separately admitted Fast Follow
manifest compiler. That infrastructure change did not alter the ordered Day 1
residuals. No open Fast Follow issue or competing packet claim existed.

This slice advances Fast Follow ProductSpec `FF-AC-06` and the exact retry
portion of `FF-AC-12`. AssuranceSpec obligations remain proposed proof design,
not a provider-owned verdict. Broader installed and rendered Desktop criteria
remain unmet and are not claimed complete.

## Implemented packet

- added a strict ref-only lookup contract over exact thread, intent, and
  idempotency identity;
- added trusted preload/main lookup IPC over the private restart-stable outcome
  ledger;
- made Queue and Steer consult retained evidence before any adapter dispatch;
- replayed retained queued/applied acknowledgement through the existing draft
  transition without sending transport twice;
- retained pending acknowledgement as pending and refused duplicate dispatch;
- made corrupt, invalid, conflicting, or unavailable reconciliation fail
  closed with the draft and retry identity intact; and
- preserved normal dispatch for an explicitly missing ledger identity; a
  reconciliation failure stays fail-closed.

The lookup result contains only schema-checked refs and acknowledgement axes.
It contains no raw message body, prompt, credential, or invented terminal
runtime observation.

## Proof

| Check | Result |
| --- | --- |
| Store, boundary, shell Queue/Steer replay, and converging-host tests | PASS — 171 passed, 11 skipped |
| Desktop typecheck | PASS |
| Fast Follow policy/spec checks | PASS — 20/20 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec package test | PASS — 104/104 |
| Sol document checks | PASS — 19/19 plus manifest check |
| `pnpm run check` | PASS |

## Honest boundary and next packet

This receipt closes only exact Queue/Steer acknowledgement replay from the
local Desktop ledger. Stop currently mints retry identity inside its adapter
and is not replay-reconciled by this packet. Sync/mobile/Pylon adapters,
historical thread search/share/export/supersession surfaces, real rendered
runtime evidence, owner acceptance, release/deployment, and Day 1 completion
remain unclaimed.
