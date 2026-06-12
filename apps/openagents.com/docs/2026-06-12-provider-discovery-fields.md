# Provider Discovery Fields in /api/pylons (#4864)

Date: 2026-06-12
Status: implemented
Owner: openagents.com worker + Pylon presence client

## What changed

`GET /api/pylons` (and the `/api/pylons/{pylonRef}` detail projection) now
carries four public-safe provider discovery fields on registered provider
Pylons:

- `providerNostrPubkey` — the provider's Nostr public key as 64 lowercase
  hex chars, exactly the value that appears as `event.pubkey` on the
  provider's public relay traffic.
- `providerNostrNpub` — the same key NIP-19 encoded, matching what the
  Pylon TUI logs as "Pylon Nostr npub".
- `providerMarketRelayRefs` — the market relay URLs the provider loop
  actually listens on, in declared order; the first entry is the canonical
  market relay.
- `providerNip90LaneRefs` — the declared NIP-90 lanes, e.g.
  `lane.public.nip90.5050.text_generation` for kind-5050 text inference and
  the kind-5934/5935/5936 labor lanes.

All four are graceful-optional: non-provider Pylons and pre-upgrade clients
project `null` / `[]`.

## Why

Orrery's 21-sat stranger-buyer probe (forum topic 499cec6e) found that
`/api/pylons` exposed no Nostr pubkey, so a relay bid could not be mapped to
registered capacity using public data alone. The pylon record carried only
`capabilityRefs`, `clientVersion`, `displayName`, `pylonRef`, `walletReady`,
and similar fields. With these fields a stranger buyer can take a NIP-90 bid
or NIP-89 handler event seen on a relay, match `event.pubkey` to
`providerNostrPubkey`, and confirm the bidder is registered capacity with
declared lanes — before paying a single sat.

## How the pubkey is carried (investigation result)

The Pylon derives a NIP-06 Nostr identity
(`apps/pylon/src/nostr-identity.ts`) and can sign NIP-98 headers, but the
worker's Pylon API (`workers/api/src/pylon-api-routes.ts`) authenticates
exclusively with `oa_agent_*` bearer tokens — there is **no NIP-98
verification anywhere in the worker**, so the worker did NOT already hold
the pubkey server-side from auth. (The pylon presence client falls back to
NIP-98 headers only when no agent token is configured, and that path is
rejected by the worker's bearer-only auth.) The registration body did
already include `identity.publicKey`/`identity.npub`, but the worker's
Effect Schema decode strips unmodeled keys.

The implementation is therefore explicit payload carriage plus projection:

- The Pylon presence client (`apps/pylon/src/presence.ts`,
  `providerDiscoveryFields`) includes the fields in registration and
  heartbeat bodies whenever the runtime declares the NIP-90 provider lane
  (`capability.public.pylon.nip90.text_inference.v0.3`, added by the
  go-online path).
- The worker (`workers/api/src/pylon-api.ts`) validates them with exact
  schema shapes (hex64, npub bech32, ws(s) relay URL, public-safe lane
  refs), persists them on the registration row (migration
  `0176_pylon_provider_discovery_fields.sql`), refreshes them from
  heartbeats via `nextRegistrationForEvent` (so pre-existing registrations
  upgrade on the next provider heartbeat without re-registering), and
  projects them in `publicPylonApiRegistrationProjection`.

## Consent semantics

A provider Pylon that goes online IS announcing publicly: its provider loop
publishes NIP-89 handler info (kind 31990) signed with this same pubkey on
the same market relays, and answers NIP-90 job requests with the same key.
Surfacing the pubkey, relay refs, and lane refs in `/api/pylons` adds
discoverability for stranger buyers, not exposure — the registry repeats
what the provider already broadcasts. Pylons that never declare the
provider lane never send the fields.

## Privacy boundary

Pubkey + relay refs + lane refs only. Nothing wallet-adjacent, no payment
material, no credential-source detail. Intake still passes through
`assertPylonApiPayloadSafe`, and field shapes are pinned at the schema
boundary.

One deliberate scanner decision (the platform-taxonomy allowlist lesson): a
64-char hex pubkey matches the `long_base64url_shaped` raw-id pattern in
`public-ref-scanner-safety.ts`, so the pubkey and npub are projected as
dedicated validated identity fields and are intentionally NOT routed
through `publicScannerSafeRef` — aliasing them would defeat the
bid-to-capacity mapping the fields exist for. Lane refs still go through
the scanner-safe path; relay refs are pinned to URL shapes that cannot
match the raw-id pattern.

## #4863 relay-cutover interplay

The relay refs are values carried from the Pylon — `relaysFromEnv()` in
`apps/pylon/src/provider-nip90.ts` (`PYLON_NIP90_RELAYS`, then
`OPENAGENTS_MARKET_RELAY_URL`, then the
`wss://openagents-market-relay.openagents.workers.dev` default) — never a
worker-side constant. When #4863 lands the `relay.openagents.com` custom
domain, providers pick up the new relay through configuration/defaults and
the next registration or heartbeat refreshes the projected refs
automatically. Nothing in the worker needs to change for the cutover.

## Claim discipline

This change makes registered provider capacity discoverable from public
data. It does NOT claim a completed live stranger-buyer leg: no assertion
is made here that a stranger buyer has paid a registered provider
end-to-end over the relay using these fields. That proof remains separate
evidence work.

## Verification

- `workers/api/src/pylon-api-routes.test.ts` — registration carrying the
  fields projects them through `/api/pylons` list/detail verbatim
  (including the scanner-shape allowlist pin); absent fields project
  `null`/`[]`; heartbeat upgrades a pre-upgrade registration; malformed
  identity fields are rejected.
- `apps/pylon/tests/presence.test.ts` — registration and heartbeat bodies
  carry the fields when the provider lane is declared and omit them
  otherwise.
- `bun test` in `apps/pylon`, targeted vitest in `workers/api`, api
  typecheck, and `bun run check:deploy` in `apps/openagents.com` all green
  at implementation time.
