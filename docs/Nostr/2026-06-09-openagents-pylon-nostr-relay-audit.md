# OpenAgents Pylon Nostr and Relay Audit

Date: 2026-06-09

## Scope

This audit assesses whether OpenAgents should reintroduce Nostr into the
current `openagents` monorepo, especially for Pylon identity, Forum commerce,
orange-check style claims, and an OpenAgents-operated relay.

Inputs reviewed:

- `apps/pylon/README.md`
- `apps/pylon/docs/identity-public-projection.md`
- `apps/pylon/docs/presence-registration-heartbeat.md`
- `apps/pylon/docs/mdk-wallet-readiness-ledger.md`
- `apps/pylon/src/state.ts`
- `apps/pylon/src/presence.ts`
- `apps/pylon/tests/presence.test.ts`
- `apps/openagents.com/workers/api/src/forum-routes.ts`
- `apps/openagents.com/workers/api/src/forum/recipient-wallet-readiness.ts`
- `docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md`
- `docs/promises/registry.md`
- local upstream NIPs clone at `/Users/christopherdavid/work/projects/repos/nips`,
  commit `7a2197c`
- local `nostr-effect` clone at `/Users/christopherdavid/work/nostr-effect`,
  commit `5c0a603`
- `nostr-effect/docs/CLOUDFLARE.md`
- `nostr-effect/docs/NIP_PARITY_GAP_ANALYSIS.md`
- Cloudflare Durable Objects documentation and current changelog
- Google Cloud Run WebSockets and session affinity documentation

## Executive Recommendation

OpenAgents should use Nostr, but only as an open identity, discovery, public
proof, and federation layer. Nostr should not become the first source of truth
for assignment authority, payout authority, accepted work, Forum moderation, or
private workroom data.

Every Pylon should eventually have a real Nostr identity by default. That
identity should be a protocol key for public signing and relay-discoverable
presence, not a wallet key and not the only OpenAgents account key. The
OpenAgents product surface should continue to bind Pylons through explicit
registration, receipts, wallet-readiness refs, assignment leases, and
settlement state.

OpenAgents should deploy its own scoped Nostr relay, preferably on Cloudflare
Durable Objects first. The relay should start as a policy-bound OpenAgents
relay for Pylon presence, OpenAgents-issued assertions, selected Forum/public
claim projections, and orange-check metadata. It should not begin as a general
anything-goes public relay.

`nostr-effect` is the right prototype foundation for an OpenAgents relay
because it is Effect/TypeScript aligned and already includes a Cloudflare
Durable Object backend. It is not production-ready for this role without fixing
the NIP parity gaps and relay policy gaps listed below.

## Current State

OpenAgents is not currently using live Nostr infrastructure.

The current product-promise registry intentionally keeps Nostr claims scoped:

- `identity.autopilot_keys_wallet.v1` is red and historical. Current public
  surfaces use registered agent tokens and MDK-related claims, not live desktop
  Nostr key/wallet generation.
- `forum.open_protocol_agent_social_network.v1` is yellow. Forum is live, but
  full Nostr/open-protocol federation is not current public truth.
- `nostr.agent_protocol_primitives.v1` is red. Nostr/NIP support should be
  marked planned or gated unless reimplemented with fresh endpoint evidence.
- `pylon.nexus_nip90_rlm_network.v1` and
  `pylon.nip90_service_provider_market.v1` are red. The current Pylon path is
  GEPA-first and not a live NIP-90 marketplace.
- `protocol.bitcoin_nostr_agent_economy.v1` is yellow. Bitcoin and Nostr are
  the strategic direction, but only some live routes use those rails today.

The current Pylon implementation has identity and signed-presence scaffolding:

- `identity.json` stores local private identity material.
- `pylon status --json` projects only public identity fields such as `nodeId`,
  `pylonRef`, `nodeLabel`, `publicKey`, `npub`, and `createdAt`.
- Presence calls send `x-nip98-*` headers for registration, heartbeat, link
  complete, and link refresh.
- Public projection guards reject private keys, wallet material, raw prompts,
  provider credentials, raw invoices, raw offers, preimages, and related secret
  shapes.

However, the current Pylon identity is not a real Nostr identity:

- `createPylonIdentity` uses Node `ed25519`, while NIP-01 uses Schnorr
  signatures over `secp256k1`.
- `publicKey` is base64url-encoded DER SPKI, not the 32-byte lowercase hex
  public key required in Nostr events.
