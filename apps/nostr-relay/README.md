# OpenAgents Nostr Relay POC

Issue: <https://github.com/OpenAgentsInc/openagents/issues/4621>

This app is the first scoped OpenAgents Nostr relay proof of concept. It wraps
`nostr-effect`'s Cloudflare Durable Object relay backend in an isolated Worker
app, using a SQLite-backed Durable Object named `NostrRelayDO`.

This is not the production OpenAgents relay. It is only the deploy and
handshake proof:

- HTTP `GET /` with `Accept: application/nostr+json` returns NIP-11 relay
  metadata.
- WebSocket `REQ` returns relay messages from the Durable Object.
- `GET /health` returns an OpenAgents POC health document.

## Commands

```sh
bun install
bun run --cwd apps/nostr-relay typecheck
bun run --cwd apps/nostr-relay dev
bun run --cwd apps/nostr-relay smoke ws://127.0.0.1:8787
bun run --cwd apps/nostr-relay deploy
bun run --cwd apps/nostr-relay smoke <deployed-wss-url>
```

## Current Deployment

Deployment URL: <https://openagents-nostr-relay-poc.openagents.workers.dev>

Cloudflare Worker version from the first deploy:
`07fa52be-c47d-4738-83c9-74e1a92dcbe2`

Verified on 2026-06-09:

```sh
bun run --cwd apps/nostr-relay smoke https://openagents-nostr-relay-poc.openagents.workers.dev
```

Result:

```text
NIP-11 GET https://openagents-nostr-relay-poc.openagents.workers.dev/
WebSocket connect wss://openagents-nostr-relay-poc.openagents.workers.dev/
Handshake messages: [["EOSE","openagents-poc-..."]]
Nostr relay smoke passed
```

Health check:

```sh
curl https://openagents-nostr-relay-poc.openagents.workers.dev/health
```

## Boundaries

Out of scope for this POC:

- production relay moderation or rate limits;
- orange-check endpoint behavior;
- Forum federation or write-back;
- payment, payout, assignment, or settlement authority;
- full current upstream NIP parity.
