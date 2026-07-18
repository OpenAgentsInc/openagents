# Sarah terminal-history harness implementation

## Outcome

Sarah's hosted runtime now has a bounded self-improvement loop over her own
authenticated owner-private conversation. It implements the safe portion of
the Full Auto MemoHarness contract needed for Sarah to improve conversational
behavior without becoming a self-releasing or authority-amplifying agent.

## Runtime flow

1. Before inference, the host resolves one released six-dimension Sarah
   `HarnessPolicyBundle` and persists an immutable turn binding.
2. The `sarah_harness_review_history` tool reads only terminal turns from the
   owner's exact Sarah thread. The current turn cannot enter the snapshot.
3. An Effect-owned compiler creates append-only private experience records
   containing source refs/digests and bounded outcome facts, not duplicate raw
   transcript bodies.
4. The optimizer receives the training partition and can propose only bounded
   conversational instructions plus a default word ceiling.
5. A separate evaluator receives a held-out partition. The Blueprint gate then
   checks quality, regression, privacy, safety, exact dimension compatibility,
   and deterministic provenance/secret fencing.
6. Only the gate can compare-and-swap a released candidate into the active
   binding. It affects the next turn; the current turn stays frozen.

## Authority boundary

Sarah can inspect the active bundle and request a review. She cannot select the
held-out result, change the candidate schema, evaluate, release, activate,
alter tools/providers/workspaces/budgets/approvals/guardrails, admit assurance,
or expand her authority. Full Auto's broader FA-AC-69–76 Desktop run lifecycle
remains a separate implementation program; this change neither claims nor
silently simulates Desktop run-start adaptation.

## Evidence

- `packages/sarah/src/index.ts`: typed six-dimension policy and frozen prompt.
- `apps/openagents.com/workers/api/src/sarah-harness-service.ts`: private
  compiler, optimizer/evaluator split, release gate, and turn binding.
- `packages/khala-sync-server/migrations/0079_sarah_harness_learning.sql`:
  private immutable storage and activation receipts.
- Unit and contract tests cover content addressing, authority-preserving
  schema bounds, privacy rejection, tool behavior, and prompt binding.
- Production proof must record a real terminal snapshot, released or rejected
  held-out evaluation, active bundle, and a later turn binding before this
  document is treated as deployment evidence.