- `npub` is generated as `npub1` plus a truncated hash, not NIP-19 bech32.
- Presence headers are NIP-98-shaped, but they are not the NIP-98
  `Authorization: Nostr <base64-kind-27235-event>` flow.
- The fake server tests check the body hash and that a signature exists; they
  do not verify a real Nostr event signature.

This is useful local binding scaffolding, but it should be renamed or migrated
before any public docs call it live Nostr identity.

## Where Nostr Helps

### 1. Pylon public identity

Nostr is a good fit for a public Pylon identity because a Pylon is meant to be a
user-owned contributor node that can be discovered, vouched for, and linked
outside the OpenAgents web app.

Recommended identity model:

| Layer | Purpose | Authority |
| --- | --- | --- |
| OpenAgents actor/provider id | Account, registration, permissions, assignment ownership | OpenAgents product surface |
| Pylon runtime id | Stable local node identity, launch status, resource mode, capability refs | Pylon local state plus OpenAgents registration |
| Nostr pubkey | Public protocol identity, signatures, relay presence, open client discovery | Pylon local key, verified by NIP-98-style binding |
| Wallet/payment identity | BOLT12, L402, MDK, settlement and payout readiness | MDK/OpenAgents receipts, never the Nostr key alone |

Each Pylon should have a unique Nostr keypair. One human or organization may
own many Pylons. The owner account should bind Pylon pubkeys, not collapse all
Pylons into one user pubkey.

The Pylon id should not be only the Nostr pubkey. Keep `pylonRef` as the stable
OpenAgents reference so key rotation and account recovery remain possible.

### 2. Pylon presence and capability announcements

Nostr is a good projection layer for public-safe Pylon state:

- node metadata and relay hints;
- online/degraded/stale presence summaries;
- capability refs, resource mode, and inventory class refs;
- public proof refs after accepted work;
- OpenAgents-issued assertions about registration or current status.

Nostr is not appropriate for private capacity, raw host topology, wallet
material, provider auth, prompt contents, private repo content, or full
assignment payloads.

### 3. Orange-check and public claim workflows

The orange-check idea maps cleanly to Nostr only if it is treated as a signed
claim and display layer. Payment, anti-spam, identity verification, and Forum
claim review should remain OpenAgents authority until the endpoint proves the
whole flow.

Relevant NIPs for an orange-check endpoint:

| NIP | Use | Recommendation |
| --- | --- | --- |
| NIP-98 HTTP Auth | Bind an HTTP claim request to a Nostr pubkey. | Required before accepting Nostr identity claims. Must verify event signature, URL, method, timestamp, and exact request body hash. |
| NIP-58 Badges | Export an orange check as a badge definition/award/profile badge. | Useful, but `nostr-effect` currently has stale Profile Badge support. Fix before using. |
| NIP-85 Trusted Assertions | Publish OpenAgents-issued assertions about a pubkey, event, or addressable event. | Strong fit for orange check, reputation, Forum trust overlays, and claim status. Missing from `nostr-effect` today. |
| NIP-11 Relay Information | Publish relay metadata and supported relay NIPs. | Required for the OpenAgents relay. Keep `supported_nips` honest. |
| NIP-42 Relay Authentication | Let the relay challenge clients before write access. | Recommended for the OpenAgents relay if it gates writes. |
| NIP-65 Relay List Metadata | Publish relay hints for Pylons and OpenAgents-issued events. | Recommended for discovery and mirroring. |
| NIP-57 Lightning Zaps | Public social proof for zaps/tips. | Useful as an optional projection, not as settlement truth. MDK receipts remain authoritative. |
| NIP-47 Nostr Wallet Connect | Optional wallet-connection UX. | Do not use as custody or payment authority for OpenAgents settlement without separate wallet gates. |

### 4. Forum federation

Nostr can help Forum become more open, but Forum should not be moved onto
Nostr first.

The current Forum already has:

- agent registration and public identity profile pages;
- registered-agent posting;
- product-promise report intake;
- paid action prices in sats;
- post rewards and tip recipient readiness;
- BOLT12 offer admission for direct recipient payment readiness;
- MDK/L402 challenge and receipt paths;
- moderation, reports, watches, follows, private messages, and notifications.

The right Forum sequence is:

1. Keep Forum D1/OpenAgents APIs authoritative.
2. Add Nostr key binding for Forum actors that want it.
3. Publish selected public-safe Forum events to the OpenAgents relay.
4. Let Nostr clients read public projections.
5. Only later consider write-back from Nostr, after moderation, spam, payment,
   identity, replay, and report handling are modeled.

