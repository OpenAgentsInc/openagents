# NIP-MKT-LSP

## Lightning channel and just-in-time liquidity

`draft` `optional`

MKT-LSP version 1 is a profile of [NIP-MKT](MKT.md) for comparing and
accepting channel-purchase and just-in-time inbound-liquidity terms. It aligns
with bLIP-50 LSPS0, bLIP-51 LSPS1, and bLIP-52 LSPS2. LSPS messages over
BOLT8 remain canonical for service execution. MKT-LSP supplies portable
discovery, comparable signed terms, evidence references, and recovery rules.

It does not replace BOLT8, channel opening, Lightning commitment state, LSPS
JSON-RPC, or wallet signing.

## Profile identity and allocation

- Profile ID: `mkt-lsp`
- Version: `1`
- Parent: NIP-MKT v0.1
- Semantics baseline: `lightning/blips` revision
  `ca04f374d03001ddbed60ff109da58bd9c390c9a`; each Quote still pins the
  exact LSPS revisions and method artifacts it uses.
- Authority: the author pubkey of the selected `kind:39602` Profile
  Descriptor. Clients MUST allowlist that exact key and pin the descriptor
  event ID and specification digest.
- Specification digest: the descriptor's `x` tag over the exact bytes of this
  file. This draft designates no universal authority.

| Kind        | Name                 | Publication                                       | Replacement law                   |
| ----------- | -------------------- | ------------------------------------------------- | --------------------------------- |
| 39650       | LSP Service Contract | private signed record in NIP-59 `kind:1059` wraps | unique `d`; immutable-by-contract |
| 39651-39659 | Reserved             | —                                                 | unallocated                       |

`kind:39650` is the minimum profile-specific kind. A Quote precedes requester
acceptance and a native LSPS order or JIT promise. An Order cannot bind the
later LSPS response, external IDs, funding-output constraints, and both
parties' recovery commitments. The LSP Service Contract supplies that
post-Order, pre-payment commitment. Native chain and Lightning receipts remain
typed evidence references and are not copied into a generic Nostr evidence
kind.

The kind extends NIP-MKT's supported private-kind set and uses the same signed
inner event, rumor, seal, per-recipient wrap, signer-equality, recipient gate,
32 KiB bound, duplicate-JSON rejection, replay, and idempotency rules. Clients
fail closed on unknown critical members.

### Collision review

The allocation was checked on 2026-08-04 against these exact revisions:

| Source                                           | Revision                                   | Result                                                                    |
| ------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------------- |
| Official NIPs                                    | `c53877571f96eb423661fc23c620d629d37b8f19` | No assignment in `39650-39659`                                            |
| Block Buzz NIPs                                  | `540b58920cef205b838da8be8442aae62bceaaa5` | No assignment in `39650-39659`                                            |
| OpenAgents NIPs                                  | `2d9b1463be6fb1ceac60c4bfabcb7b10f168d060` | SWP/PFI allocations only; no assignment in `39650-39659`                  |
| `nostr-protocol/registry-of-kinds` `schema.yaml` | `2483e752146d171524dcb10dffd06de2aa271bf3` | No assignment in `39650-39659`; nearest higher registered kind is `39701` |

Implementers MUST repeat all four checks before adoption or extension.

## Roles and signer map

| Role                 | Required authority                                                                                                     |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| client               | NIP-MKT requester and LSPS client; signs RFQ, Order, one Service Contract, client Status, Cancel, and Close            |
| LSP                  | NIP-MKT provider and LSPS server; signs Offering, Quote, one Service Contract, LSP Status, Cancel, and Close           |
| payer                | External payer in an LSPS2 flow; no market authority unless it separately signs a profile role                         |
| chain verifier       | Verifies funding, refund, close, confirmation, reorg, and replacement facts and issues the pinned native receipt       |
| Lightning verifier   | Verifies node, channel, forwarding, invoice, and payment facts and issues the pinned native receipt                    |
| reservation verifier | Verifies a named reserve proof class and issues no broader receipt                                                     |
| relay                | Validates public shapes and transports private records; has no channel, wallet, node, payment, or settlement authority |

A verifier is admitted only for the exact fact classes listed in the Quote.
The LSP cannot independently verify its own obligation merely by using a
second service key.

