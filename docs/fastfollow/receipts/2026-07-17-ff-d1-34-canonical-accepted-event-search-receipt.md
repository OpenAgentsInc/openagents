---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_34.canonical_accepted_event_search.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "116c77903ea2db9c5903c125a7f1925d54923391"
claim_revision: "f41e349278de06e64b7127f0272bd9efddf1487a"
implementation_revision: "ba7ac6e82d3b465815e9ff1957426ee0c2c89429"
proof_rung: "canonical_accepted_event_search_projection"
observed_at: "2026-07-17T23:08:51Z"
---

# FF-D1-34 canonical accepted-event search receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-34 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-33 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, open issues, known baselines, Git
configuration, dependencies, claims, and active worktrees were reconciled
before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, mobile, T3,
Full Auto, and teardown surfaces. Authoritative supersession/reversion
producers and named-group membership remain absent. No open reproducible bug,
claim, or worktree owned the new shared search paths. AssuranceSpec remained
proof design rather than a provider-owned verdict.

## Implemented packet

- added `openagents.thread_event_search_projection.v1`, a pure rebuildable
  owner-local projection over existing canonical event bundles;
- returns the exact original thread/event/sequence and preserves the bundle's
  accepted, superseded, or reverted state, including exact replacement,
  revert, and restored refs;
- performs deterministic bounded string-leaf filtering only after the search
  route is selected and returns no event body or synthesized replacement;
- rejects malformed artifacts, duplicate thread/event identity, invalid
  relation identity, self-supersession, invalid reversion, invalid query/limit
  bounds, and oversized artifact sets;
- exposes index and result truncation independently and skips all event-byte
  scanning for a blank query; and
- adds no transcript, acceptance, persistence, transport, disclosure,
  mutation, provider, host, renderer, or release authority.

## Proof

| Check | Result |
| --- | --- |
| Focused search/artifact/authority tests | PASS — 17/17 |
| Agent Runtime Schema typecheck | PASS |
| Fast Follow root coverage | PASS — 7/7 |
| Fast Follow package checks | PASS — 13/13 plus typecheck/distribution |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 107/107 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS after quiescent rerun |
| `pnpm run check:fast` | PASS |
| Targeted AssuranceSpec suite | BASELINE FAIL — 189/190; environment digest |

The first repository lint attempt observed a deleted `test-tmp` fixture path
while the earlier broad test invocation was cleaning its own temporary files.
The unchanged gate passed from the quiescent isolated worktree. AssuranceSpec
reproduced only the previously recorded environment-profile digest snapshot
drift. This packet did not absorb or weaken either condition and did not mutate
shared Git configuration.

## Honest boundary and next packet

This receipt closes only the shared canonical accepted-event search
projection. It does not acquire or persist a historical index, wire Desktop,
render pixels, produce supersession/reversion facts, authorize named groups,
register the canonical export host in `main.ts`, or prove an installed runtime
journey. Those residuals, owner acceptance, and Day 1 completion remain
unclaimed.

The exact tested implementation tree landed on `origin/main` at
`ba7ac6e82d3b465815e9ff1957426ee0c2c89429` before this documentation-only
claim release.