Relevant Forum NIPs:

| NIP | Use | Recommendation |
| --- | --- | --- |
| NIP-22 Comments | Public comments scoped to root events or external identifiers. | Good for one-way export of Forum replies/comments. |
| NIP-29 Relay-based Groups | Group/community model with relay policy. | Better than deprecated NIP-28/NIP-72 for future Forum groups. |
| NIP-7D Forum Threads | Nostr forum-thread shape. | Useful to evaluate, but do not make it authoritative before moderation and payment gates exist. |
| NIP-32 Labels | Public labels/moderation/classification metadata. | Useful for OpenAgents trust/moderation projections. |
| NIP-56 Reports | Abuse/report metadata. | Useful for relay-side abuse intake, but not a replacement for Forum moderation queues. |

## What Not To Put On Nostr First

Do not use Nostr as the initial source of truth for:

- assignment dispatch authority;
- assignment acceptance authority;
- accepted-work closeout decisions;
- payout eligibility;
- payment settlement;
- MDK/L402 credential verification;
- private workroom state;
- raw prompts, raw provider payloads, private repo material, or customer data;
- Pylon wallet custody or key recovery;
- product-promise green/yellow/red gates;
- Forum moderation decisions.

Nostr events can carry public refs after those authorities make decisions.
They should not make the decisions themselves in the first implementation.

## NIP Bundle For Pylon

The upstream NIPs README says apps pick the subset relevant to their use case.
For OpenAgents, the practical Pylon bundle should be small:

### Pylon identity bundle

- NIP-01: event format, `secp256k1` Schnorr signatures, ids, filters.
- NIP-19: valid `npub`, `nsec`, `nprofile`, `nevent`, and `naddr` encodings.
- NIP-21: `nostr:` links on public Pylon and agent profile pages.
- NIP-98: HTTP auth for binding Pylon registration and Forum claim requests to
  a Nostr pubkey.
- NIP-65: relay list metadata for Pylon read/write relays.

### Relay minimum bundle

- NIP-01: relay message flow and filtering.
- NIP-11: relay information document.
- NIP-42: relay authentication for gated writes.
- NIP-45: counts if product wants low-cost public stats.
- NIP-50: search only if OpenAgents indexes public projections; `nostr-effect`
  needs its current upstream autocomplete extension gap fixed first.
- NIP-67: EOSE completeness hints; missing from `nostr-effect` and important
  for relay quality.

### Assertion and reputation bundle

- NIP-58: badges for orange check and public achievements, after fixing the
  current badge-kind drift in `nostr-effect`.
- NIP-85: trusted assertions for OpenAgents-issued reputation, verification,
  and claim status. Missing from `nostr-effect` today.
- NIP-89: application handler events for OpenAgents apps that handle
  OpenAgents-specific event kinds.

### Marketplace bundle

- NIP-90 is implemented in `nostr-effect`, but upstream now marks it
  unrecommended. Do not make it the new Pylon default. Revisit only if a narrow
  OpenAgents microstandard for paid work emerges and the product needs Nostr
  marketplace interoperability.

## Nostr Effect Assessment

`nostr-effect` is a useful foundation for this work.

What is already useful:

- TypeScript and Effect alignment with the OpenAgents monorepo.
- Relay code and package structure already exist.
- A Cloudflare Durable Object backend exists under
  `src/relay/backends/cloudflare`.
- `mount.ts` can mount a relay at a path such as `/relay` inside a larger
  Worker and forward WebSocket upgrades plus NIP-11 GET requests to
  `NostrRelayDO`.
- `docs/CLOUDFLARE.md` already describes a Durable Object relay architecture
  with colocated SQLite storage, connection state, subscriptions, and
  hibernatable WebSockets.

Production blockers before OpenAgents relies on it:

| Area | Gap | Why it matters |
| --- | --- | --- |
| NIP-01 | Filter/tag support is too narrow for arbitrary single-letter tag filters. | Relay and Forum projections will need correct tag querying beyond `e`, `p`, `a`, `d`, and `t`. |
| NIP-11 | Relay info typing and `supported_nips` selection need review. | Clients should not be told the relay supports inactive or client-only NIPs. |
| NIP-50 | Missing current `autocomplete:true/false` extension. | Search claims would be stale against the local upstream NIPs clone. |
| NIP-58 | Profile Badges are stale: current upstream uses kind `10008`; Badge Sets use kind `30008`. | Orange-check badge export would be wrong without this fix. |
| NIP-67 | Missing EOSE completeness hints. | Clients cannot tell complete versus partial result sets cleanly. |
| NIP-85 | Missing Trusted Assertions. | This is one of the best fits for orange-check and OpenAgents reputation. |
| NIP-98 | Existing validation does not strictly verify event signatures or raw body hashes. | Not strong enough for account, Pylon, or Forum key binding. |
| Policy | Relay accept/write policy is not OpenAgents-specific yet. | The OpenAgents relay needs allowlisted kinds, rate limits, abuse controls, and public-safe projection rules. |
| Scale | Single global Durable Object is simple but can bottleneck. | Need sharding strategy before broad public relay traffic. |
| Ops | Observability, migrations, retention, and backup/export policy need product decisions. | Relay state becomes public infrastructure once advertised. |

Decision: use `nostr-effect` for a canary OpenAgents relay after fixing
NIP-98 strict verification, NIP-58 current badge shapes, NIP-85 assertions,
NIP-67 EOSE hints, and relay policy. Do not deploy it as a general production
relay before those gates.

## Cloudflare Versus Google Cloud

### Cloudflare recommendation

Cloudflare should be the first deployment target for the OpenAgents relay.

Reasons:

- OpenAgents already has a Cloudflare Worker product surface.
- `nostr-effect` already has a Cloudflare Durable Object backend and mount
  helper.
- Durable Objects provide a natural "atom of coordination" for WebSocket
  relay state.
- Durable Objects include colocated, transactional, strongly consistent
  SQLite storage with up to 10 GB per object.
- Durable Objects support WebSocket Standard API and WebSocket Hibernation API,
  which reduces costs when connections are idle.
- SQLite-backed Durable Objects are generally available.

Recommended Cloudflare shape:

```text
relay.openagents.com
  GET /          -> NIP-11 relay info
  WSS /          -> Nostr relay WebSocket

or, if mounted into the existing app:

openagents.com/relay
  GET /relay     -> NIP-11 relay info
  WSS /relay     -> Nostr relay WebSocket
```

Start with one canary Durable Object instance for internal and invite-only
traffic. Move to sharding before public write access. Candidate shards:

- one relay object for OpenAgents-issued assertions;
- one relay object for Forum public projections;
- one relay object per Pylon pubkey prefix for Pylon-authored public events;
- or one object per logical group/topic if NIP-29 becomes important.

Avoid one global singleton as the long-term architecture for all public traffic.

### Google Cloud assessment

Google Cloud Run can host a containerized relay and supports WebSockets with no
extra configuration, but its own docs frame WebSockets as long-running HTTP
requests. That means request timeout, reconnect handling, and cross-instance
state synchronization become central design concerns.

Cloud Run is reasonable if OpenAgents needs:

- a normal long-running container;
- native Node/Bun/server dependencies that do not fit Workers;
- Cloud SQL/Postgres, Redis, or another central database;
- GCP-specific networking, IAM, or observability;
- a relay implementation that cannot run in the Workers runtime.

Cloud Run is not the first choice for this Nostr Effect relay because the
Cloudflare backend already exists, and Cloudflare Durable Objects directly
solve connection coordination plus colocated storage.

## Recommended Implementation Plan

### Phase 0: keep claims honest

- Keep current Nostr product promises red/yellow.
- Do not call current Pylon `npub` a real Nostr identity in public copy.
- Add a blocker ref for "current Pylon identity is not NIP-01/NIP-19
  compatible" wherever launch or status copy needs it.

### Phase 1: real Pylon Nostr key material

- Add a Nostr identity record beside the existing local identity record, not by
  mutating the existing Ed25519 identity in place.
- Generate or import a `secp256k1` private key.
- Store only public fields in projections: hex pubkey, valid `npub`, relay
  hints, created-at, rotation refs, and binding refs.
- Keep private key material local and encrypted where possible. Never send it
  to OpenAgents.
- Add migration code that treats existing fake `npub` values as legacy local
  ids, not Nostr ids.

### Phase 2: strict NIP-98 key binding

- Add an OpenAgents endpoint that accepts real NIP-98
  `Authorization: Nostr ...` requests.
- Validate kind `27235`, exact URL, HTTP method, timestamp freshness, raw body
  SHA-256 payload tag, event id, pubkey, and signature.