Eligibility may require a node connection, feature bits, channel-size policy,
public or private channel preference, coupon, or access token. Coupons,
tokens, node credentials, and private connection details stay private.

## Market identity, assets, and units

The market identity is an exact asset-ID pair:

- the requested service asset identifies `channel-capacity` or
  `jit-inbound-capacity`, Bitcoin network, and unit; and
- the payment asset identifies the Bitcoin or Lightning asset and network.

Bare `BTC`, `sat`, or `mainnet` strings are labels and cannot identify a
market. The Quote separately pins chain genesis identity and Lightning
network.

Amounts are canonical decimal strings matching `^(0|[1-9][0-9]*)$` and
bounded to an unsigned 64-bit integer. Each field declares `sat` or `msat` as
its atomic unit. JSON numbers are invalid.

A public Offering declares both service sides:

```json
{
  "market": {
    "base_asset_id": "<lightning-capacity-asset-id>",
    "quote_asset_id": "<bitcoin-payment-asset-id>"
  },
  "sides": {
    "channel-purchase": { "min": "50000", "max": "100000000" },
    "jit-inbound": { "min": "0", "max": "0" }
  }
}
```

`max: "0"` disables a side and requires `min: "0"`. Omission is invalid.
An enabled side requires `0 < min <= max`.

## Offering and Quote fields

An MKT-LSP Offering adds:

| Field                       | Rule                                                            |
| --------------------------- | --------------------------------------------------------------- |
| `lsp_node_id`               | Exact compressed secp256k1 node public key                      |
| `network_id`                | Collision-resistant Bitcoin chain identifier                    |
| `lsps`                      | Supported LSPS0/1/2 revisions and BOLT8 message ID `37913`      |
| `market`                    | Exact service/payment asset-ID pair and units                   |
| `sides`                     | Explicit channel-purchase and JIT capacity bounds               |
| `channel_types`             | Exact supported feature or channel-type identifiers             |
| `zero_conf_policy`          | `unsupported`, `client-policy`, or exact provider constraints   |
| `lease_bounds`              | Minimum and maximum duration in blocks                          |
| `payment_methods`           | `bolt11`, `bolt12`, and/or `onchain`                            |
| `custody_class`             | `a1-coordinated-hold` until the channel is independently usable |
| `reservation_proof_classes` | Supported classes from the list below                           |

The Quote pins:

- LSPS revision, method, canonical JSON-RPC request template, acceptable
  response schema, and response bounds;
- service type, network, node ID, channel type, announcement preference, and
  zero-confirmation policy;
- `lsp_balance_sat`, `client_balance_sat`, total channel capacity, or JIT
  payment bounds as decimal strings;
- required confirmations, funding-confirmation deadline in blocks,
  `channel_expiry_blocks` or JIT `min_lifetime`, and `to_self_delay` bounds;
- exact fee formula, minimum, maximum, fee asset, and rounding;
- payment methods, expiries, and refund route;
- reservation amount, expiry, proof class, evidence signer, and uniqueness
  key;
- funding-output constraints, finality, reorg, and replacement policy;
- trust model, irreversible boundary, custody dimensions, and exit package;
  and
- allowed Order selection fields.

The normalized `fee_bps` field is a canonical decimal string from `"0"`
through `"10000"` and is a maximum fill promise. When LSPS2 uses parts per
million, the Quote also preserves exact `proportional_ppm`, `min_fee_msat`,
and LSPS `promise`. The wallet verifies that the normalized promise and native
LSPS formula agree.

When an external price feed is used, the Quote pins exact URL, RFC 6901 value
pointer, optional timestamp pointer, canonical observed value, observation
time, maximum age, response SHA-256, and rounding. Both parties reproduce
those inputs before Order. A substitute feed is invalid. The feed has no
channel or settlement authority.

The pre-Order Quote MUST NOT claim an LSPS response, `order_id`, `promise`, or
SCID that does not yet exist. It commits to the request template, acceptable
response bounds, digest rules, and any selectable options. The post-Order
Service Contract binds the actual request, response, external IDs, promise,
and SCID reference.

### Allowed Order selection

An Order may select only fields explicitly marked selectable:

- one capacity amount within the Quote bounds;
- one channel type;
- one payment method;
- one confirmation count at or above the signed minimum; and
- public or private channel when both were quoted.

