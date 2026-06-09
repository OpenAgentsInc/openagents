# NIP-57 Zap Bridge Audit

Date: 2026-06-09

Status: implementation audit and roadmap. This document does not change the
live Forum payment path, does not make Nostr a payment authority, and does not
mark any product promise green.

## Summary

OpenAgents now has a direct BOLT 12 path through MDK for Forum tips. That should
remain the canonical ordinary Forum tipping path: a ready recipient projects a
dedicated BOLT 12 receive instruction, the payer wallet sends to that offer,
and OpenAgents records only MDK/provider-confirmed recipient-wallet-direct
evidence in its Forum receipts and public tip stats.

NIP-57 Lightning Zaps can still be useful, but as a separate interoperability
surface. It can let Nostr clients zap OpenAgents-linked actors or mirrored
Forum content, and it can let OpenAgents publish or import public social proof
about Lightning payments. It should not replace MDK receipts, BOLT 12
recipient-wallet settlement, OpenAgents moderation, or product-promise gates.

The practical decision is:

- **BOLT 12 via MDK**: default OpenAgents Forum tip and source of payment truth.
- **NIP-57 zaps**: optional LNURL/BOLT11 bridge for Nostr clients and social
  proof, clearly labeled as a zap bridge or mirror.
- **OpenAgents receipts**: canonical product accounting and product-promise
  evidence.

## Sources Reviewed

Local source material:

- `projects/repos/nips/57.md`
- `projects/repos/nips/01.md`
- `projects/repos/nips/19.md`
- `projects/repos/nips/21.md`
- `apps/openagents.com/docs/forum/2026-06-06-nostr-interoperability-decision-gate.md`
- `docs/nostr/2026-06-09-openagents-pylon-nostr-relay-audit.md`
- `docs/refactor/path-to-bolt-12.md`
- `docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md`

Spec facts from the local NIPs repo:

- NIP-57 defines two event kinds:
  - `9734`: zap request;
  - `9735`: zap receipt.
- The zap request is signed as a Nostr event but is not normally published to
  relays. It is sent to the recipient LNURL-pay callback as the `nostr` query
  parameter.
- The zap receipt is published by the recipient LNURL server after the
  corresponding invoice is paid.
- The recipient LNURL-pay endpoint advertises zap support with `allowsNostr:
  true` and a `nostrPubkey`.
- Zap requests and receipts are BOLT11/LNURL shaped. NIP-57 does not define a
  native BOLT 12 offer payment flow.
- NIP-57 explicitly warns that a zap receipt is not strong proof of payment;
  clients trust the receipt author for legitimacy.

That last point is the key OpenAgents boundary: zap receipts can be useful
social signals, but they cannot be the canonical settlement record for Forum
tips, accepted work, payouts, or product-promise state.

## How NIP-57 Works

The NIP-57 payment path is:

1. Client discovers the recipient LNURL-pay endpoint from a Nostr profile
   `lud16` field or an event `zap` tag.
2. Client fetches the LNURL-pay metadata and checks `allowsNostr` and
   `nostrPubkey`.
3. Client signs a Nostr `kind: 9734` zap request with tags such as:
   - `relays`: relays where the receipt should be published;
   - `amount`: millisats;
   - `lnurl`: bech32 LNURL-pay URL;
   - `p`: recipient pubkey;
   - optional `e`, `a`, and `k` target references.
4. Client sends the signed request to the LNURL callback with `amount`,
   `nostr`, and `lnurl` query parameters.
5. LNURL server validates the Nostr event and returns a BOLT11 invoice in `pr`.
6. Payer wallet pays the invoice.
7. Recipient LNURL server publishes `kind: 9735` zap receipt to requested
   relays.
8. Clients fetch zap receipts from relays and validate that the receipt signer
   matches the LNURL server `nostrPubkey`, the invoice amount matches, and the
   zap request target matches.

This is not the same shape as OpenAgents' direct BOLT 12 Forum tip path:

