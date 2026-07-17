---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_03.desktop_queue_steer_control.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "cf00d8e14f7af3f236412e94fd6105fa43277b88"
proof_rung: "desktop_adapter_control_lowering"
observed_at: "2026-07-17T11:47:06Z"
---

# FF-D1-03 Desktop Queue/Steer control receipt

## Authority and obligation reconciliation

The owner-accepted program and the bounded FF-D1-03 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after landed FF-D1-01 and FF-D1-02. Current
`origin/main`, open issue state, existing worktrees/processes, and Fast Follow
receipts showed no competing Day 1 packet claim. An unrelated command-palette
change that touched the shell landed before implementation and was reconciled
into this packet's base without changing its Queue/Steer behavior.

This slice advances Fast Follow ProductSpec `FF-AC-06` and the Queue/Steer
portion of `FF-AC-12`. The bounded AssuranceSpec material remains proposed
proof design rather than a provider-owned verdict. The broader Desktop
ProductSpec criteria still require installed rendered evidence and are not
claimed complete here.

## Implemented packet

- added one provider-neutral outcome constructor shared by Queue, Steer, and
  the already-landed Stop outcome path;
- made the Desktop shell prefer typed Queue/Steer ChatHost seams while retaining
  the existing provider-specific seams as compatibility fallbacks;
- validated exact control kind, thread, turn (for Steer), and message identity
  before local adapter dispatch, with mismatches rejected without transport;
- returned typed queued delivery for local and durable-conversation Queue and
  typed applied, unsupported, or failed delivery for local Steer;
- forwarded canonical Queue/Steer through the converging Desktop host without
  translating either control into the other; and
- retained lost acknowledgement as pending admission/delivery evidence instead
  of reporting success.

Terminal runtime observation remains independent and pending in every adapter
acknowledgement. Raw message content remains outside the ref-only control
envelope.

## Proof

| Check | Result |
| --- | --- |
| Composer, shell, local adapter, durable adapter, and capability tests | PASS — 244 passed, 11 skipped |
| Desktop typecheck | PASS |
| Fast Follow checks | PASS — 7/7 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec package test | PASS — 104/104 |
| Sol document checks | PASS — 19/19 plus manifest check |
| `pnpm run check` | PASS |

## Honest boundary and next packet

This receipt closes only foreground Desktop Queue/Steer lowering and typed
adapter acknowledgement. Outcomes are not durably stored across restart and
lost acknowledgements are not yet replay-reconciled. Sync/mobile/Pylon
adapters, thread search/share/export/supersession surfaces, real rendered
runtime evidence, owner acceptance, release, deployment, and Day 1 completion
remain unclaimed.
