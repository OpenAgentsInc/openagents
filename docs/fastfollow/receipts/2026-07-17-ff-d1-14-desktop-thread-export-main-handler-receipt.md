---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_14.desktop_thread_export_main_handler.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "88c24ec985469ef4a434a96ade59f074bf40bb4e"
claim_revision: "9692c8d7d243d7378e25593844123aabd21f7f47"
proof_rung: "trusted_sender_canonical_export_main_handler_seam"
observed_at: "2026-07-17T16:09:50Z"
---

# FF-D1-14 Desktop thread-export main-handler receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-14 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-13 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The open Fast Follow parser issue is a separate reproducible bug and does not
supersede this owner-ordered packet. Active work continued to own Desktop
`main.ts`, history, shell, renderer, update, and release surfaces. This packet
therefore changed only two new handler files plus its receipt and ledger paths.
It advances Fast Follow ProductSpec `FF-AC-04`, `FF-AC-06`, and `FF-AC-12`;
AssuranceSpec inventory remains proposed proof design rather than a
provider-owned verdict.

## Implemented packet

- registered exactly the fixed `openagents:thread-export:write` channel through
  an injected host seam with one idempotent cleanup;
- rejected closed, untrusted, throwing-trust-check, malformed, and
  broader-audience requests before transport invocation;
- passed only the exact schema-decoded, ref-only canonical export receipt to
  the existing path-free transport boundary, never renderer-selected channel,
  destination, bytes, filesystem, process, or provider authority;
- preserved only exact cancelled, written, and bounded rejected transport
  results through the shared result decoder; and
- collapsed thrown, malformed, and path-leaking outcomes to the typed
  `transport_unavailable` result without projecting native details.

## Proof

| Check                               | Result                                      |
| ----------------------------------- | ------------------------------------------- |
| Focused handler/bridge/export tests | PASS — 27/27                                |
| Isolated handler TypeScript compile | PASS                                        |
| Fast Follow policy/spec checks      | PASS — 20/20                                |
| Behavior-contract checks            | PASS — 36/36                                |
| ProductSpec focused test            | PASS — 104/104                              |
| `pnpm run check`                    | PASS                                        |
| `pnpm run check:fast`               | PASS                                        |
| Desktop package typecheck           | BASELINE FAIL — unrelated lifecycle schema  |
| Targeted AssuranceSpec suite        | BASELINE FAIL — 189/190; environment digest |

The Desktop typecheck reproduces only the pre-existing lifecycle-schema
failures recorded by FF-D1-10 through FF-D1-13: existing
conversation/gateway fixtures omit thread `status`, and one live-subscription
fake omits `renameThread` and `setThreadStatus`. The new handler files produce
no reported type errors and pass isolated strict compilation. The targeted
AssuranceSpec suite reproduces the unrelated environment-profile digest
snapshot mismatch at 189/190. Neither baseline is owned, absorbed, weakened,
or claimed fixed here.

The implementation landed on `main` as
`73b8c9101f9a53f607ec042186b01d5918f00093`. The fetched remote tree exactly
matched the fully checked local implementation tree.

## Honest boundary and next packet

This receipt closes only the tested trusted-sender main-process handler and
registration lifecycle seam. It does not compose the handler in Desktop
`main.ts`, connect a renderer command, render disclosure/export pixels,
authorize broader audiences, prove an installed runtime-rendered journey, or
release/deploy anything. Those residuals, remaining adapters, owner acceptance,
and Day 1 completion remain unclaimed.