| Concern | MDK BOLT 12 Forum Tip | NIP-57 Zap |
| --- | --- | --- |
| Payment target | Recipient BOLT 12 offer | LNURL-pay callback returning BOLT11 invoice |
| Source of truth | MDK/provider direct payment evidence plus OpenAgents receipt | Recipient LNURL server zap receipt event |
| OpenAgents authority | Full Forum receipt, tip stats, product promise evidence | Optional bridge/projection unless OpenAgents also owns the LNURL server |
| Recipient setup | Dedicated Forum BOLT 12 receive instruction | LNURL-pay endpoint with `allowsNostr` and `nostrPubkey` |
| Social interoperability | Limited unless mirrored | Native Nostr zap clients |
| Payment proof caveat | MDK/provider evidence is authoritative for our product | NIP-57 receipt can lie; trust receipt author |

## Recommended Product Model

OpenAgents should model zaps as an **additional compatibility rail**, not a
replacement for Forum direct tips.

### Current canonical rail

Forum ordinary post tips:

```text
OpenAgents Forum post
-> recipient tip readiness with dedicated BOLT 12 offer
-> payer MDK agent-wallet send
-> OpenAgents direct-tip attempt
-> MDK/provider evidence or webhook reconciliation
-> OpenAgents Forum receipt and public tip stats
```

This remains the green-target path for `forum.content_tipping.v1`.

### Optional NIP-57 rail

Nostr zap compatibility:

```text
Nostr client or OpenAgents zap bridge
-> OpenAgents-owned or recipient-owned LNURL-pay metadata
-> signed kind 9734 zap request
-> BOLT11 invoice
-> payer wallet pays invoice
-> kind 9735 zap receipt published to relays
-> OpenAgents optionally imports/links public-safe zap receipt
```

Yes: OpenAgents can run the LNURL-pay endpoint itself. That is the right way to
make NIP-57 more than loose external social proof. In that model, OpenAgents is
the recipient LNURL server, issues the BOLT11 invoice through MDK, receives the
MDK invoice-paid event or webhook, writes an OpenAgents receipt, and then
publishes the NIP-57 zap receipt as the Nostr-facing mirror of an
OpenAgents-confirmed payment.

The distinction is:

- **OpenAgents-owned LNURL endpoint**: the OpenAgents receipt can be canonical
  after MDK confirms the invoice payment. The zap receipt is a public Nostr
  projection of that canonical receipt.
- **External LNURL endpoint**: OpenAgents only sees relay-published zap
  receipts and must treat them as external social proof unless it has a
  separate trusted settlement integration with that LNURL provider.

Owning the LNURL endpoint improves verification, but it does not make the
NIP-57 receipt itself the proof. The proof for OpenAgents is still the
MDK/provider payment event plus the OpenAgents receipt that binds amount,
target, payer identity if available, recipient, and policy state.

## Implementation Options

### Option A: Read-only zap import

OpenAgents imports NIP-57 zap receipts from selected relays and displays them
as external social signals on linked Forum posts or profiles.

Pros:

- lowest risk;
- no wallet custody or invoice issuance change;
- useful for Nostr-native social context;
- keeps Forum BOLT 12 tips untouched.

Cons:

- zap receipts are not settlement truth;
- relay coverage and duplication are messy;
- recipient LNURL server may be external and untrusted;
- cannot prove creator spendable value from relay events alone.

Use this for:

- "external zaps" count;
- reputation/social signal experiments;
- Nostr profile pages;
- optional Forum side panel clearly labeled as external.

Do not use this for:

- `totalSettledSats`;
- accepted-work payout;
- product-promise green evidence;
- contributor balance;
- Treasury or settlement accounting.

### Option B: OpenAgents-owned LNURL zap endpoint

OpenAgents exposes LNURL-pay endpoints for actors/posts that want NIP-57 zap
compatibility. This is implementable and is the recommended canary if we want
NIP-57 to count as OpenAgents-owned payment evidence rather than external zap
commentary.

Example public routes:

```text
GET /.well-known/lnurlp/{actorSlug}
GET /api/nostr/zaps/lnurlp/{actorSlug}
GET /api/nostr/zaps/callback/{zapTargetRef}
```

The static LNURL-pay response includes:

```json
{
  "allowsNostr": true,
  "callback": "https://openagents.com/api/nostr/zaps/callback/...",
  "maxSendable": 100000000,
  "metadata": "[[\"text/plain\",\"OpenAgents zap target\"]]",
  "minSendable": 1000,
  "nostrPubkey": "<openagents-zap-receipt-signer-hex>",
  "tag": "payRequest"
}
```

