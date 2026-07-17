---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_17.desktop_thread_export_workflow.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "600fcb58e653878b35de1fe505590c5e572c9daf"
claim_revision: "9b3e56b3ba372d036a0d4caa97999d71aad31f07"
proof_rung: "renderer_safe_canonical_export_create_then_write_workflow"
observed_at: "2026-07-17T17:10:16Z"
---

# FF-D1-17 Desktop canonical-export workflow receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-17 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-16 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

No open issue, worktree, or claim owns the two new workflow paths. The open
Desktop export issue matches are unrelated signed-release and download work.
Active work continues to own Desktop `main.ts` and broad renderer surfaces, so
this packet touches neither. It advances Fast Follow ProductSpec `FF-AC-04`,
`FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- added one named Effect workflow that validates the exact owner-only canonical
  export request before either host operation;
- sequenced create before write and stopped immediately on bounded creation
  rejection;
- delegated only the identity-bound canonical export receipt to write, never
  caller-selected events, authority relations, paths, bytes, filesystem,
  process, or provider authority;
- preserved cancellation and bounded create/write rejection while collapsing
  thrown, malformed, path-leaking, and native-error outcomes to bounded
  unavailable reasons;
- required a written artifact ref and digest to match the created receipt
  exactly; and
- projected only renderer-safe status and artifact identity, never the receipt,
  event payloads, paths, bytes, or native details.

## Proof

| Check                                      | Result                                      |
| ------------------------------------------ | ------------------------------------------- |
| Focused workflow/export-chain tests        | PASS — 37/37                                |
| Isolated workflow TypeScript compile       | PASS                                        |
| Fast Follow policy/spec checks             | PASS — 20/20                                |
| Behavior-contract checks                   | PASS — 36/36                                |
| ProductSpec focused test                   | PASS — 104/104                              |
| `pnpm run check`                           | PASS                                        |
| `pnpm run check:fast`                      | PASS                                        |
| Desktop package typecheck                  | PASS                                        |
| Targeted AssuranceSpec suite               | BASELINE FAIL — 189/190; environment digest |
| Accidental unscoped workspace test command | BASELINE FAIL — unrelated existing failures |

While this packet was in flight, disjoint landing `ea99862e52` repaired the
pre-existing Desktop conversation-lifecycle schema drift recorded by FF-D1-10
through FF-D1-16. After rebasing onto that current `main`, the complete Desktop
package typecheck passed. The production workflow also passes isolated strict
compilation. The targeted AssuranceSpec suite still reproduces the known
environment-profile digest snapshot mismatch at 189/190.

An inadvertently unscoped ProductSpec package command expanded to the whole
workspace and reproduced unrelated current-main failures before it was stopped;
the exact ProductSpec test subsequently passed 104/104. No baseline is owned,
absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only the tested renderer-safe create-then-write workflow.
It does not compose either export handler in Desktop `main.ts`, connect the
workflow to a renderer command, render disclosure/export pixels, authorize
broader audiences, prove an installed runtime-rendered journey, or
release/deploy anything. Those residuals, remaining adapters, owner acceptance,
and Day 1 completion remain unclaimed.
