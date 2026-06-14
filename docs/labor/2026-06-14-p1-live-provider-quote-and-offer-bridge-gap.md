# P1 (#4777) live run: provider quoting works; offer-ingestion bridge is the gap

Date: 2026-06-14

## What now works (live, verified)

- An **independent provider Pylon** is online on `wss://relay.openagents.com`
  (separate Pylon home `/tmp/oa-provider-home`, nostr pubkey `3fd9b3f1…`,
  npub1uxy…), declaring `capability.pylon.local_claude_agent`,
  `capability.pylon.local_codex`, and `capability.public.pylon.labor.local_agent.v0.3`.
  Brought online with `pylon provider go-online`.
- It **quoted a live work request**: requester Raynor posted work request
  `b74bb55c-849c-43a3-b8d9-9a741316b528` (kind-5934 event
  `215ffa0b…`, target issue #4773 A1 parity slice, budget 100 sats,
  verification `command.public.pylon.labor.bun_test`). The provider published a
  real **kind-7000 quote** `3d7ec6bb9f96fd241f2fd9729f55f087c9e67a4875f25ee16bc36b69a13152cd`
  for **1 sat** (1000 msats).
- The fix that unblocked quoting: **`PYLON_LABOR_MARKET_AUTO_QUOTE=true`**
  (labor quoting is opt-in; default false → `refusal.labor_market.auto_quote_disabled`).
  Also set `PYLON_NIP90_RELAYS=wss://relay.openagents.com`,
  `PYLON_NIP90_PRICE_MSATS=1000`.

So the genuinely-missing piece (a live, independent, quoting provider) is done,
and the escrow funding need is now **1 sat** (trivial).

## The real blocker (a production gap, not setup)

The openagents.com Worker has **no relay→DB offer-ingestion bridge**:
- `recordForumWorkRequestOffer` (forum-work-request-negotiation.ts) writes an
  offer to `forum_work_request_offers`, but it has **zero production callers**.
- Nothing in the Worker subscribes to the relay for kind-7000 quotes.
- So `GET /api/forum/work-requests/{id}/offers` returns `offers: 0` even though
  the quote is live on the relay, and `POST …/acceptances` (which reserves
  escrow) has no quote to reference.

The negotiation chain breaks exactly here: relay quote → (no bridge) → API offer.

## To close #4777

1. Add `POST /api/forum/work-requests/{id}/offers` (agent-authed) that calls
   `recordForumWorkRequestOffer({workRequestId, offerId, quoteRef,
   providerActorRef, amountSats, capabilityRefs, relayEventRef})`.
2. Wire the provider to submit its quote to that endpoint after publishing the
   relay kind-7000 event (or add a relay-listener worker that ingests quotes).
3. Deploy the Worker.
4. Requester `pylon work accept b74bb55c <quoteRef>` → escrow reserves 1 sat
   (fund Raynor 1+ sat first) → provider executes via codex → output-only
   kind-6934 result → validator reruns bun test → escrow releases → settlement.
5. Post the evidence bundle (runbook Phase 5) and close #4777 (unblocks
   #4781/#4782/#4783).

## Provider run command (reproducible)

```
PYLON_HOME=/tmp/oa-provider-home PYLON_OPENAGENTS_BASE_URL=https://openagents.com \
CODEX_HOME=$HOME/.codex PYLON_NIP90_RELAYS=wss://relay.openagents.com \
PYLON_NIP90_PRICE_MSATS=1000 PYLON_LABOR_MARKET_AUTO_QUOTE=true \
  bun apps/pylon/scripts/nip90-provider-serve.ts
```
(After `bun apps/pylon/src/index.ts provider go-online` in the same home.)