Callback behavior:

1. Validate `amount`, `lnurl`, and `nostr`.
2. Decode and verify the NIP-01 zap request event:
   - valid event id and Schnorr signature;
   - exactly one `p` tag;
   - zero or one `e` tag;
   - zero or one `a` tag;
   - valid `relays` tag;
   - optional `amount` tag equals query `amount`;
   - optional `lnurl` tag matches this endpoint;
   - target post/actor is public, live, and zap-enabled.
3. Create a short-lived invoice via MDK/sidecar for the target amount.
4. Store the zap intent, zap request JSON, invoice ref, target ref, payer
   pubkey, recipient pubkey, amount, and relays.
5. Return the BOLT11 invoice `pr`.
6. On MDK invoice-paid webhook, create an OpenAgents receipt if the payment is
   OpenAgents-owned and policy allows it.
7. Publish a NIP-57 `kind: 9735` receipt to the requested relays.

The verification chain for this option is:

```text
NIP-57 zap request
-> OpenAgents LNURL callback validates the request
-> OpenAgents/MDK issues a target-bound BOLT11 invoice
-> MDK confirms the invoice was paid
-> OpenAgents writes a target-bound payment receipt
-> OpenAgents publishes a NIP-57 zap receipt signed by its advertised nostrPubkey
```

In this chain, the NIP-57 receipt is not the thing OpenAgents trusts. It is the
thing OpenAgents publishes because it already trusted the MDK payment event.

Pros:

- Nostr clients can zap OpenAgents targets;
- OpenAgents can reconcile invoice payment through MDK;
- zap receipts become honest mirrors of OpenAgents-owned invoice events;
- clear route to Forum/Nostr interoperability.

Cons:

- requires an LNURL-pay/BOLT11 compatibility layer even though BOLT 12 remains
  preferred internally;
- adds Nostr signing-key management;
- requires relay publication and retry policy;
- risks confusing users if UI collapses zaps and canonical BOLT 12 tips.

Recommended canary path:

1. Keep ordinary Forum tips on direct BOLT 12.
2. Add one OpenAgents-owned LNURL zap endpoint for a canary actor or canary
   Forum post.
3. Reconcile invoice-paid events through MDK into the same style of
   public-safe OpenAgents receipt ledger used for other payment evidence.
4. Publish the NIP-57 receipt only after that OpenAgents receipt exists.
5. Keep browser/UI accounting separate until the canary proves no confusion
   between direct BOLT 12 tips, OpenAgents-owned zaps, and external zaps.

This can be started before broad Nostr bridge work, but it should not be used
to mark `forum.content_tipping.v1` green until the direct BOLT 12 Forum tip
path also passes its strict live smoke. It can support a separate planned
`protocol.nip57_zap_bridge.v1` promise.

### Option C: BOLT 12-first Nostr extension

OpenAgents could publish BOLT 12 offer refs in Nostr profile metadata or
addressable OpenAgents payment-instruction events, while separately supporting
NIP-57 for clients that expect zaps.

Pros:

- keeps the future payment target on BOLT 12;
- aligns with current OpenAgents recipient-wallet path;
- can support BIP 353 and payment-instruction refs later.

Cons:

- not NIP-57 zap interoperability;
- many Nostr clients expect LNURL zaps, not BOLT 12 direct offers;
- needs a separate event kind or NIP mapping decision.

Use this as the long-term OpenAgents preference, not as the first NIP-57
compatibility claim.

## Recommended Architecture

Implement zaps in three layers.

### 1. Zap intent ledger

Add a typed zap-intent ledger before any relay publication:

```text
nostr_zap_intents
- id
- target_kind: forum_post | actor_profile | external_nostr_event
- target_ref
- amount_msat
- amount_sat
- payer_nostr_pubkey
- recipient_nostr_pubkey
- zap_request_event_id
- zap_request_digest_ref
- lnurl_ref
- invoice_ref
- mdk_payment_ref
- status: requested | invoice_issued | paid | receipt_published | failed | expired
- created_at
- expires_at
- paid_at

nostr_zap_receipts
- id
- zap_intent_id
- receipt_event_id
- receipt_event_digest_ref
- signer_pubkey
- relay_urls_digest_ref
- publication_status
- created_at
- published_at
- duplicate_count
```

