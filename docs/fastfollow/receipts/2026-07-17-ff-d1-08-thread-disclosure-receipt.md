---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_08.thread_disclosure.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "36fff3452dc5e533e9dce40b6838dbfe526e9e11"
claim_revision: "06ce1def9dea5e112a9dda00b0abe9b3b020e67a"
proof_rung: "shared_thread_disclosure_and_export_receipt_algebra"
observed_at: "2026-07-17T14:03:00Z"
---

# FF-D1-08 thread disclosure and export receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-08 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-07 released. Current `origin/main` also
contained the separately landed Sync/mobile attention projection, target
resolution, and inbox work. Those paths did not overlap this shared schema
slice. Active Desktop work owned history, shell, runtime-conversation, and
rendered surfaces, so this packet explicitly excluded them. Current GitHub
issue searches found no open Fast Follow disclosure/export issue or competing
claim; repository policy does not require a feature issue for this plan packet.

This slice advances Fast Follow ProductSpec `FF-AC-04`, `FF-AC-06`, and
`FF-AC-12`. The bounded AssuranceSpec inventory is proposed proof design, not a
provider-owned verdict, and does not prove this new product delta. No Desktop
consumer, actual sharing, exported artifact, rendered surface, installed
runtime, or Day 1 completion claim is made.

## Implemented packet

- added distinct provider-neutral visibility-change and export-create intents
  with stable intent and idempotency identity;
- modeled owner-only, workspace-member, named-group, and internet-readable
  audiences without an ambiguous `unlisted` state;
- kept administrator access as an independent explicit axis and rejected
  contradictory cross-workspace audience/admin policy;
- added distinct accepted-pending, rejected, failed, visibility-applied, and
  export-created receipt evidence with intent-kind consistency;
- bound successful export evidence to exact artifact ref, SHA-256 digest,
  format, and artifact audience without carrying exported bytes; and
- added deterministic new/exact-retry/conflicting-reuse classification while
  rejecting malformed refs, timestamps, digests, and nested raw thread fields.

These contracts classify intent and evidence only. They grant no disclosure,
export, persistence, transport, provider, acceptance, deployment, or release
authority.

## Proof

| Check                             | Result                           |
| --------------------------------- | -------------------------------- |
| Focused thread-disclosure tests   | PASS — 8/8                       |
| Agent runtime schema typecheck    | PASS                             |
| Fast Follow policy/spec checks    | PASS — 20/20                     |
| Behavior-contract checks          | PASS — 36/36                     |
| ProductSpec package test          | PASS — 104/104                   |
| Sol document checks               | PASS — 19/19 plus manifest check |
| `pnpm run check` and `check:fast` | PASS                             |

The packet implementation landed on `main` as
`9582538eb287cb77f1c50ad50c69965ef65d839c` with an exact remote tree match.

## Honest boundary and next packet

This receipt closes only the shared thread disclosure/export intent and receipt
algebra. It does not wire a Desktop command or pixel, persist or transport an
intent, create or inspect an exported artifact, or prove real disclosure. Those
residuals, remaining adapters, rendered runtime evidence, owner acceptance,
release/deployment, and Day 1 completion remain unclaimed.