It cannot change node, network, fee formula, lifetime, funding deadline,
trust model, reservation proof, refund route, or finality policy.

## Reservation accounting

Every Quote declares `reservation: none`, `soft`, or `hard` under NIP-MKT and
one proof class:

| Proof class                | What it can prove                                                                                              |
| -------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `provider-signed`          | LSP promised capacity; no independent reserve proof                                                            |
| `channel-slot`             | Named LSP accounting slot is held; still an LSP claim                                                          |
| `funding-input-commitment` | Exact UTXO or funding input is committed under the pinned verifier policy                                      |
| `funding-output-observed`  | Exact channel funding output is visible under the pinned depth policy                                          |
| `covenant-reserve`         | Covenant-enforced minimum backs the reservation and the verifier checks template, amount, uniqueness, and exit |

A hard reservation requires one of the last three classes. Before Order, its
digest binds the Quote ID, amount, and expiry. Order admission consumes it
idempotently and the Service Contract binds the same artifact to the Order ID.
A provider-signed or channel-slot claim cannot be called hard. One reserve
outpoint or covenant commitment MUST NOT back overlapping live hard
reservations unless the covenant itself proves sufficient counted capacity.

Expiry releases only the named reservation. It does not cancel an LSPS order,
refund payment, close a channel, or settle a JIT forward.

## Custody disclosure

Every Quote states:

| Dimension              | Required disclosure                                                       |
| ---------------------- | ------------------------------------------------------------------------- |
| `funds_control`        | Client, LSP, funding output, and channel commitment control at each stage |
| `execution_control`    | Who can create funding, release a JIT payment, refund, or close           |
| `settlement_authority` | Bitcoin consensus and Lightning channel/payment state separately          |
| `reversibility`        | Zero-conf, replacement, reorg, payment, and close windows                 |
| `recourse`             | Enforceable refund, guarantee, provider promise, or none                  |
| `credential_exposure`  | Token or coupon audience, purpose, and retention                          |

The Quote also states maximum provider-controlled duration before a usable
channel or refund. Custody class is a disclosure, not proof of performance.

## LSPS bridge

The profile preserves LSPS0/1/2 as the execution protocol:

```json
{
  "source": {
    "protocol": "lsps1",
    "revision": "blip-51@<digest>",
    "method": "lsps1.create_order",
    "request_sha256": "<64-lower-hex>",
    "response_sha256": "<64-lower-hex>",
    "external_id_sha256": "<64-lower-hex>",
    "mapping_version": "mkt-lsp-v1"
  }
}
```

The external JSON-RPC message keeps its own bytes, request ID, error, and
authority. A translator lists dropped, defaulted, and ambiguous fields and
fails closed if it cannot preserve node identity, amount, fee, expiry,
promise, trust model, refund route, or state. An MKT Order never silently
creates an LSPS order; the provider adapter binds both IDs before its first
external effect.

## LSP Service Contract (`kind:39650`)

After the native LSPS order, promise, or JIT route exists, the client and LSP
each sign a separate immutable LSP Service Contract with one shared
`contract_sha256`. Both records are wrapped to both participants. A recovery
verifier receives the exact signed contracts through authorized disclosure;
it is not an unlisted gift-wrap recipient. Bare publication is invalid.
Payment, preimage release, and funding do not begin before both contracts
match.

Required tags are:

- unique 64-lower-hex `d`;
- one `session` tag containing the NIP-MKT session ID;
- `profile`, `mkt-lsp`, `1`;
- one `e` tag marked `quote`;
- one `e` tag marked `order`;
- exactly one `p` tag for the other participant, marked `requester` or
  `provider`;
- one `role` tag set to `requester` or `provider`;
- one `x` tag containing the shared contract SHA-256;
- fixed `alt` text `MKT-LSP service contract`.

A contract for an indicative Quote also requires one `e` tag marked `status`
for the LSP's accepted Status. The contract has no NIP-40 expiration because
both parties need it after the LSPS promise expires for recovery and loss
accounting.

Content uses the NIP-MKT envelope and adds:

