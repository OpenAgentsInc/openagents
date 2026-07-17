---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_15.desktop_thread_export_create_preload.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "f64af79791424f1819655cdfa63e40e7297689b1"
claim_revision: "4ae0516adc2d02af5909d85d7fc175dc6469383d"
proof_rung: "sandboxed_owner_only_canonical_export_creation_boundary"
observed_at: "2026-07-17T16:25:12Z"
---

# FF-D1-15 Desktop canonical-export creation preload receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-15 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-14 released. Current `origin/main`, prior Day 1
receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The open Fast Follow parser issue is a separate reproducible bug and does not
supersede this owner-ordered packet. No open issue or worktree owns thread
export creation, `preload.cts`, or the two new bridge paths. Active work
continues to own Desktop `main.ts`, history, shell, renderer, update, and
release surfaces. This packet advances Fast Follow ProductSpec `FF-AC-04`,
`FF-AC-06`, and `FF-AC-12`; AssuranceSpec inventory remains proposed proof
design rather than a provider-owned verdict.

## Implemented packet

- added one fixed `openagents:thread-export:create` channel for an exact
  owner-only canonical export intent;
- rejected malformed, raw-content-bearing, broader-audience, other-format,
  visibility, or envelope-smuggled requests before IPC invocation;
- exposed one decoded `threadExports.create` method through sandboxed preload
  rather than raw `ipcRenderer`, caller-supplied events, authority relations,
  receipt metadata, artifact bytes, destination paths, filesystem, process, or
  provider authority;
- admitted only exact stored/unchanged results whose decoded export receipt
  matches the requested intent, idempotency key, thread, format, and audience;
  and
- preserved bounded rejection reasons while collapsing native or malformed
  replies to `command_unavailable` without projecting native details.

## Proof

| Check                                  | Result                                           |
| -------------------------------------- | ------------------------------------------------ |
| Focused create/bridge/command tests    | PASS — 21/21                                     |
| Isolated production-contract compile   | PASS                                             |
| Desktop production build/preload proof | PASS — fixed create channel and method present   |
| Fast Follow policy/spec checks         | PASS — 20/20                                     |
| Behavior-contract checks               | PASS — 36/36                                     |
| ProductSpec focused test               | PASS — 104/104                                   |
| `pnpm run check`                       | PASS                                             |
| `pnpm run check:fast`                  | PASS                                             |
| Desktop package typecheck              | BASELINE FAIL — unrelated lifecycle schema drift |
| Targeted AssuranceSpec suite           | BASELINE FAIL — 189/190; environment digest      |

The Desktop typecheck reproduces only the pre-existing lifecycle-schema
failures recorded by FF-D1-10 through FF-D1-14: existing conversation/gateway
fixtures omit thread `status`, and one live-subscription fake omits
`renameThread` and `setThreadStatus`. The new bridge and preload integration
produce no reported type errors, pass isolated strict compilation, and build
into the production preload. The targeted AssuranceSpec suite reproduces the
unrelated environment-profile digest snapshot mismatch at 189/190. Neither
baseline is owned, absorbed, weakened, or claimed fixed here.

## Honest boundary and next packet

This receipt closes only the sandboxed creation request/result boundary. It
does not register the main-process creation handler, compose either export
handler in Desktop `main.ts`, connect a renderer create-then-write command,
render disclosure/export pixels, authorize broader audiences, prove an
installed runtime-rendered journey, or release/deploy anything. Those
residuals, remaining adapters, owner acceptance, and Day 1 completion remain
unclaimed.
