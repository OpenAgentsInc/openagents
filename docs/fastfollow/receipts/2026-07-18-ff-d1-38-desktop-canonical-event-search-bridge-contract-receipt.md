---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_38.desktop_canonical_event_search_bridge_contract.20260718"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "341164e633bcfeb0098c16e80ad7effbae948fc6"
claim_revision: "73035dd3f885703999eeeb8425edcedc12b35fdc"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_canonical_event_search_bridge_contract"
observed_at: "2026-07-18T01:57:56Z"
---

# FF-D1-38 Desktop canonical accepted-event search bridge contract receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-38 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-37 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, README/docs, open issues, known baselines,
Git configuration, dependencies, claims, and active worktrees were reconciled
before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, Full Auto,
mobile, T3, and teardown surfaces. The proposal's two bridge files were
unclaimed. Historical artifact files do not carry the original canonical
receipt identity needed for honest backfill, so this packet did not infer or
manufacture it. AssuranceSpec remained proof design rather than a provider-
owned verdict.

## Implemented packet

- admits only exact requests containing a string query and optional integer
  limit from 1 through 100, normalizing bounded whitespace only after the
  search route is selected;
- invokes exactly `openagents:thread-event-search:query` and rejects malformed
  input before any host invocation;
- schema-decodes only the FF-D1-34 bounded projection or a closed redacted set
  of acquisition/transport reasons;
- enforces exact nested projection and authority keys, bounded refs, counts,
  snippets, results, relation refs, unique identities, and consistent result
  totals; and
- rejects receipts, artifact bytes, paths, event bodies, native errors, extra
  fields, self-supersession, invalid reversion, or malformed replies, collapsing
  transport failure to one path-free `transport_unavailable` outcome.

The implementation follows the repository's Effect guidance by using Effect
Schema at the untrusted result boundary, keeping construction pure and bounded,
and retaining deterministic parsing only inside the already-selected search
command.

## Proof

| Check | Result |
| --- | --- |
| Focused bridge and accepted-event projection tests | PASS — 12/12 |
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

This receipt closes only the typed IPC contract for requesting the existing
bounded accepted-event search projection. It does not expose the method from
preload, register a main-process handler, compose `main.ts`, add renderer UI or
pixels, backfill historical sessions, produce supersession/reversion facts,
authorize named groups, or prove an installed runtime journey. Those residuals,
owner acceptance, and Day 1 completion remain unclaimed.

The exact tested implementation tree will replace `pending_remote_landing`
when it lands on `origin/main`, before the documentation-only claim release.