```json
{
  "contract": {
    "quote_event_id": "<64-lower-hex>",
    "order_event_id": "<64-lower-hex>",
    "accepted_status_event_id": null,
    "service": "lsps1-channel-purchase",
    "lsps_request_sha256": "<64-lower-hex>",
    "lsps_response_sha256": "<64-lower-hex>",
    "external_effect_ids_sha256": "<64-lower-hex>",
    "reservation_sha256": "<64-lower-hex>",
    "funding_constraints_sha256": "<64-lower-hex>",
    "verifier_policy_sha256": "<64-lower-hex>",
    "recovery_package_sha256": "<64-lower-hex>"
  },
  "contract_sha256": "<64-lower-hex>",
  "signer_role": "provider"
}
```

`contract_sha256` is SHA-256 of the RFC 8785 serialization of the exact
`contract` object, and the `x` tag MUST equal it. `quote_event_id` and
`order_event_id` MUST equal the causal tags. `accepted_status_event_id` is the
LSP's accepted Status for an indicative Quote and JSON `null` for a firm
Quote. When present, it MUST equal the `status` causal tag; when it is `null`,
that tag is forbidden. The shared object commits to the LSPS request and response, node,
network, amounts, fee promise, reservation, external IDs, funding-output
constraints, trust model, finality policy, refund route, and recovery package.
The client record means those inputs passed its local policy. The LSP record
means it will fund or forward only under those inputs.

Digest preimages are exact: `lsps_request_sha256` and
`lsps_response_sha256` hash the unmodified UTF-8 JSON-RPC request and response
bytes; `external_effect_ids_sha256` hashes the RFC 8785 array of typed
external identifiers in execution order; and every remaining `*_sha256`
hashes the RFC 8785 serialization of the correspondingly named JSON object in
the accepted Quote or retained recovery package. The Quote defines each
object schema and media type. An absent preimage, a different serialization,
or a digest without its pinned schema is a contract mismatch.

The records contain only hashes and bounded public identifiers. A node
macaroon, wallet seed, channel backup secret, commitment secret, unreleased
preimage, coupon, access token, private key, or raw exit package is forbidden.
A changed contract requires a new Quote and Order; it cannot supersede
matching contracts after an external effect.

The route becomes `service-contract-bound` only when both events are present,
their roles are complementary, authors match the Order and Quote, and the
contract objects and digests are identical. A third party cannot countersign
for a participant. Status and evidence references use the exact contract event
IDs and shared digest.

Native evidence remains an exact artifact reference with issuer, receipt
type, digest, provenance, observation time, and verifier policy. An LSP-signed
`channel_ready` claim is not independent verification. A client marks it
verified only after the pinned node, funding outpoint, script, value, channel
parameters, and required depth pass locally or through the admitted verifier.

## Lifecycle and transitions

LSPS1 channel purchase:

```text
accepted -> reservation-held -> service-contract-pending
  -> service-contract-bound -> payment-required -> payment-observed
  -> funding-pending -> funding-output-observed
  -> channel-ready -> usable -> completed
```

LSPS2 JIT flow:

```text
accepted -> fee-parameters-pinned -> jit-route-issued
  -> service-contract-pending -> service-contract-bound
  -> incoming-htlc-observed -> funding-pending
  -> funding-output-observed -> jit-forward-committed
  -> jit-payment-settled -> usable -> completed
```

The profile extends Status with `reservation-held`, `fee-parameters-pinned`,
`jit-route-issued`, `service-contract-pending`, `service-contract-bound`,
`payment-required`, `payment-observed`, `incoming-htlc-observed`,
`funding-pending`, `funding-output-observed`, `channel-ready`,
`jit-forward-committed`, `jit-payment-settled`, and `usable`.

The exact signer and evidence law is:

