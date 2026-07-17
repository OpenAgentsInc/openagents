---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_35.desktop_canonical_event_search_artifact_source.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "3e79a76d718304bb58cef184da97d6587167cca6"
claim_revision: "8067dda00f7ed42f5cea27f3b9160ea3e3bcf292"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_canonical_event_search_artifact_acquisition"
observed_at: "2026-07-17T23:25:18Z"
---

# FF-D1-35 Desktop canonical-event search artifact-source receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-35 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-34 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, open issues, known baselines, Git
configuration, dependencies, claims, and active worktrees were reconciled
before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, mobile, T3,
Full Auto, and teardown surfaces. Authoritative supersession/reversion
producers and named-group membership remain absent. No open reproducible bug,
claim, or worktree owned the two new Desktop adapter paths. AssuranceSpec
remained proof design rather than a provider-owned verdict.

## Implemented packet

- added one Desktop adapter that accepts only bounded canonical owner-only
  `export_created` receipts with an exact artifact-ref/SHA-256 binding;
- loads each unique artifact through the injected private-store seam and
  re-verifies byte bounds, SHA-256, fatal UTF-8, JSON/schema, and receipt-to-
  artifact intent/thread/format/audience identity before search;
- dedupes exact receipt replay and shared-artifact receipts to one load while
  rejecting conflicting receipt or artifact identity;
- delegates only verified canonical bundles to FF-D1-34 and returns only its
  bounded projection or one redacted unavailable reason, never bytes, paths,
  receipt bodies, or store authority;
- performs no receipt validation or private artifact read for a blank query;
  and
- adds no new persistence, transcript, acceptance, visibility, transport,
  provider, host, preload, renderer, or release authority.

## Proof

| Check | Result |
| --- | --- |
| Focused acquisition/search/artifact-store/compiler/disclosure/authority tests | PASS — 29/29 |
| Desktop and Agent Runtime Schema typechecks | PASS |
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

This receipt closes only verified acquisition from already-persisted canonical
export receipts into the shared search projection. It does not persist a
receipt catalog, ingest broader historical sessions, wire the Desktop host or
UI, render pixels, produce supersession/reversion facts, authorize named
groups, register acquisition in `main.ts`, or prove an installed runtime
journey. Those residuals, owner acceptance, and Day 1 completion remain
unclaimed.

The exact tested implementation tree will replace `pending_remote_landing`
when it lands on `origin/main`, before the documentation-only claim release.
