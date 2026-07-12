# D1-A: native authoritative conversation continuation

- Issue: #8668
- Parent tracks: #8574 and #8597
- Status: closed; historical checked-in issue source
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md) R2/D1 and
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

The canonical client implementations of `chat.createThread`,
`chat.appendMessage`, and `chat.renameThread` now live in
`@openagentsinc/khala-sync-client`. The existing TanStack DB collection adapter
re-exports those implementations, so old and new owned consumers no longer
carry parallel client mutator code.

Both new native hosts register those mutators in the shared overlay and expose
one host-only conversation service after personal Sync reaches `live`. The
service subscribes exact thread scopes, lists only confirmed thread/message
rows, and returns public-safe refs, timestamps, server entity versions, actual
scope cursor/phase, and pending count. It omits owner identity and never returns
the raw store, session, overlay, transport, credential, or optimistic body.
Denial/sign-out/reconnect-before-live removes the capability.

## Convergence receipt

The deterministic two-host e2e uses the real Desktop `node:sqlite` adapter,
the real mobile Expo-SQLite adapter, the shared session/overlay/push protocol,
and a server-authoritative canonical chat fake:

1. Desktop creates one thread and first message.
2. Mobile confirms the same refs and appends one follow-up.
3. Desktop confirms that follow-up.
4. Both report identical message refs/bodies/versions and thread cursor `5` in
   `live` phase.
5. Both stores close, reopen, resubscribe, and reconstruct the same confirmed
   state without duplicate objects.

## Explicit residual

This is code/fixture proof, not a deployed Cloud SQL/live-account or physical-
device receipt. The Desktop renderer and mobile Home view are not yet wired to
the service. Provider-neutral runtime events, assistant replies/roles,
interrupt/resume, rich composer context, and terminal durable outcomes remain
the next D1 slices; `chat_message` is not extended locally to invent them.
