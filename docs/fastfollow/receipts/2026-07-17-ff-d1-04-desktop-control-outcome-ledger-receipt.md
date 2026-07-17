---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_04.desktop_control_outcome_ledger.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_verified"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 4
base_revision: "300f37878c19ef9503dd0c031fa16b4206532439"
proof_rung: "desktop_restart_stable_control_outcomes"
observed_at: "2026-07-17T12:33:00Z"
---

# FF-D1-04 Desktop control-outcome ledger receipt

## Authority and obligation reconciliation

The owner-accepted program and the bounded FF-D1-04 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-03 landed. The claim was reconciled onto the
then-current `origin/main` before product mutation. This slice advances Fast
Follow ProductSpec `FF-AC-06` and the durable-outcome portion of `FF-AC-12`.
AssuranceSpec obligations remain proposed proof design rather than a
provider-owned verdict.

## Implemented packet

- added one schema-checked renderer/preload/main record boundary for ref-only
  Queue, Steer, and Stop outcomes;
- added a private, mode-0600, atomic JSON ledger below Desktop user data,
  bounded to the newest 512 records;
- reconstructs identical records after close/reopen and treats exact retries
  as idempotent;
- permits only monotonic advancement from pending evidence and rejects
  cross-thread/identity reuse or conflicting non-pending evidence;
- fails closed on corrupt ledgers and persistence failures instead of
  overwriting or consuming unrecorded delivery success; and
- rejects extra/raw fields, so message bodies, prompts, credentials, and
  unobserved terminal events cannot enter the persisted record.

The shell waits for durable admission before a successful Queue or Steer
acknowledgement clears the draft. Stop outcomes are likewise recorded before
the handler returns. The underlying provider action remains independent of
this evidence ledger.

## Proof

| Check | Result |
| --- | --- |
| Store, shell, and converging-host focused tests | PASS — 164 passed, 11 skipped |
| Desktop typecheck | PASS |
| Fast Follow checks | PASS — 7/7 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec package test | PASS — 104/104 |
| `pnpm run check` | PASS |

Local implementation commit before final main integration:
`0e6ca0b6b107d0079aa8efd2943120ecc02f5885`.

## Honest boundary and next packet

This receipt closes only provider-neutral local control-outcome persistence
across Desktop restart. It does not replay a lost acknowledgement, dispatch a
control twice, or infer terminal runtime observation. Lost-ACK
reconciliation, Sync/mobile/Pylon adapters, rendered runtime evidence, owner
acceptance, release/deployment, and Day 1 completion remain unclaimed.
