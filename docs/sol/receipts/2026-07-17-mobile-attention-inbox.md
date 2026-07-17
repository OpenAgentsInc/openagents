# MOBILE-PARITY-03 confirmed attention inbox receipt

- Date: 2026-07-17
- Epic: #8950
- Leaves: #8951, #8952, #8953
- Destination: `packages/khala-sync`, `packages/khala-sync-client`,
  `packages/khala-sync-server`, `apps/openagents-mobile`
- Status: deterministic source receipt; no physical push or installed-device
  acceptance claim

## Landed boundary

Runtime attention is projected into the authenticated owner's personal Sync
scope as exact routing metadata only. Prompt text, choices, tool details, and
other interaction bodies remain in the thread-private projection. Pending and
terminal attention share one stable interaction identity and are written in
the same server transaction as the full interaction update.

The confirmed client inbox is live-authority only, bounded, stable-sorted, and
fail-closed for malformed rows, entity-reference mismatch, and owner-scope
mismatch. Mobile deep links and notification payloads contain only schema,
attention, thread, and turn refs; unknown keys are rejected.

The authenticated mobile host watches the personal inbox. Its Effect Native
Attention destination shows pending question/approval/plan-review metadata and
non-actionable terminal accounting. In-app selection and accepted native
return candidates dispatch the same controller intent, re-resolve the exact
tuple against the current confirmed inbox, and open the existing thread-scoped
interaction surface. Unknown, terminal, mismatched, stale, and invalid targets
do not navigate.

## Verification boundary

- shared runtime-attention contract and privacy tests pass;
- confirmed inbox and exact mobile target resolver tests pass;
- native deferred-delivery, Effect Native controller, and authenticated mobile
  Sync-host tests pass;
- all 29 mobile test files / 148 tests pass;
- mobile, Khala Sync, Khala Sync client, and Khala Sync server typechecks pass;
- the focused real-PostgreSQL server test was attempted earlier in the packet
  but PostgreSQL initialization was blocked by exhausted host System V shared
  memory identifiers before test execution.

This evidence does not prove Expo permission/token registration, APNs/FCM
delivery, server notification dispatch, cold installed-app return, or physical
iOS/Android acceptance. Those remain explicit follow-ons and must not be
inferred from the source-level queue and routing proof.