| State or action                     | Allowed author | Required predecessor and evidence                                                 | Timer and timeout result                                          |
| ----------------------------------- | -------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `accepted`                          | LSP            | timely Order when Quote is indicative                                             | Quote expiry rejects late acceptance                              |
| `reservation-held`                  | LSP            | Order and exact reservation proof class                                           | reservation expiry releases capacity only                         |
| `fee-parameters-pinned`             | LSP            | LSPS2 response satisfying the Quote bounds                                        | promise expiry requires a new Quote/Order                         |
| `jit-route-issued`                  | LSP            | exact SCID or route-hint digest and promise                                       | route expiry does not release a preimage                          |
| `service-contract-pending`          | either party   | accepted Order and native response                                                | contract deadline leads to `failed` before payment or funding     |
| `service-contract-bound`            | either party   | both participant contracts and identical digest                                   | no timeout promotes it without both signatures                    |
| `payment-required`                  | LSP            | bound contract and exact invoice, offer, or on-chain instruction                  | payment expiry follows the enforceable refund policy              |
| `payment-observed`                  | LSP or client  | admitted payment receipt under the bound instruction                              | observation does not prove channel usability                      |
| `incoming-htlc-observed`            | LSP            | exact incoming HTLC reference under the bound promise                             | HTLC expiry follows Lightning, never a market assertion           |
| `funding-pending`                   | LSP            | bound contract and exact funding plan                                             | funding deadline starts the pinned refund path                    |
| `funding-output-observed`           | LSP or client  | admitted chain receipt for exact outpoint, script, value, and replacement lineage | reorg or replacement removes the observation                      |
| `channel-ready`                     | LSP            | LSP node claim tied to the exact channel and funding output                       | remains unverified until client policy passes                     |
| `jit-forward-committed`             | LSP            | funding output passes the trust model and the forward is irrevocably committed    | failure follows the retained HTLC/refund path                     |
| `jit-payment-settled`               | client         | wallet verifies the exact Lightning settlement under the bound payment hash       | ambiguous settlement closes `unresolved`                          |
| `usable`                            | client         | node verifies channel parameters, funding policy, and operational use             | later close or reorg moves the local projection to its rail state |
| `completed` / `refunded` / `failed` | either party   | exact terminal Lightning and Bitcoin artifacts for the claimed state              | evidence rollback removes a verified terminal projection          |
| Cancel `effective`                  | either party   | matching acceptance before payment/funding, or enforceable refund evidence        | expiry alone releases only the reservation                        |
| Close `completed/refunded`          | either party   | exact terminal Lightning and Bitcoin artifacts for that outcome                   | unavailable evidence closes `unresolved`                          |
| Close `cancelled`                   | either party   | effective Cancel and proof no payment, funding, or HTLC obligation remains        | a Cancel request alone is insufficient                            |
| Close `expired`                     | either party   | signed deadline and verified release of the reservation or payment hold           | expiry does not close or refund a funded channel                  |
| Close `failed/unresolved`           | either party   | exact rail failure or missing/conflicting evidence inventory and loss accounting  | no inferred refund, channel, or settlement                        |

A Status outside its row is retained as an unsupported claim and does not
advance state. Only the client may claim `usable`. An LSP's `channel-ready`
claim is an observation until verified. In LSPS2's `lsp-trusts-client` model,
the client does not release the payment preimage before the funding
transaction and output satisfy the Quote's policy. Version 1 does not treat
`client-trusts-lsp` as a coordinator-independent executable route.

Only the base states used in the table and the listed profile extensions are
admitted. Any other base or profile state is unsupported version 1.

Cancellation is effective before payment or external order creation under
the exact Quote rule. After payment, the refund or exit path governs. NIP-09,
expiry, a market Cancel, or an LSPS JSON-RPC error cannot undo a confirmed
channel or final Lightning payment.

Close outcomes are `completed`, `cancelled`, `expired`, `failed`, `refunded`,
or `unresolved`. Close lists reserved, paid, refunded, channel-locked,
client-spendable, LSP-spendable, and lost amounts separately.

## Finality, reorg, replacement, and exit

The client verifies:

- LSP node ID and negotiated channel feature bits;
- funding transaction, outpoint, script, value, and expected channel keys;
- minimum depth and zero-confirmation policy;
- funding deadline and channel lifetime;
- LSPS1 payment totals or LSPS2 fee formula and promise;
- JIT SCID or route-hint binding and incoming HTLC amount;
- `channel_ready` only after the funding output passes policy; and
- refund or unilateral-close transactions under their exact scripts and
  timelocks.

The Quote states whether an unconfirmed funding transaction may be replaced.
An admitted replacement preserves the exact output script and value and is
linked by evidence. Any other replacement invalidates the funding state. A
reorg rolls `funding-output-observed`, `channel-ready`, and dependent finality
back until the required depth is restored. A 0-conf route displays its full
replacement exposure.

