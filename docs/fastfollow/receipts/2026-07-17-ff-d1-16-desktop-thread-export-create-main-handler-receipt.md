---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_16.desktop_thread_export_create_main_handler.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "337d01b44b405097b1f2747f845af5d94bc71a61"
claim_revision: "9ca9bedfe7800879791bf9104d23319536372f39"
proof_rung: "trusted_sender_canonical_export_creation_handler_seam"
observed_at: "2026-07-17T16:43:52Z"
---

# FF-D1-16 Desktop canonical-export creation-handler receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-16 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-15 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The open Fast Follow parser issue is a separate reproducible bug and does not
supersede this owner-ordered packet. No open issue, worktree, or claim owns the
two new creation-handler paths. Active work continues to own Desktop `main.ts`,
history, shell, renderer, update, and release surfaces. This packet advances
Fast Follow ProductSpec `FF-AC-04`, `FF-AC-06`, and `FF-AC-12`; AssuranceSpec
inventory remains proposed proof design rather than a provider-owned verdict.

## Implemented packet

- registered exactly the fixed `openagents:thread-export:create` channel
  through an injected host seam with one idempotent cleanup;
- rejected closed, untrusted, throwing-trust-check, malformed, raw-bearing, and
  broader-audience requests before command invocation;
- passed only the exact schema-decoded owner-only canonical export intent to
  the existing command coordinator, never renderer-selected events, authority
  relations, receipt metadata, digests, paths, bytes, filesystem, process, or
  provider authority;
- preserved exact stored, unchanged, and bounded rejected outcomes only after
  FF-D1-15 identity-bound result decoding; and
- collapsed thrown, mismatched, malformed, and path-leaking command outcomes
  to `command_unavailable` without projecting native details.

## Proof

| Check                                | Result                                           |
| ------------------------------------ | ------------------------------------------------ |
| Focused handler/bridge/command tests | PASS — 25/25                                     |
| Isolated handler TypeScript compile  | PASS                                             |
| Fast Follow policy/spec checks       | PASS — 20/20                                     |
| Behavior-contract checks             | PASS — 36/36                                     |
| ProductSpec focused test             | PASS — 104/104                                   |
| `pnpm run check`                     | PASS                                             |
| `pnpm run check:fast`                | PASS                                             |
| Desktop package typecheck            | BASELINE FAIL — unrelated lifecycle schema drift |
| Targeted AssuranceSpec suite         | BASELINE FAIL — 189/190; environment digest      |

The Desktop typecheck reproduces only the pre-existing lifecycle-schema
failures recorded by FF-D1-10 through FF-D1-15: existing conversation/gateway
fixtures omit thread `status`, and one live-subscription fake omits
`renameThread` and `setThreadStatus`. The new handler files produce no reported
type errors and pass isolated strict compilation. The targeted AssuranceSpec
suite reproduces the unrelated environment-profile digest snapshot mismatch at
189/190. Neither baseline is owned, absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only the tested trusted-sender creation handler and
registration lifecycle seam. It does not compose either export handler in
Desktop `main.ts`, connect a renderer create-then-write command, render
disclosure/export pixels, authorize broader audiences, prove an installed
runtime-rendered journey, or release/deploy anything. Those residuals,
remaining adapters, owner acceptance, and Day 1 completion remain unclaimed.
