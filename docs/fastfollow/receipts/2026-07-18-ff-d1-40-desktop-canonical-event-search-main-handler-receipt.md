---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_40.desktop_canonical_event_search_main_handler.20260718"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "75b116befe78dda9bdcf1a4d378da7896f8cf793"
claim_revision: "014a7aa5f0697a3a8c8a4ff1c0144f6705b30c5e"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_canonical_event_search_main_handler"
observed_at: "2026-07-18T02:35:44Z"
---

# FF-D1-40 Desktop canonical accepted-event search main-handler receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-40 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-39 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, README/docs, live bug issues, known
baselines, Git configuration, dependencies, claims, and active worktrees were
reconciled before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, Full Auto,
mobile, T3, and teardown surfaces. No active worktree, accepted-plan claim, or
open reproducible bug owned the two new handler paths. AssuranceSpec remained
proof design rather than a provider-owned verdict.

## Implemented packet

- registers exactly FF-D1-38's fixed canonical accepted-event search channel
  and removes it exactly once when the handler resource closes;
- rejects closed, untrusted, throwing-trust, malformed, and broader requests
  before invoking the search dependency;
- passes only the normalized query and optional bounded limit decoded by
  FF-D1-38;
- decodes only exact bounded results and requires every available projection's
  query to match the normalized request; and
- preserves bounded acquisition refusals while collapsing thrown, malformed,
  query-mismatched, receipt/path-leaking, and native-detail outcomes to the
  existing path-free `transport_unavailable` result.

## Proof

| Check | Result |
| --- | --- |
| Focused handler/bridge/preload/acquisition/projection tests | PASS — 25/25 |
| Desktop typecheck | PASS |
| Agent Runtime Schema typecheck | PASS |
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

This receipt closes only the trusted-sender main-process handler seam. It does
not register the handler in Electron, compose private receipt-catalog and
artifact acquisition, edit `main.ts`, add renderer commands or pixels,
backfill historical sessions, produce supersession/reversion facts, authorize
named groups, or prove an installed runtime journey. Those residuals, owner
acceptance, and Day 1 completion remain unclaimed.

The exact tested implementation tree will replace `pending_remote_landing`
when it lands on `origin/main`, before the documentation-only claim release.
