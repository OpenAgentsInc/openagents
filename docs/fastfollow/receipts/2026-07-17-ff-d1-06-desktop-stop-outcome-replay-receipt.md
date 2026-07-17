---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_06.desktop_stop_outcome_replay.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_implemented"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "0ccc8bc5a1a2b1b719b4d2dfdaf799c9d5ad72bc"
claim_revision: "e273868467"
proof_rung: "desktop_exact_stop_ack_replay"
observed_at: "2026-07-17T13:12:00Z"
---

# FF-D1-06 Desktop Stop-outcome replay receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-06 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-05 landed and released. FF-D1-04 supplied the
restart-stable ledger and FF-D1-05 supplied its strict lookup boundary. Current
`origin/main` contained no later Fast Follow claim, and a current GitHub issue
search found no open Fast Follow Stop/idempotency issue. Repository policy does
not require a feature issue for this accepted-plan packet.

This slice advances Fast Follow ProductSpec `FF-AC-06` and the working Stop
portion of `FF-AC-12`. AssuranceSpec obligations remain proposed proof design,
not a provider-owned verdict. No rendered, installed, remote-control, or Day 1
completion claim is made.

## Implemented packet

- replaced the local harness's per-click random Stop identity with a stable
  identity derived from the exact active turn;
- exposed the same stable ref-only identity for durable-conversation Stop;
- forwarded that identity through the converging Desktop chat host;
- made the shell reconcile the identity against the private durable outcome
  ledger before calling interrupt transport;
- replayed any retained acknowledgement, including pending, without a second
  interrupt signal; and
- made unavailable, corrupt, invalid, or conflicting reconciliation fail
  closed while leaving terminal UI state to the runtime's observed event.

The identity contains only thread, intent, and idempotency refs. This packet
adds no raw message content, remote adapter, credential path, terminal
fabrication, deployment, or release behavior.

## Proof

| Check | Result |
| --- | --- |
| Shell, local-harness, durable-conversation, and outcome-ledger tests | PASS — 193 passed, 11 skipped |
| Desktop typecheck | PASS |
| Fast Follow policy/spec checks | PASS — 20/20 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec package test | PASS — 104/104 |
| Sol document checks | PASS — 19/19 plus manifest check |

## Honest boundary and next packet

This receipt closes only exact local Desktop Stop acknowledgement replay. It
does not prove the interrupt reached a terminal runtime state; the independent
terminal event remains authoritative. Sync/mobile/Pylon adapters, historical
thread search/share/export/supersession, real rendered runtime evidence, owner
acceptance, release/deployment, and Day 1 completion remain unclaimed.
