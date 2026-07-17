---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_22.desktop_thread_visibility_main_composition.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "6b8ca94e0b6322762a6243ed9dac0ef9e7d7d8da"
claim_revision: "6340ce8ff9e7719bfb6e83699f430d24c393068f"
implementation_revision: "4f3151a1666df8ecc2c2d7b8fe14b939f6af4620"
proof_rung: "desktop_thread_visibility_main_composition_resource"
observed_at: "2026-07-17T18:55:29Z"
---

# FF-D1-22 Desktop thread-visibility main-composition receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-22 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-21 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The tested implementation tree was landed on current `origin/main` at
`4f3151a1666df8ecc2c2d7b8fe14b939f6af4620` after rebasing over disjoint
renderer work and regenerating the Sol manifest. This documentation-only
release annotation records that remote fact; it does not extend scope.

No open issue, worktree, or claim owns the two new visibility-composition
paths. Active work continues to own Desktop `main.ts` and broad renderer
surfaces, so this packet touches neither. It advances Fast Follow ProductSpec
`FF-AC-04`, `FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains
proposed proof design rather than a provider-owned verdict.

## Implemented packet

- added one Effect composition resource that opens the private restart-stable
  visibility store and registers exactly one FF-D1-21 fixed-channel handler;
- adapted the store's Effect operation inside the main-process boundary while
  exposing only the handler lifetime resource;
- preserved stored, unchanged replay, corrupt-store, and other bounded policy
  results without exposing the store, its path, Effect runtime, or raw errors;
- made close idempotent and suppressed native cleanup details; and
- proved close/reopen against the same private file returns the identical
  original receipt for an exact retry, without advancing visibility version or
  substituting newly generated metadata.

## Proof

| Check                                      | Result                                      |
| ------------------------------------------ | ------------------------------------------- |
| Focused composition/visibility tests       | PASS — 29/29                                |
| Desktop package typecheck                  | PASS                                        |
| Fast Follow package checks                 | PASS — 13/13                                |
| Behavior-contract checks                   | PASS — 36/36                                |
| ProductSpec focused test                   | PASS — 104/104                              |
| Sol document tests and manifest            | PASS — 19/19                                |
| `pnpm run check`                           | PASS                                        |
| `pnpm run check:fast`                      | PASS                                        |
| Targeted AssuranceSpec suite               | BASELINE FAIL — 189/190; environment digest |
| Root Fast Follow coverage                  | BASELINE FAIL — teardown seed still owned   |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch. Root Fast Follow teardown coverage still fails only
because committed `FASTFOLLOW.md` does not yet reference the separately landed
mobile-component teardown. Another active checkout owns that seed update; this
packet neither absorbed nor weakened the unrelated repair. The package Fast
Follow checks and every authored FF-D1-22 check pass.

## Honest boundary and next packet

This receipt closes only the tested visibility handler/store composition
resource. It does not call that resource from `main.ts`, connect a renderer
command, authorize or publish content to any audience, render disclosure
pixels, prove an installed runtime-rendered journey, or release/deploy
anything. Those residuals, remaining adapters, owner acceptance, and Day 1
completion remain unclaimed.
