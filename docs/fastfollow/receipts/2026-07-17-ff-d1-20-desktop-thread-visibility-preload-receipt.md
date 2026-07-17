---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_20.desktop_thread_visibility_preload.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "3d2797a54fadfba9e783e997bee79d0a51d197b9"
claim_revision: "10878526eb95201820e0ceae88a85351d92cd18c"
proof_rung: "sandboxed_thread_visibility_apply_boundary"
observed_at: "2026-07-17T18:17:30Z"
---

# FF-D1-20 Desktop thread-visibility preload receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-20 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-19 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

No open issue, worktree, or claim owns the new visibility bridge paths or
`preload.cts`. Active work continues to own Desktop `main.ts` and broad renderer
surfaces, so this packet touches neither. It advances Fast Follow ProductSpec
`FF-AC-04`, `FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains
proposed proof design rather than a provider-owned verdict.

## Implemented packet

- added one fixed `openagents:thread-visibility:apply` channel contract and one
  sandboxed `threadVisibility.apply` preload method;
- admitted only an exact ref-only visibility intent with exact expected-version,
  audience, administrator-access, and target shapes;
- rejected raw-content, export, nested audience-member, extra-envelope, and
  caller-supplied receipt metadata before IPC invocation;
- decoded stored and unchanged results only when their ref-only applied receipt
  matched the exact intent, idempotency, thread, and target identity;
- preserved bounded FF-D1-19 rejection reasons and collapsed malformed replies
  or native failures to `command_unavailable`; and
- exposed no raw IPC, caller-selected channel, receipt metadata, filesystem,
  process, provider, membership, administrator, publication, or transport
  authority.

## Proof

| Check                                    | Result                                      |
| ---------------------------------------- | ------------------------------------------- |
| Focused bridge/store/disclosure tests    | PASS — 19/19                                |
| Desktop package typecheck                | PASS                                        |
| Desktop production build / built preload | PASS                                        |
| Fast Follow package checks               | PASS — 13/13                                |
| Behavior-contract checks                 | PASS — 36/36                                |
| ProductSpec focused test                 | PASS — 104/104                              |
| Sol document tests and manifest          | PASS — 19/19                                |
| `pnpm run check`                         | PASS                                        |
| `pnpm run check:fast`                    | PASS                                        |
| Targeted AssuranceSpec suite             | BASELINE FAIL — 189/190; environment digest |
| Root Fast Follow coverage                | BASELINE FAIL — new teardown not yet seeded |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch. Current `origin/main` also contains a newly landed
mobile-component teardown that committed `FASTFOLLOW.md` does not yet reference,
so the root teardown-coverage test fails on untouched main. Another active
checkout owns `FASTFOLLOW.md`; this packet neither absorbed nor weakened that
unrelated repair. The schema package and authored FF-D1-20 checks pass.

## Honest boundary and next packet

This receipt closes only the sandboxed visibility apply request/result boundary.
It does not register or compose the main-process handler, connect a renderer
command, authorize or publish content to any audience, render disclosure pixels,
prove an installed runtime-rendered journey, or release/deploy anything. Those
residuals, remaining adapters, owner acceptance, and Day 1 completion remain
unclaimed.
