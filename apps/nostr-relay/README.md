# OpenAgents Scoped Market Relay

Issue: <https://github.com/OpenAgentsInc/openagents/issues/4636>

This app runs the scoped OpenAgents market-event Nostr relay. It wraps the
`nostr-effect` Cloudflare Durable Object relay backend and adds an
OpenAgents-specific transport policy before messages reach the shared relay
handler.

This relay is event transport only. It grants no payment, identity,
moderation, assignment, payout, or settlement authority. Market acceptance,
payment, work execution, and public promise transitions must be proven by their
own receipt-backed systems.

## Allowed Events

The relay accepts only the market event kinds needed for the five Bitcoin
revenue-stream rails:

- NIP-90 job requests: `5000` through `5999`
- NIP-90 job results: `6000` through `6999`
- NIP-90 job feedback: `7000`
- NIP-DS listing and offer kinds: `30404`, `30406`
- NIP-89 handler information: `31989`, `31990`

All other event kinds are rejected before storage or broadcast.

## Limits

Publish limits are per pubkey per Durable Object instance:

- `24` event publishes per `60` seconds
- `64 KiB` maximum event content

REQ limits:

- `4` filters per request
- `100` maximum `limit`
- `64` ids per filter
- `16` authors per filter
- `16` kinds per filter
- `32` values per supported tag filter

REQ filters that name disallowed kinds are closed without creating a
subscription.

## Retention

Stored market events are retained for `30` days. NIP-89 handler information is
retained for `180` days because handler advertisements change less often and
are useful for discovery. Retention cleanup runs opportunistically on health
checks and valid WebSocket messages.

## Routes

- HTTP `GET /` with `Accept: application/nostr+json` returns NIP-11 relay
  metadata from `nostr-effect`.
- WebSocket `/` accepts Nostr relay messages.
- `GET /health` and `GET /metrics` return relay policy, retention status, and
  stored event counts by kind/range.

## Commands

```sh
bun install
bun run --cwd apps/nostr-relay typecheck
bun run --cwd apps/nostr-relay test
bun run --cwd apps/nostr-relay dev
bun run --cwd apps/nostr-relay smoke ws://127.0.0.1:8787
bun run --cwd apps/nostr-relay deploy
bun run --cwd apps/nostr-relay smoke https://openagents-market-relay.openagents.workers.dev
```

## Current Deployment

Production Workers.dev URL:
<https://openagents-market-relay.openagents.workers.dev>

Custom production hostname is intentionally not changed by this repo alone.
Coordinate DNS/hostname changes on issue #4636 before adding a custom route.
