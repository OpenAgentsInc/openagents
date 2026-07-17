---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.owner_expansion.ff_d1_01.20260716"
class: "admission_and_implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "f2c5591e3b5a2c160f436fb62633a6367272c70d"
proof_rung: "contract_and_desktop_lowering"
observed_at: "2026-07-17T04:40:49Z"
---

# Fast Follow expansion admission and FF-D1-01 receipt

## Supersession

The owner conversation quoted in the
[accepted plan](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
supersedes the earlier Day 1 `blocked_by_policy` disposition for new work. The
historical gap and gap receipt are not edited: they remain truthful for their
exact revision and observation time.

The accepted plan activates the ordered five-day initial program and admits
bounded implementation packets without a feature issue. This is compatible
with repository issue policy, which reserves GitHub issues for reproducible
bugs.

## Implemented packet

FF-D1-01 adds the first Day 1 implementation foundation:

- `openagents.runtime_control_intent.v2` with distinct `turn.queue`,
  `turn.steer`, and `turn.interrupt` discriminants;
- stable ref-only command identity, explicit target generation or typed
  unknown, ordering, origin, and admission deadline;
- `openagents.runtime_control_outcome.v1`, keeping admission, delivery, and
  terminal observation separate;
- lost-ACK replay classification that distinguishes exact retry from
  conflicting identity reuse; and
- Desktop composer lowering of existing Queue and Steer decisions into the
  shared contract while preserving current local adapter payloads.

Raw message text remains in the owner-local Desktop envelope and never enters
the shared control contract. The Desktop shell dispatches by the canonical
semantic discriminant; its existing adapter methods remain unchanged.

## Proof

| Check | Result |
| --- | --- |
| Shared thread-control and existing runtime-schema tests | PASS |
| `packages/agent-runtime-schema` typecheck | PASS |
| Desktop composer and shell tests | PASS |
| Focused Desktop local runtime/turn/queue tests | PASS |
| Desktop typecheck | PASS |
| Fast Follow checks | PASS |
| Behavior-contract checks | PASS |
| ProductSpec checks | PASS |
| Sol document manifest/check/tests | PASS |
| `pnpm run check` | PASS |

## Honest boundary and next packet

This receipt does not claim Day 1 complete. The shared schema and Desktop
Queue/Steer lowering are implemented; durable shared dispatch/outcomes for
Khala Sync, Pylon, mobile, and other adapters are not. Stop is represented by
the canonical shared discriminant, while the existing Desktop Stop adapter has
not yet been migrated to consume the new envelope.

The next Day 1 packet should wire compatible adapters to the shared contract,
return typed `unsupported` rather than rerouting, persist outcomes across
restart/retry, and produce real Queue/Steer/Stop runtime receipts. A later
surface packet owns rendered thread visibility/share/export and supersession
relationships.

No deployment, release, provider spend, credential, settlement, public
promise, or owner acceptance is claimed here.
