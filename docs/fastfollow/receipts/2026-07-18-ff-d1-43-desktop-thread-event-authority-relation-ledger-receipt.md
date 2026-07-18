---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_43.desktop_thread_event_authority_relation_ledger.20260718"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "de18ab353fe45aa726080deb2216c67a2e521fab"
claim_revision: "c0f52d55c6a5b7f573fabb120f3a3ba8eedf36cc"
implementation_revision: "8588604915502e5a1689de71dba2f6c889a7cb5c"
proof_rung: "desktop_terminal_thread_event_authority_relation_ledger"
observed_at: "2026-07-18T03:34:58Z"
---

# FF-D1-43 Desktop terminal thread-event authority-relation ledger receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-43 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-42 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, README/docs, live bug issues, known
baselines, Git configuration, dependencies, claims, and active worktrees were
reconciled before mutation.

Active work owned Desktop `main.ts` and renderer surfaces. Historical exports
without original receipt identity cannot be reconstructed honestly. No active
worktree, accepted-plan claim, or open reproducible bug owned the two new
ledger paths. AssuranceSpec remained proof design rather than a provider-owned
verdict.

## Implemented packet

- admits only exact ref-only `superseded` and `reverted` relations decoded by
  the shared authority-v1 schema;
- rejects accepted facts because the confirmed timeline remains their source;
- stores at most one terminal relation per exact thread/event identity in a
  bounded owner-private atomic ledger;
- keeps exact replay idempotent while rejecting conflicts, extra fields,
  invalid transitions, capacity overflow, and corrupt persisted state; and
- lists deterministic exact terminal relations for one validated thread ref
  without projecting bodies, summaries, prompts, providers, paths, or native
  errors.

## Proof

| Check | Result |
| --- | --- |
| Focused ledger/shared-authority/export/search tests | PASS — 36/36 |
| Desktop typecheck | PASS |
| Agent Runtime Schema typecheck | PASS |
| Fast Follow root coverage | PASS — 7/7 |
| Fast Follow package checks | PASS — 13/13 |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 107/107 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS |
| `pnpm run check:fast` | PASS |
| Targeted AssuranceSpec compiler | BASELINE FAIL — 5/6; environment digest snapshot only |

AssuranceSpec reproduced only the previously recorded environment-profile
digest drift (`e46c...` expected, `14cb...` observed). This packet did not
absorb or weaken that condition and did not mutate shared Git configuration.

## Honest boundary and next packet

This receipt closes only private persistence for terminal authority facts
already observed elsewhere. It does not observe, infer, authorize, or create a
supersession/reversion fact and does not claim a producer exists. Real
observation and producer composition, merging terminal facts with confirmed
accepted timelines, historical backfill, `main.ts`, renderer pixels,
named-group authority, and installed runtime evidence remain unclaimed.

The exact tested implementation tree landed on `origin/main` at
`8588604915502e5a1689de71dba2f6c889a7cb5c` before the documentation-only
claim release.
