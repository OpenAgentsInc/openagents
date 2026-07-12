# M2-A: visible mobile conversation over authoritative Sync

- Issue: #8671
- Parent track: #8597
- Program: #8566
- Status: closed; historical checked-in issue source
- Depends on: closed #8667 and #8668

## Landed boundary

The Expo host waits through the bounded native-session recovery/Sync bootstrap
window, then selects one visible conversation authority before mounting Home:

- live confirmed personal Sync → canonical account-linked conversation;
- signed-out/not-live/unavailable → the existing public-local Khala path.

An explicit sign-in/sign-out transition disposes the old Home program and
remounts a new one. Thread catalogs are never combined. The adapter sends only
confirmed public-safe thread/message projections to Effect Native; owner refs,
credentials, SQLite/session/transport objects, and raw rows remain host-only.

The existing transcript, drawer, and native composer now support confirmed
thread navigation plus canonical create/append. Stable `thread.mobile.*` and
`message.mobile.*` refs are generated before enqueue. A draft is visibly
`YOU · PENDING`, is replaced only when that exact ref appears confirmed, and is
removed with an honest pending-reconciliation error on timeout. Denial/sign-out
clears account-linked projection state.

## Evidence

- `apps/openagents-mobile/tests/mobile-conversation.test.ts` proves bounded
  selection, confirmed reconstruction, stable refs, and exact-ref timeout.
- `apps/openagents-mobile/tests/authoritative-home.test.ts` proves the existing
  Effect Native surface receives confirmed refs/versions, renders pending
  honestly, replaces only from confirmed state, and clears on denial.
- Contract: `openagents_mobile.chat.authoritative_sync_mode.v1`.

## Explicit residual

Canonical `chat_message` still represents owner messages and has no assistant
role. Provider-neutral runtime/reasoning/tool replies, repository/workroom
binding, physical-device live-account acceptance, and App Store deployment are
later leaves; this change does not infer or claim them.