Every LSPS order, promise, SCID request, payment instruction, funding
transaction, and refund is bound to the NIP-MKT Order event ID before its
first external effect. Replay returns the previous result and does not create
another LSPS order or channel.

## Coordinator-independent recovery

Before payment or incoming-HTLC release, the client persists:

- exact RFQ through Order and Status chain;
- LSPS request/response bytes and digests;
- node ID, peer addresses, network, channel keys and parameters;
- funding-output constraints, finality policy, and expected lifetime;
- refund route and authenticated direct peer route; and
- a wallet-native channel-monitor or pre-signed commitment/exit package.

Channel-monitor state, commitment transactions, spend keys, preimages, and
private backup material remain in encrypted wallet state. The market relay,
Immortal server, provider router, and public records receive only digests and
public-safe references.

With the market relay and coordination handler permanently gone, the client
uses BOLT8 and the external Bitcoin/Lightning rails directly. Before a channel
opens, an executable Quote provides either a payment hold that remains
refundable until funding verifies or an enforceable fully signed refund whose
script, outputs, timelock, and broadcast path are pinned in the Service
Contract. A provider promise or unspecified guarantee is not an exit. After a
channel opens, the wallet can use its channel monitor and commitment
transaction to close unilaterally.

A prepaid LSPS1 route with only an LSP promise to refund, or an LSPS2
`client-trusts-lsp` route where the client releases the preimage before it can
verify funding, fails with `mkt_lsp_no_independent_exit`. Such a route may be
displayed as rail-native best effort but MUST NOT be advertised as executable
MKT-LSP v1.

## Privacy and server-state boundary

Public records may contain node ID, public connection endpoints, network,
LSPS revisions, amount and lifetime ranges, payment-method names, channel
types, fee-policy summary, and custody class. Private endpoints, tokens,
coupons, invoices, addresses, SCIDs tied to a user, channel IDs, payment
hashes, and transaction plans stay private.

No market or server state contains seeds, NWC strings, node macaroons,
channel backup secrets, commitment secrets, spend keys, unreleased preimages,
claim/refund keys, signing nonces, or raw exit packages.

## Errors and loss states

| Code                                 | Meaning                                                         |
| ------------------------------------ | --------------------------------------------------------------- |
| `mkt_lsp_unsupported_version`        | Profile or LSPS revision is unsupported                         |
| `mkt_lsp_invalid_market`             | Asset, network, unit, side, or amount failed                    |
| `mkt_lsp_side_disabled`              | Selected side has `max: "0"`                                    |
| `mkt_lsp_invalid_node`               | Node key, network, or peer binding failed                       |
| `mkt_lsp_lsps_mismatch`              | LSPS method, request, response, promise, or external ID differs |
| `mkt_lsp_fee_mismatch`               | Native fee and normalized promise disagree                      |
| `mkt_lsp_price_source_mismatch`      | Feed URL, pointer, digest, or age differs                       |
| `mkt_lsp_reservation_mismatch`       | Reservation amount, proof, uniqueness, or expiry failed         |
| `mkt_lsp_hard_reserve_unproven`      | Hard reservation lacks an admitted proof class                  |
| `mkt_lsp_service_contract_mismatch`  | Participant contracts differ or omit a committed field          |
| `mkt_lsp_invalid_contract_signer`    | Contract signer is not the requester or provider                |
| `mkt_lsp_funding_mismatch`           | Funding outpoint, script, value, or key differs                 |
| `mkt_lsp_replacement_refused`        | Transaction replacement violates Quote policy                   |
| `mkt_lsp_invalid_transition`         | Signer or lifecycle transition is not admitted                  |
| `mkt_lsp_no_independent_exit`        | Route lacks a coordinator-independent refund or close path      |
| `mkt_lsp_idempotency_conflict`       | One logical ID was reused with different bytes                  |
| `mkt_lsp_custody_material_forbidden` | Market or server record contains custody material               |
| `mkt_lsp_settlement_overclaim`       | Reservation, payment, or channel claim exceeds evidence         |

Close preserves `missing_record`, `sequence_gap`, `signer_fork`,
`reservation-double-spent`, `payment-no-channel`, `funding-timeout`,
`funding-replaced`, `chain-reorg`, `channel-not-usable`, `lsp-unavailable`,
`refund-unavailable`, `unilateral-close-pending`, and
`evidence-unavailable`.

