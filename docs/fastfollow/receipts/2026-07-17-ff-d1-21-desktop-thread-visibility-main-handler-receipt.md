---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_21.desktop_thread_visibility_main_handler.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "b6852266992055da79f7f00386e3a891ab449105"
claim_revision: "f50376c32b457571f09de41c62b2af24bbf6f3f4"
implementation_revision: "6f186971d996fc7606a22bdda30eb0bab2ce853e"
proof_rung: "desktop_thread_visibility_main_handler_seam"
observed_at: "2026-07-17T18:37:57Z"
---

# FF-D1-21 Desktop thread-visibility main-handler receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-21 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-20 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The tested implementation tree was landed unchanged on `origin/main` at
`6f186971d996fc7606a22bdda30eb0bab2ce853e`. This documentation-only release
annotation records that remote fact; it does not extend the implemented scope.

No open issue, worktree, or claim owns the two new visibility-handler paths.
Active work continues to own Desktop `main.ts` and broad renderer surfaces, so
this packet touches neither. It advances Fast Follow ProductSpec `FF-AC-04`,
`FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- registered exactly the fixed `openagents:thread-visibility:apply` channel and
  made teardown idempotent, with post-close calls rejected;
- rejected untrusted senders, trust-check failures, malformed or expanded
  envelopes, raw-content input, export input, and caller-supplied receipt
  metadata before policy application;
- passed only the decoded visibility intent with a host-supplied receipt
  reference and observation time into the private policy application;
- returned stored and unchanged results only when exact intent, idempotency,
  thread, and target identity matched, while retaining bounded policy
  rejections; and
- collapsed metadata-generation, application, malformed, mismatched, native,
  and path-bearing failures to `command_unavailable` without detail leakage.

## Proof

| Check                                 | Result                                      |
| ------------------------------------- | ------------------------------------------- |
| Focused handler/bridge/store tests    | PASS — 24/24                                |
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
Follow checks and every authored FF-D1-21 check pass.

## Honest boundary and next packet

This receipt closes only the tested main-process visibility handler seam. It
does not compose the handler in `main.ts`, connect a renderer command, authorize
or publish content to any audience, render disclosure pixels, prove an
installed runtime-rendered journey, or release/deploy anything. Those
residuals, remaining adapters, owner acceptance, and Day 1 completion remain
unclaimed.
