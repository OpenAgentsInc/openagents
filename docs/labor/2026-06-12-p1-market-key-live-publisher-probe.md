# P1 Market-Key Live Publisher Probe

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-12

Issue scope: #4777, feeding #4781, #4782, #4783, and #4786.

## Claim

The operator-controlled market-key signing blocker for the Forum
work-request bridge is cleared. The Worker now has a dedicated
`FORUM_WORK_REQUEST_MARKET_SECRET_KEY` secret configured, a deployment carrying
that secret is live, and a ref-only no-spend work request published a
retrievable kind-5934 relay event.

This does not complete the first live negotiated labor job. No independent
provider quote, requester acceptance, escrow reserve, provider execution,
validator acceptance, release, or settlement evidence exists for this probe.

## Production Action

- Generated a dedicated 64-hex Nostr market signing key and stored it only in
  the ignored local workspace secret backup.
- Uploaded it to the `openagents-autopilot` Worker as
  `FORUM_WORK_REQUEST_MARKET_SECRET_KEY` through Wrangler stdin.
- Deployed `openagents-autopilot` Worker version
  `f87df619-8678-40ad-872d-5ae35e953a80`.

## No-Spend Probe

- route: `POST /api/forum/work-requests`
- auth: registered-agent bearer
- idempotency key: `p1-4777-market-key-probe-20260612T034322Z`
- status: `201`
- work request id: `f3da4627-246c-444d-885a-0f779964a779`
- work request state: `open`
- topic slug: `backlog-issue-4773-a1-api-parity-matrix-slice`
- relay ref: `relay.public.market.0a2b94b3a5372b3a5cf8cbeb1325da9b`
- relay URL: `wss://openagents-market-relay.openagents.workers.dev`
- job event id:
  `d480e175984bb3afafa92162438c9b56a1399b5631f9f88110fea11673520327`
- job event kind: `5934`

Relay lookup:

- queried the owned relay by event id over WebSocket
- result: found
- returned event kind: `5934`
- tag count: `15`

Post-publish status:

- `GET /api/forum/work-requests` listed one open request.
- `GET /api/forum/work-requests/f3da4627-246c-444d-885a-0f779964a779`
  showed state `open`.
- Eight 15-second polls showed `offerCount: 0` and no accepted quote.

## Remaining Live Blockers

- An independent contributor Pylon must quote the work request.
- The requester must accept exactly one quote with escrow reserve evidence.
- The contributor must execute and deliver output-only refs.
- A validator must accept the result before release.
- Escrow release, payout ladder, and settlement visibility receipts must be
  attached before any live labor-market, P5 backlog-faucet, P6 provider-mode,
  P7 Lane C fanout, or parent-MVP claim cites this job.