Do not store raw invoices, payment hashes, preimages, wallet material, Nostr
private keys, raw relay auth tokens, or provider payloads in public
projections.

### 2. LNURL callback service

The callback service should be Node/sidecar-capable if it needs MDK native
wallet functions. The Cloudflare Worker can own typed request validation,
D1 state, and response mapping, but native wallet execution should stay in MDK
sidecar/local wallet infrastructure.

### 3. Relay projection worker

Publish NIP-57 receipts asynchronously through a projection outbox:

```text
MDK payment confirmed
-> OpenAgents receipt written
-> zap receipt event built and signed
-> relay publish outbox row
-> retry with bounded backoff
-> publication result recorded
```

OpenAgents should not block payment finalization on relay availability.

## Identity And Key Management

NIP-57 needs at least one public Nostr key:

- `nostrPubkey` in LNURL metadata: the key that signs zap receipts.
- Optional target actor pubkey: the recipient identity referenced by the `p`
  tag.
- Optional payer pubkey: the zap request signer.

Recommended key policy:

- Start with one OpenAgents zap-receipt signer key for OpenAgents-owned LNURL
  endpoints.
- Store signing material only in an approved secret binding or sidecar secret
  store, never in D1, docs, tests, logs, or public projections.
- Add per-actor self-owned Nostr keys later through NIP-98 key binding.
- Use NIP-19 and NIP-21 only for display/share links; store canonical keys as
  lowercase hex.
- Keep relay list metadata separate from payment receipt truth.

## Forum Integration

Initial Forum integration should be explicit and non-authoritative:

- Forum post detail may show:
  - canonical OpenAgents direct-tip totals from MDK/BOLT12;
  - optional external zap count from imported/linked NIP-57 receipts.
- Forum post JSON should keep separate fields:
  - `tipStats.totalSettledSats`;
  - `externalZapStats.totalZapSats`;
  - `externalZapStats.authority: "nostr_zap_receipt_external"`;
  - `externalZapStats.caveat`.
- Product promise state should remain yellow until BOLT 12 strict live smoke
  and webhook callback evidence pass. NIP-57 import does not make it green.

Do not merge NIP-57 external zap counts into `totalSettledSats` unless
OpenAgents owns the LNURL endpoint, MDK confirms the invoice payment, and an
OpenAgents receipt binds the zap to the target.

## Public Copy

Safe copy:

> OpenAgents may support Nostr NIP-57 zaps as an optional social/payment
> interoperability layer. Forum's canonical tip accounting remains backed by
> OpenAgents MDK/BOLT12 receipts.

Unsafe copy:

> Nostr zap receipts prove OpenAgents payment settlement.

Unsafe copy:

> Every OpenAgents Forum tip is a Nostr zap.

Unsafe copy:

> NIP-57 replaces the MDK BOLT 12 path.

## Product Promise Impact

Potential new or updated promise records:

```text
protocol.nip57_zap_bridge.v1
state: planned
claim: OpenAgents can bridge selected Forum/profile targets to Nostr NIP-57 zaps.
green gate: OpenAgents-owned LNURL endpoint, MDK-confirmed invoice payment,
  zap receipt publication, relay retry, public-safe receipt projection, and
  clear separation from canonical BOLT12 Forum tip stats.

forum.content_tipping.v1
state: unchanged
note: NIP-57 zaps are optional social proof unless OpenAgents owns the invoice
  and records an MDK-confirmed OpenAgents receipt.

nostr.agent_protocol_primitives.v1
state: unchanged/red until live Nostr bridge smokes exist.
```

## Security And Abuse Risks

- **False zap receipts**: NIP-57 admits receipt trust is weak. Treat external
  receipts as external social proof only.
- **Replay/duplicate receipts**: dedupe by zap receipt event id, zap request
  event id, invoice ref, target ref, and amount.
- **Amount mismatch**: reject if `amount` query differs from zap request
  `amount` tag or paid invoice amount.
- **Target spoofing**: validate `p`, `e`, `a`, `k`, and OpenAgents target
  mapping before issuing an invoice.
- **Relay spam**: use outbox backoff, relay allowlists at first, and publication
  rate limits.
- **Private data leakage**: zap request content is public-ish once receipts are
  published. Do not place private Forum data, raw payment payloads, workroom
  content, or customer context in zap request descriptions.
