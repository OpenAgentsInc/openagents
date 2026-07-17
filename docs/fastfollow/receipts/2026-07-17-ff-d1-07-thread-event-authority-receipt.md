---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_07.thread_event_authority.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_implemented"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "0cfd9334c3"
claim_revision: "a475240d4b669c9723ef804572b0a0798f3de571"
proof_rung: "shared_thread_event_authority_relation_algebra"
observed_at: "2026-07-17T13:48:00Z"
---

# FF-D1-07 thread-event authority receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-07 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-06 released. Current `origin/main` also
contained the separately landed mobile controller and Sync portable-session
controls. Those paths did not overlap this shared schema slice. A current
GitHub issue search found no open Fast Follow supersession/revert issue or
competing claim; repository policy does not require a feature issue for this
accepted-plan packet.

This slice advances the accepted-event authority portion of Fast Follow
ProductSpec `FF-AC-03` and `FF-AC-04`. AssuranceSpec obligations remain
proposed proof design, not a provider-owned verdict. No Desktop consumer,
rendered surface, installed runtime, or Day 1 completion claim is made.

## Implemented packet

- added the provider-neutral `openagents.thread_event_authority.v1` union for
  distinct accepted, superseded, and reverted relation facts;
- kept authority facts ref-only, with bounded exact thread, event, relation,
  replacement, revert, and restored-event refs;
- rejected malformed refs and timestamps, raw transcript fields, and
  self-referential supersession or revert triples;
- added deterministic projection over append-only observation order with
  explicit missing, resolved, and conflict results; and
- failed closed on invalid evidence, cross-thread facts, duplicate relation
  identity, ambiguous ordering, and invalid authority transitions.

The new contract supplies evidence classification only. It grants no mutation,
provider, transcript, sharing, export, visibility, acceptance, deployment, or
release authority.

## Proof

| Check                                | Result                           |
| ------------------------------------ | -------------------------------- |
| Focused thread-event authority tests | PASS — 6/6                       |
| Agent runtime schema typecheck       | PASS                             |
| Fast Follow policy/spec checks       | PASS — 20/20                     |
| Behavior-contract checks             | PASS — 36/36                     |
| ProductSpec package test             | PASS — 104/104                   |
| Sol document checks                  | PASS — 19/19 plus manifest check |
| `pnpm run check` and `check:fast`    | PASS                             |

The package's unscoped `test` command expanded to the workspace suite and
observed an unrelated API Worker deploy-bundle contract failure. The packet's
exact focused test and typecheck passed. All required repository gates recorded
above passed before landing.

## Honest boundary and next packet

This receipt closes only the shared accepted-event authority relation algebra.
It does not wire these relations into Desktop history or prove share/export
visibility, adapter behavior, a rendered surface, or a live runtime. Those
residuals, owner acceptance, release/deployment, and Day 1 completion remain
unclaimed.
