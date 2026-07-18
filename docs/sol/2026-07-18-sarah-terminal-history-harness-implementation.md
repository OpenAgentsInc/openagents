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

## Production proof

The implementation commits `94a7375be8`, `a06bb2174c`, and `9c58332a3c` are
on `main`. Migration `0079_sarah_harness_learning.sql` is applied in the
production Cloud SQL database. Cloud Run revision
`openagents-monolith-00190-nwq` serves 100% of production traffic and passed
the health, public `/sarah` tombstone, and real-browser portal smokes.

On 2026-07-18, the production proof ran against the verified owner's real
Sarah thread and produced these bounded results without projecting transcript
content or owner identifiers:

- model: `gemma-4-31b-it`;
- terminal experiences: 11 total, with 9 training and 2 held out;
- partitions: disjoint, and every experience remained `owner_private`;
- independent evaluation: approved, with quality, regression, privacy, and
  safety scores of `1.0`;
- released review: `review.sarah.harness.e9c78553bf24c1bf88777c2b`;
- activated bundle: `harness.bundle.sarah.5d5b7e52cd22a6e6dabad174`;
- bundle digest:
  `sha256:5d5b7e52cd22a6e6dabad174a408b59405690d5a9f65b67402da3f6e36f231e6`;
- the active bundle matched the released candidate, and a later proof turn
  bound that exact ref and digest before inference.

The production identity audit found two historical Sarah histories. The one
current active identity matching the admitted owner email retained authority;
the other historical receipt was refused. Hosted Sarah checks current active,
non-deleted owner identity on every dispatch, so neither an opaque
Sarah-shaped thread ref nor an old authority receipt can grant access by
itself. Raw authority receipts, thread refs, owner ids, and transcript content
remain private and are intentionally absent from this evidence document.