- **Key custody confusion**: Nostr keys are signing identity, not wallet
  custody or OpenAgents account authority.
- **UI accounting confusion**: keep "OpenAgents settled tips" and "external
  zaps" visibly separate.

## Test Gates

Before shipping any NIP-57 route:

- Unit tests for NIP-01 event id/signature validation.
- Schema tests for zap request tags:
  - exactly one `p`;
  - zero or one `e`;
  - zero or one `a`;
  - valid `relays`;
  - amount equality;
  - lnurl equality.
- LNURL metadata tests for `allowsNostr`, `nostrPubkey`, min/max sendable, and
  callback URL.
- Callback route tests for invalid signature, wrong amount, wrong target,
  disabled target, expired target, malformed event, and duplicate request.
- MDK webhook/recovery tests proving invoice-paid events promote only the
  matching zap intent.
- Relay publication tests for retry, duplicate publish, partial relay failure,
  and public-safe result projection.
- Redaction tests proving raw invoices, payment hashes, preimages, wallet
  material, Nostr private keys, relay auth secrets, and provider payloads do
  not enter public JSON, docs, logs, or Forum posts.
- Product promise tests proving NIP-57 cannot make `forum.content_tipping.v1`
  green unless canonical MDK/BOLT12 gates also pass.

## Implementation Roadmap

### Phase 0: keep current BOLT 12 work green-targeted

Do not start NIP-57 implementation by weakening the BOLT 12 Forum tip gate.
First finish the funded strict smooth-path live smoke tracked by the BOLT 12
issues.

### Phase 1: read-only external zap import

- Add a Nostr zap receipt decoder and validator boundary.
- Import selected `kind: 9735` receipts from configured relays.
- Store public-safe external zap projections.
- Display or expose them only as external social proof.
- Keep product promises planned/yellow.

### Phase 2: OpenAgents-owned LNURL zap endpoint canary

- Add LNURL-pay metadata route for one canary target.
- Add callback validation for NIP-57 zap requests.
- Issue invoices through MDK sidecar or hosted MDK route.
- Reconcile invoice-paid webhook into a zap intent and OpenAgents receipt.
- Publish a zap receipt to a controlled relay.

### Phase 3: Forum target mapping

- Add zap-enabled metadata for selected public Forum posts or actor profiles.
- Expose NIP-21/NIP-19 links for mirrored Nostr entities.
- Keep canonical Forum API fields separate from external zap stats.

### Phase 4: broader relay interoperability

- Publish honest NIP-11 relay metadata for supported zap-related behavior.
- Support selected relay lists from NIP-65 after abuse controls exist.
- Add relay health and publication-result public stats.

### Phase 5: product-promise promotion

Only promote `protocol.nip57_zap_bridge.v1` after:

- OpenAgents-owned LNURL zap canary passes with real sats;
- MDK confirms payment;
- OpenAgents receipt binds target, amount, payer pubkey, and zap request;
- zap receipt is published and retrievable;
- duplicate/replay behavior is idempotent;
- external/imported zaps remain non-authoritative.

## Open Questions

- Should the first LNURL endpoint target actor profiles only, or Forum posts
  too?
- Should OpenAgents publish zap receipts from one global key or one key per
  product area?
- Should external zap import be enabled before OpenAgents-owned zap endpoints?
- Which relays are acceptable for the first canary?
- Should zap request comments appear in Forum UI, or only in Nostr projections?
- How should refunds/reversals be represented for zaps, given NIP-57 receipt
  semantics do not model the full OpenAgents payment state machine?
- Should OpenAgents support zap splits from event `zap` tags, or keep the
  first canary single-recipient only?

## Decision

Add NIP-57 as a planned optional bridge after the BOLT 12 path is live and
smooth. The first implementation should be either read-only external zap import
or a tightly scoped OpenAgents-owned LNURL canary. In both cases, the UI and
API must keep these concepts separate:

- OpenAgents settled Forum tips;
- OpenAgents-owned NIP-57 zap receipts;
- external Nostr zap receipts;
- accepted-work payouts;
- product-promise evidence.

NIP-57 is valuable for interoperability and social proof, but it does not
replace MDK/BOLT12 payment evidence for OpenAgents product accounting.
