---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_02.desktop_stop_control.20260716"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "fdc673ad54120ff14ff3483d13a9051a2469b258"
proof_rung: "desktop_adapter_control_lowering"
observed_at: "2026-07-17T05:10:00Z"
---

# FF-D1-02 Desktop Stop control receipt

## Authority and obligation reconciliation

The owner-accepted program and the bounded FF-D1-02 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-01. Current `origin/main`, open issue state,
existing worktrees/processes, and Fast Follow receipts showed no competing
Day 1 packet claim.

This slice advances Fast Follow ProductSpec `FF-AC-06` and the Stop portion of
`FF-AC-12`. The current bounded `specs/**` AssuranceSpec material is proposed
proof design, not a provider-owned verdict; it contains no admitted obligation
that can promote this packet. The Phase 2 Desktop ProductSpec still requires
an installed rendered journey before its broader Stop/Steer criterion can be
claimed complete.

## Implemented packet

- added one canonical constructor for ref-only `turn.interrupt` intents and
  one outcome constructor that keeps terminal observation pending;
- migrated the local Fable/Codex ChatHost adapter and the durable conversation
  ChatHost adapter to mint the exact active thread/turn control before
  signaling their existing provider-specific transports;
- returned `openagents.runtime_control_outcome.v1` acknowledgement with
  admission, delivery, and terminal axes kept separate;
- mapped an unavailable durable adapter to typed `unsupported` without queue,
  steer, or new-turn fallback; and
- preserved the legacy boolean interrupt seam for existing capability callers
  while the Desktop Stop handler prefers the new canonical seam.

Missing, terminal, stale, or thread-mismatched active state returns no control
outcome and dispatches nothing. Raw message content never enters the control
envelope.

## Proof

| Check | Result |
| --- | --- |
| Composer, local adapter, durable adapter, shell, capability, and Electron-boundary tests | PASS — 239 passed, 11 skipped |
| Desktop typecheck | PASS |
| Fast Follow checks | PASS — 7/7 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec package test | PASS — 104/104 |
| `pnpm run check` | PASS |

## Honest boundary and next packet

This receipt closes only foreground Desktop Stop lowering and typed adapter
acknowledgement. Outcomes are not yet durably stored across restart or
lost-ACK replay. Queue/Steer outcome persistence, Sync/mobile/Pylon adapters,
thread search/share/export/supersession surfaces, rendered Desktop evidence,
owner acceptance, release, deployment, and Day 1 completion remain unclaimed.