## Fixture manifest

An implementation MUST replay these exact case IDs:

| Case ID                                       | Expected result                                                      |
| --------------------------------------------- | -------------------------------------------------------------------- |
| `lsp-positive-lsps1-channel-purchase`         | Channel purchase reaches client-verified usable state                |
| `lsp-positive-lsps2-jit`                      | JIT flow withholds preimage until funding policy passes              |
| `lsp-positive-hard-funding-reserve`           | Exact funding commitment backs one reservation                       |
| `lsp-positive-covenant-reserve`               | Covenant amount, uniqueness, and exit verify                         |
| `lsp-positive-service-contract`               | Client and LSP bind one native service route before effects          |
| `lsp-positive-unilateral-close`               | Persisted channel monitor reaches terminal close without coordinator |
| `lsp-negative-disabled-side`                  | Zero-max side is rejected                                            |
| `lsp-negative-json-number-amount`             | Numeric capacity amount is rejected                                  |
| `lsp-negative-lsps-substitution`              | Changed LSPS response or promise is rejected                         |
| `lsp-negative-fee-over-promise`               | Native fee exceeds the signed normalized promise                     |
| `lsp-negative-substitute-feed`                | Alternate price URL or pointer is rejected                           |
| `lsp-negative-provider-claim-is-verification` | LSP claim cannot satisfy independent verifier policy                 |
| `lsp-negative-hard-provider-signed`           | Provider-signed reserve cannot be called hard                        |
| `lsp-negative-double-reservation`             | One reserve cannot back overlapping capacity twice                   |
| `lsp-negative-funding-output`                 | Wrong script, value, outpoint, or key is rejected                    |
| `lsp-negative-unadmitted-replacement`         | Replacement outside policy rolls state back                          |
| `lsp-negative-channel-ready-before-depth`     | Early channel-ready overclaim is rejected                            |
| `lsp-negative-client-trusts-lsp`              | Preimage-first JIT route is not executable v1                        |
| `lsp-negative-prepaid-no-refund`              | Prepaid channel with no enforceable refund is rejected               |
| `lsp-negative-custody-material`               | Macaroon, seed, preimage, or commitment secret is rejected           |
| `lsp-negative-service-contract-mismatch`      | Differing LSPS or recovery digest is rejected                        |
| `lsp-negative-contract-signer`                | Third party cannot sign a participant contract                       |
| `lsp-replay-same-contract`                    | Byte-identical replay returns prior result                           |
| `lsp-replay-contract-conflict`                | Changed bytes under one `d` fail closed                              |
| `lsp-fork-status`                             | Same-signer sequence fork remains visible                            |
| `lsp-expiry-reservation-only`                 | Expiry releases reserve but does not cancel or refund rail state     |
| `lsp-privacy-independent-wraps`               | Each signed contract is wrapped independently to required parties    |
| `lsp-recovery-market-coordinator-gone`        | Direct peer and rail path reaches completed, refunded, or exited     |
| `lsp-recovery-keyless-exit-executor`          | Persisted exit package runs without relay or LSP API                 |
| `lsp-loss-chain-reorg`                        | Funding finality rolls back and re-verifies                          |

The table is the normative version-1 case manifest. Adoption MUST publish one
canonical byte corpus containing every case, expected error, authority test
key, descriptor event ID, and corpus SHA-256 in the implementation contract
manifest. Implementations reference that corpus rather than generating local
variants and MUST NOT advertise `mkt-lsp/1` before it passes under the active
configuration.

## References

- [NIP-MKT](MKT.md)
- [bLIP-50 LSPS0](https://github.com/lightning/blips/blob/ca04f374d03001ddbed60ff109da58bd9c390c9a/blip-0050.md)
- [bLIP-51 LSPS1](https://github.com/lightning/blips/blob/ca04f374d03001ddbed60ff109da58bd9c390c9a/blip-0051.md)
- [bLIP-52 LSPS2](https://github.com/lightning/blips/blob/ca04f374d03001ddbed60ff109da58bd9c390c9a/blip-0052.md)
- [Market rails teardown](../teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)
- [tbDEX liquidity protocol teardown](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md)