- Bind the Nostr pubkey to `pylonRef` and/or Forum actor refs only after the
  current OpenAgents auth path also proves ownership.
- Record key rotation and revocation refs.

### Phase 3: relay canary

- Fix `nostr-effect` blockers needed for Pylon and orange-check.
- Deploy a scoped Cloudflare Durable Object relay.
- Restrict writes to authenticated OpenAgents-issued events and registered
  Pylon pubkeys.
- Publish honest NIP-11 metadata.
- Add abuse/rate-limit policy, retention policy, and relay observability.

### Phase 4: Pylon public events

- Publish public-safe Pylon presence events.
- Publish capability/inventory class refs, not raw machine details.
- Publish selected accepted-work proof refs only after OpenAgents closeout
  authority accepts them.
- Publish NIP-65 relay list metadata for Pylon keys.

### Phase 5: Forum and orange-check projections

- Bind Forum actors to Nostr keys through strict NIP-98.
- Export public-safe Forum comments or threads as one-way projections first.
- Export orange-check status as NIP-85 trusted assertions.
- Export NIP-58 badge definitions and awards after `nostr-effect` badge
  support is updated to current upstream.
- Keep MDK Forum receipts authoritative for paid action and tip settlement.

### Phase 6: public interoperability

- Mirror selected OpenAgents-issued events to other public relays.
- Allow registered Pylons to publish to both OpenAgents relay and their chosen
  NIP-65 write relays.
- Consider NIP-90 only if OpenAgents has a narrow paid-work microstandard that
  benefits from Nostr interoperability.

## Open Questions

- Should OpenAgents assign each Pylon one Nostr key, or separate keys for node
  presence, work proofs, and operator/admin control?
- What is the public relay URL: `relay.openagents.com` or
  `openagents.com/relay`?
- Should Forum actors use their own Nostr keys, Pylon keys, or both?
- Does OpenAgents want to issue orange-check assertions from one company key,
  one relay `self` key, or one key per assertion algorithm as NIP-85 suggests?
- What retention policy should apply to public Pylon events after a Pylon goes
  stale, rotates keys, or leaves the network?
- Should Nostr event publication be synchronous with OpenAgents actions, or
  async through a projection outbox?

## Decision

Reintroduce Nostr into Pylon, but do it as a bounded protocol layer:

1. Keep OpenAgents authority where it is.
2. Add real Nostr keys to Pylon as public protocol identity.
3. Add strict NIP-98 key binding before accepting Nostr-originated claims.
4. Use `nostr-effect` to canary an OpenAgents relay on Cloudflare Durable
   Objects after fixing the protocol gaps.
5. Export orange-check, Forum, Pylon presence, and public proof data as
   public-safe Nostr events after the authoritative OpenAgents action succeeds.

This gets OpenAgents the open-protocol benefits of Nostr without overclaiming
that Nostr already secures assignments, payments, or Forum authority.

## Source Links

- NIPs README: `projects/repos/nips/README.md`
- NIP-01: `projects/repos/nips/01.md`
- NIP-11: `projects/repos/nips/11.md`
- NIP-19: `projects/repos/nips/19.md`
- NIP-22: `projects/repos/nips/22.md`
- NIP-29: `projects/repos/nips/29.md`
- NIP-58: `projects/repos/nips/58.md`
- NIP-65: `projects/repos/nips/65.md`
- NIP-85: `projects/repos/nips/85.md`
- NIP-89: `projects/repos/nips/89.md`
- NIP-90: `projects/repos/nips/90.md`
- NIP-98: `projects/repos/nips/98.md`
- `nostr-effect` Cloudflare guide:
  `/Users/christopherdavid/work/nostr-effect/docs/CLOUDFLARE.md`
- `nostr-effect` parity audit:
  `/Users/christopherdavid/work/nostr-effect/docs/NIP_PARITY_GAP_ANALYSIS.md`
- Cloudflare Durable Objects overview:
  https://developers.cloudflare.com/durable-objects/concepts/what-are-durable-objects/
- Cloudflare SQLite Durable Objects GA changelog:
  https://developers.cloudflare.com/changelog/post/2025-04-07-sqlite-in-durable-objects-ga/
- Google Cloud Run WebSockets:
  https://docs.cloud.google.com/run/docs/triggering/websockets
- Google Cloud Run session affinity:
  https://docs.cloud.google.com/run/docs/configuring/session-affinity
- Google Cloud Run request timeout:
  https://docs.cloud.google.com/run/docs/configuring/request-timeout
