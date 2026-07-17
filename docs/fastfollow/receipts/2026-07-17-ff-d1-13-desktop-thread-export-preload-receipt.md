---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_13.desktop_thread_export_preload.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "ffefa153e866b5a0fb1af5b6f2411edb3bb85a27"
claim_revision: "a793ad45660d4f099fd7406422d8f0726c5c071f"
proof_rung: "sandboxed_ref_only_canonical_export_preload_boundary"
observed_at: "2026-07-17T15:52:16Z"
---

# FF-D1-13 Desktop thread-export preload receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-13 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-12 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The open Fast Follow parser issue is a separate reproducible bug and does not
supersede this owner-ordered packet. Active work continued to own Desktop
`main.ts`, history, shell, renderer, update, and release surfaces. This packet
therefore changed only the new bridge contract/test and the unclaimed sandboxed
preload allowlist. It advances Fast Follow ProductSpec `FF-AC-04`, `FF-AC-06`,
and `FF-AC-12`; AssuranceSpec inventory remains proposed proof design rather
than a provider-owned verdict.

## Implemented packet

- added one fixed `openagents:thread-export:write` channel contract for an
  exact ref-only owner canonical-export receipt;
- rejected malformed, raw-content-bearing, broader-audience, other-format,
  pending, failed, visibility, or envelope-smuggled requests before invocation;
- exposed one decoded `threadExports.write` method through the sandboxed preload
  rather than raw `ipcRenderer`, a caller-selected channel, filesystem, process,
  provider, destination-path, or artifact-byte authority;
- decoded only bounded cancelled, written, and rejected results with exact keys
  so a leaked path, bytes, raw error, or unknown reason cannot reach renderer
  code; and
- collapsed native invocation failures and malformed replies to the typed
  `transport_unavailable` reason without projecting native details.

## Proof

| Check                                  | Result                                           |
| -------------------------------------- | ------------------------------------------------ |
| Focused bridge/command/store tests     | PASS — 22/22                                     |
| Isolated bridge TypeScript compile     | PASS                                             |
| Desktop production build/preload proof | PASS — fixed channel and bridge present          |
| Fast Follow policy/spec checks         | PASS — 20/20                                     |
| Behavior-contract checks               | PASS — 36/36                                     |
| ProductSpec focused test               | PASS — 104/104                                   |
| `pnpm run check`                       | PASS                                             |
| `pnpm run check:fast`                  | PASS                                             |
| Desktop package typecheck              | BASELINE FAIL — unrelated lifecycle schema drift |
| Targeted AssuranceSpec suite           | BASELINE FAIL — 189/190; environment digest      |

The Desktop typecheck reproduces only the pre-existing lifecycle-schema
failures recorded by FF-D1-10 through FF-D1-12: existing conversation/gateway
fixtures omit thread `status`, and one live-subscription fake omits
`renameThread` and `setThreadStatus`. The new bridge and preload integration
produce no reported type errors, pass isolated strict compilation, and build
into the production preload. The targeted AssuranceSpec suite reproduces the
unrelated environment-profile digest snapshot mismatch at 189/190. Neither
baseline is owned, absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only the sandboxed preload request/result boundary. It does
not register the main-process handler, connect a renderer command, render
disclosure/export pixels, authorize broader audiences, prove an installed
runtime-rendered journey, or release/deploy anything. Those residuals,
remaining adapters, owner acceptance, and Day 1 completion remain unclaimed.
