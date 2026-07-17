# MOBILE-PARITY-03D confirmed thread lifecycle receipt

- Date: 2026-07-17
- Epic: #8954
- Leaves: #8955, #8956, #8957
- Destination: `packages/khala-sync`, `packages/khala-sync-client`,
  `packages/khala-sync-server`, `apps/openagents-mobile`
- Status: deterministic source receipt; no physical-device acceptance claim

## Landed boundary

The owner-private chat authority now models active, archived, and terminal
deleted threads. Legal transitions use an exact status and `updatedAt`
compare-and-set boundary and write the business row plus personal/thread
metadata changelog post-images in one transaction. Deleted rows are private
metadata tombstones; message bodies remain confined to the thread scope.

The confirmed conversation client defaults to active rows and can explicitly
read archived or deleted status for bounded reconciliation. Mobile sends the
exact confirmed baseline and reports success only after a newer matching
personal-scope post-image is observed. It does not turn optimistic overlay
admission into completion.

The Effect Native drawer provides active-chat rename/archive/delete management,
an archived section with restore/delete, visible pending/conflict copy, and an
explicit second confirmation before terminal deletion. Archive/delete removes
the selected thread and composer from active navigation after confirmation.

## Verification boundary

- shared/client lifecycle tests: 20 passed;
- Khala Sync client suite: 29 passed files / 202 passed tests, 3 skipped;
- focused mobile adapter/UI tests: 30 passed;
- complete mobile test directory: 29 passed files / 151 passed tests;
- Khala Sync, client, server, and mobile TypeScript checks pass;
- the checked-in real-PostgreSQL lifecycle suite was attempted, but local
  PostgreSQL initialization was blocked by exhausted host System V shared-memory
  identifiers before execution. No unrelated process was terminated to mask
  that host limitation.

This receipt does not prove physical iOS/Android behavior, share-sheet or quick
actions, push-token registration, APNs/FCM delivery, cold-return routing, or the
phase 4 Files/repository workbench. Those remain separate acceptance rungs.
