---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_18.desktop_thread_export_main_composition.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "dcfac105bbce524f87bdfa194d113fc535b40cce"
claim_revision: "a3ff7ef774d9358a8ce6cb345d35eb2c9dd8e081"
proof_rung: "atomic_canonical_export_main_handler_composition"
observed_at: "2026-07-17T17:34:28Z"
---

# FF-D1-18 Desktop canonical-export main composition receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-18 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-17 released. Current `origin/main`, all prior
Day 1 receipts and releases, Fast Follow revision 3, the accepted plan,
relevant ProductSpec and AssuranceSpec obligations, repository invariants,
open issues, known baselines, Git configuration, and active worktrees were
reconciled before mutation.

No open issue, worktree, or claim owns the two new composition paths. The open
Desktop export issue matches are unrelated signed-release and download work.
Active work continues to own Desktop `main.ts` and broad renderer surfaces, so
this packet touches neither. It advances Fast Follow ProductSpec `FF-AC-04`,
`FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- added one named Effect acquisition that composes the already-landed fixed
  write and create handler registrations with one shared trusted-sender gate;
- returned only an idempotent close resource, never raw handlers, IPC,
  filesystem, process, provider, path, receipt, or event authority;
- closed both handlers exactly once in reverse acquisition order;
- returned typed bounded `write` or `create` acquisition failures without
  projecting native messages, paths, causes, or stacks;
- avoided create registration when write acquisition failed; and
- rolled back an acquired write handler exactly once before reporting create
  acquisition failure, while cleanup failures remained contained.

## Proof

| Check                             | Result                                      |
| --------------------------------- | ------------------------------------------- |
| Focused composition/handler tests | PASS — 26/26                                |
| Isolated composition TypeScript   | PASS                                        |
| Desktop package typecheck         | PASS                                        |
| Fast Follow policy/spec checks    | PASS — 20/20                                |
| Behavior-contract checks          | PASS — 36/36                                |
| ProductSpec focused test          | PASS — 104/104                              |
| `pnpm run check`                  | PASS                                        |
| `pnpm run check:fast`             | PASS                                        |
| Targeted AssuranceSpec suite      | BASELINE FAIL — 189/190; environment digest |

The targeted AssuranceSpec suite reproduces the known environment-profile
digest snapshot mismatch at 189/190. It is outside this packet and is not
owned, absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only tested atomic composition and cleanup of the two
canonical-export main handler resources. It does not add the actual Desktop
`main.ts` call site, connect the workflow to a renderer command, render
disclosure/export pixels, authorize broader audiences, prove an installed
runtime-rendered journey, or release/deploy anything. Those residuals,
remaining adapters, owner acceptance, and Day 1 completion remain unclaimed.
