---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_39.desktop_canonical_event_search_preload.20260718"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "afe51fd0e5396a5f13bcbd83f3cabe47b48928a5"
claim_revision: "d91251159ce248d6e13b65cd802c4a01d0b3f1c3"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_canonical_event_search_preload"
observed_at: "2026-07-18T02:15:15Z"
---

# FF-D1-39 Desktop canonical accepted-event search preload receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-39 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-38 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, README/docs, live bug issues, known
baselines, Git configuration, dependencies, claims, and active worktrees were
reconciled before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, Full Auto,
mobile, T3, and teardown surfaces. No active worktree, accepted-plan claim, or
open reproducible bug owned the sandboxed preload or FF-D1-38 bridge-test scope.
AssuranceSpec remained proof design rather than a provider-owned verdict.

## Implemented packet

- imports only FF-D1-38's already-decoded search invoker into the sandboxed
  preload;
- exposes one `openagentsDesktop.threadSearch.query(value)` method;
- delegates invocation through FF-D1-38's fixed
  `openagents:thread-event-search:query` channel and exact request/result
  decoders;
- exposes no raw `ipcRenderer`, selectable channel, subscription, receipt,
  artifact byte, path, event body, or native error surface; and
- proves the production preload bundle builds with the method present in the
  compiled `dist/preload.cjs` artifact.

## Proof

| Check | Result |
| --- | --- |
| Focused bridge/preload and accepted-event projection tests | PASS — 13/13 |
| Desktop typecheck | PASS |
| Agent Runtime Schema typecheck | PASS |
| Desktop production build and compiled preload inspection | PASS |
| Fast Follow root coverage | PASS — 7/7 |
| Fast Follow package checks | PASS — 13/13 plus typecheck/distribution |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 107/107 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS |
| `pnpm run check:fast` | PASS |
| Targeted AssuranceSpec suite | BASELINE FAIL — 189/190; environment digest |

AssuranceSpec reproduced only the previously recorded environment-profile
digest snapshot drift. This packet did not absorb or weaken that condition and
did not mutate shared Git configuration.

## Honest boundary and next packet

This receipt closes only sandboxed preload exposure of the existing bounded
search contract. It does not register a main-process handler, compose artifact
acquisition into `main.ts`, add renderer commands or pixels, backfill historical
sessions, produce supersession/reversion facts, authorize named groups, or
prove an installed runtime journey. Those residuals, owner acceptance, and Day
1 completion remain unclaimed.

The exact tested implementation tree will replace `pending_remote_landing`
when it lands on `origin/main`, before the documentation-only claim release.
