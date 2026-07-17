# MOBILE-PARITY-03E full mobile chat threads receipt

- Date: 2026-07-17
- Epic: #8961
- Leaves: #8962, #8963, #8964
- Destination: `packages/khala-sync-client`, `apps/openagents-mobile`
- Status: deterministic source receipt; no physical-device acceptance claim

## Landed boundary

The confirmed client now preserves image attachments and returns the newest
500 messages in deterministic order. The number is an explicit retained
window, not a claim that older canonical history does not exist. Attachment
bytes remain in the owner/thread scope and are not copied into the personal
thread catalog.

The mobile home owns one generation-fenced live lease for the ordinary
selected chat. Initial selection, chat replacement, new-chat creation, and
attention navigation bind through that owner. Coding-session entry, archive or
delete, sign-out, authority loss, and screen unmount close it. Late opens and
callbacks from a superseded generation cannot replace the current transcript.

The Effect Native conversation surface names the selected thread, reports the
retained-versus-total message boundary and retained runtime-event count, and
renders explicit loading, refreshing, unavailable, and empty-history copy.
Confirmed image attachments use the shared `Image` primitive with alt text and
a filename/size caption. User messages and runtime events retain their
confirmed chronological ordering.

## Verification boundary

- Khala Sync client suite: 29 passed files / 203 passed tests, 3 skipped;
- complete mobile test directory: 29 passed files / 154 passed tests;
- focused selected-thread and accessibility tests: 22 passed;
- Khala Sync client and mobile TypeScript checks pass.

This receipt does not prove physical iOS or Android behavior, unbounded or
paginated history before the 500-message retained window, non-image file
attachments, share-sheet/quick actions, physical push delivery, or the phase 4
Files/repository workbench. Those remain separate acceptance rungs.
