---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_19.desktop_thread_visibility_policy_store.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "a86ffc74992137c8ddafacfa201fa37dd6665dbd"
claim_revision: "9587300f6299c61a210686e6bc83c2fcfc006b86"
implementation_revision: "780887ff5fe74dafb857caaf482c8a36f580b9a9"
proof_rung: "restart_stable_private_thread_visibility_policy_evidence"
observed_at: "2026-07-17T17:47:42Z"
---

# FF-D1-19 Desktop thread-visibility policy store receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-19 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-18 released. Current `origin/main`, all prior
Day 1 receipts and releases, Fast Follow revision 3, the accepted plan,
relevant ProductSpec and AssuranceSpec obligations, repository invariants,
open issues, known baselines, Git configuration, and active worktrees were
reconciled before mutation.

The tested implementation tree was landed unchanged on `origin/main` at
`780887ff5fe74dafb857caaf482c8a36f580b9a9`. This documentation-only release
annotation records that remote fact; it does not extend the implemented scope.

No open issue, worktree, or claim owns the two new visibility-store paths.
Active work continues to own Desktop `main.ts` and broad renderer surfaces, so
this packet touches neither. It advances Fast Follow ProductSpec `FF-AC-04`,
`FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- added a private main-process visibility-policy ledger using named Effect
  operations and bounded atomic persistence;
- accepted only decoded ref-only `thread.visibility.set` intents, keeping raw
  thread material and export intents outside the adapter;
- permitted first observation only from explicit version zero or
  `not_observed`, then required exact optimistic versions and advanced them
  monotonically by one;
- returned the identical stored receipt for exact retry while rejecting stale
  versions and conflicting intent or idempotency identity reuse;
- reconstructed exact current policy and receipt evidence after reopen and
  refused corrupt history without overwriting it; and
- recorded explicit owner-only, workspace, group, or internet-readable policy
  evidence without publishing content or granting audience membership,
  administrator, network, provider, deployment, or release authority.

## Proof

| Check                                  | Result                                      |
| -------------------------------------- | ------------------------------------------- |
| Focused visibility/disclosure tests    | PASS — 14/14                                |
| Desktop package typecheck              | PASS                                        |
| Fast Follow policy/spec checks         | PASS — 20/20                                |
| Behavior-contract checks               | PASS — 36/36                                |
| ProductSpec focused test               | PASS — 104/104                              |
| Sol document tests and manifest        | PASS — 19/19                                |
| `pnpm run check`                       | PASS                                        |
| `pnpm run check:fast`                  | PASS                                        |
| Targeted AssuranceSpec suite           | BASELINE FAIL — 189/190; environment digest |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch at 189/190. It is outside this packet and is not
owned, absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only private restart-stable application and evidence for
explicit visibility policy. It does not compose the store into Desktop
`main.ts`, expose a preload or renderer command, authorize or publish content
to any audience, render disclosure/export pixels, prove an installed
runtime-rendered journey, or release/deploy anything. Those residuals,
remaining adapters, owner acceptance, and Day 1 completion remain unclaimed.
