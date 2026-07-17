---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_23.desktop_thread_visibility_audience_authorization.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_implemented"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "9de9b278d13c326996c666fb8903f72d7adc3ccf"
claim_revision: "dfb0569ea260d8d6e77d8241075333daa2b4f8e3"
proof_rung: "desktop_thread_visibility_audience_authorization_decision"
observed_at: "2026-07-17T19:06:52Z"
---

# FF-D1-23 Desktop thread-visibility audience-authorization receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-23 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-22 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

No open issue, worktree, or claim owns the two new audience-authorization
paths. Active work continues to own Desktop `main.ts` and broad renderer
surfaces, so this packet touches neither. It advances Fast Follow ProductSpec
`FF-AC-04`, `FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains
proposed proof design rather than a provider-owned verdict.

## Implemented packet

- added one exact bounded request carrying an applied visibility receipt,
  actor/owner refs, and at most 32 unique workspace authority facts with
  bounded unique group refs;
- authorized only the exact owner, an internet-readable target, matching
  workspace membership, matching named-group membership, or an explicitly
  configured matching workspace administrator;
- denied absent or mismatched authority without inferring membership, group,
  administrator, owner, or publication state;
- rejected raw-content, expanded envelope, export receipt, malformed target,
  duplicate workspace/group, and oversized authority input; and
- bound every allow/deny decision to the exact receipt, thread, and applied
  visibility version while exposing no content, paths, credentials, native
  errors, or authority-store details.

## Proof

| Check                                 | Result                                      |
| ------------------------------------- | ------------------------------------------- |
| Focused authorization/disclosure tests | PASS — 15/15                                |
| Desktop package typecheck             | PASS                                        |
| Fast Follow package checks            | PASS — 13/13                                |
| Behavior-contract checks              | PASS — 36/36                                |
| ProductSpec focused test              | PASS — 104/104                              |
| Sol document tests and manifest       | PASS — 19/19                                |
| `pnpm run check`                      | PASS                                        |
| `pnpm run check:fast`                 | PASS                                        |
| Targeted AssuranceSpec suite          | BASELINE FAIL — 189/190; environment digest |
| Root Fast Follow coverage             | BASELINE FAIL — teardown seed still owned   |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch. Root Fast Follow teardown coverage still fails only
because committed `FASTFOLLOW.md` does not yet reference the separately landed
mobile-component teardown. Another active checkout owns that seed update; this
packet neither absorbed nor weakened the unrelated repair. The package Fast
Follow checks and every authored FF-D1-23 check pass.

## Honest boundary and next packet

This receipt closes only the pure applied-policy authorization decision. It
does not query a real authority store, fetch or publish content, transport a
thread, call from `main.ts`, connect a renderer command, render disclosure
pixels, prove an installed runtime journey, or release/deploy anything. Those
residuals, owner acceptance, and Day 1 completion remain unclaimed.
