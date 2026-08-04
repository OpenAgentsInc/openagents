# NIP-MKT-P2P

## Peer-to-peer Bitcoin trades

`draft` `optional`

MKT-P2P version 1 is a profile of [NIP-MKT](MKT.md) for peer trades between
Bitcoin and a fiat or other reversible payment rail. It bridges NIP-69 and
Mostro-style order, hold-invoice, bond, cooperative-cancel, and dispute
semantics into the NIP-MKT negotiation spine. It does not create a second
public P2P order network.

NIP-69 orders and Mostro records remain canonical for their own meaning. A
profile record that references one proves only the reference and mapping. It
does not upgrade the source signature, escrow, reputation, payment, or dispute
authority.

## Profile identity and allocation

- Profile ID: `mkt-p2p`
- Version: `1`
- Parent: NIP-MKT v0.1
- Authority: the author pubkey of the selected `kind:39602` Profile
  Descriptor. A client MUST allowlist that exact key and pin the descriptor
  event ID and specification digest before treating the profile as executable.
- Specification digest: the descriptor's `x` tag over the exact bytes of this
  file. This draft does not designate a universal authority key or executable
  descriptor.

| Kind        | Name           | Publication                                       | Replacement law                   |
| ----------- | -------------- | ------------------------------------------------- | --------------------------------- |
| 39620       | P2P Resolution | private signed record in NIP-59 `kind:1059` wraps | unique `d`; immutable-by-contract |
| 39621-39629 | Reserved       | —                                                 | unallocated                       |

`kind:39620` is the minimum profile-specific kind. NIP-MKT Status and Close
are participant claims. A solver or arbiter's decision is a third-party
authority record and MUST NOT be mislabeled as a requester/provider Status or
Close. Offerings, RFQs, Quotes, Orders, participant Status records, Cancels,
and Closes continue to use NIP-MKT base kinds.

The kind extends NIP-MKT's supported private-kind set and uses the same signed
inner event, rumor, seal, per-recipient wrap, signer-equality, recipient gate,
32 KiB bound, duplicate-JSON rejection, replay, and idempotency rules. Clients
fail closed on unknown critical members.

### Collision review

The allocation was checked on 2026-08-04 against these exact revisions:

| Source                                           | Revision                                   | Result                                                                    |
| ------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------------- |
| Official NIPs                                    | `c53877571f96eb423661fc23c620d629d37b8f19` | No assignment in `39620-39629`                                            |
| Block Buzz NIPs                                  | `540b58920cef205b838da8be8442aae62bceaaa5` | No assignment in `39620-39629`                                            |
| OpenAgents NIPs                                  | `2d9b1463be6fb1ceac60c4bfabcb7b10f168d060` | SWP/PFI allocations only; no assignment in `39620-39629`                  |
| `nostr-protocol/registry-of-kinds` `schema.yaml` | `2483e752146d171524dcb10dffd06de2aa271bf3` | No assignment in `39620-39629`; nearest higher registered kind is `39701` |

Implementers MUST repeat all four checks before adopting or extending this
allocation.

## Roles, keys, and authority

| Role           | Required authority                                                                                         |
| -------------- | ---------------------------------------------------------------------------------------------------------- |
| maker          | Signs the Quote as the NIP-MKT provider and signs provider Status, Cancel, and Close records               |
| taker          | Signs the RFQ and Order as the NIP-MKT requester and signs requester Status, Cancel, and Close records     |
| buyer          | The party delivering the reversible payment; derived from the accepted `side`, never from pubkey position  |
| seller         | The party delivering Bitcoin; derived from the accepted `side`, never from pubkey position                 |
| coordinator    | Supplies the named escrow or hold-invoice service; has only the authority declared in the accepted Quote   |
| solver         | May sign a `kind:39620` decision when its key is in the pinned solver set and the decision is within scope |
| appeal arbiter | May supersede a solver decision only when its key, appeal scope, and deadline were pinned in the Quote     |
| rail verifier  | Verifies Lightning, Bitcoin, or payment-rail evidence under the exact policy pinned in the Quote           |
| relay          | Transports and validates visible shapes; has no escrow, dispute, payment, or settlement authority          |

Maker and taker SHOULD use a fresh trade key for each order. No profile record
may require a link from a trade key to a long-lived identity. A party MAY
disclose such a link by an exact signed reference, but absence is valid.

A public NIP-69 order under a trade key can be the discovery source. In that
case the RFQ uses the admitted `e` marker `evidence` and content field
`source.event_id` to identify the NIP-69 event. This profile does not extend
the base causal-marker vocabulary. It permits the NIP-MKT blind-capability RFQ
path. A base `kind:39601` Offering is optional. If used, it SHOULD also be
authored under a trade-scoped key and expire.

## Assets, market identity, and amounts

`base_asset_id` and `quote_asset_id` form the market identity. They MUST be
collision-resistant registry identifiers. Display tickers, symbols, and names
MUST NOT be used for matching. An ISO 4217 identifier is a registry identifier
for a fiat unit; the bare text `USD` is only a label.

Every amount is a canonical decimal string matching
`^(0|[1-9][0-9]*)$`. It expresses the atomic unit declared for its asset and
MUST fit an unsigned 64-bit integer in version 1. JSON numbers are invalid.
The Quote pins asset IDs, atomic units, and the exact amount on both legs.

A public capability or Offering contains both `buy` and `sell` side objects:

```json
{
  "market": {
    "base_asset_id": "<bitcoin-asset-id>",
    "quote_asset_id": "<registry-fiat-asset-id>"
  },
  "sides": {
    "buy": { "min": "0", "max": "0" },
    "sell": { "min": "10000", "max": "1000000" }
  }
}
```

`max: "0"` disables that side and requires `min: "0"`. Omission does not
disable a side. An enabled side requires `0 < min <= max`.

### Fixed and range orders

`amount_mode` is `fixed` or `range`:

- `fixed` requires one exact base amount in the RFQ;
- `range` requires inclusive `min_amount` and `max_amount`, and the taker
  selects one exact amount in the Order; and
- the selected amount MUST be inside both the source order range and the
  signed Quote range. The Quote event ID binds the permitted selection.

An Order may select only `base_amount`, one advertised payment method, and a
credential-presentation reference explicitly marked selectable by the Quote.
It cannot change side, asset, price rule, fee, bond, coordinator, solver set,
timers, or custody terms.

## Offering and Quote fields

A public MKT-P2P Offering adds these bounded fields:

| Field                   | Rule                                                                          |
| ----------------------- | ----------------------------------------------------------------------------- |
| `market`                | Exact asset-ID pair and atomic-unit identifiers                               |
| `sides`                 | Explicit `buy` and `sell` bounds with zero-max disable law                    |
| `payment_method_ids`    | At most 16 public method identifiers; no account details                      |
| `amount_mode`           | `fixed`, `range`, or `both`                                                   |
| `nip69`                 | Supported source revisions and kinds; version 1 recognizes order kind `38383` |
| `custody_class`         | `a1-coordinated-hold` for the Mostro-style path                               |
| `bond_policy`           | Public policy summary or `none`; exact terms still belong in the Quote        |
| `dispute_policy_digest` | SHA-256 of the complete solver, arbiter, appeal, and timer policy             |

The Quote contains all of the following:

- exact market, side, atomic units, and leg amounts;
- `price` as either a fixed canonical ratio or a pinned-feed rule;
- `fee_bps`, a canonical decimal string from `"0"` through `"10000"`, fee
  asset, rounding direction, and maximum fee amount;
- selected payment method and its reversible/finality class;
- coordinator key and external service identity;
- seller hold-invoice policy, invoice expiry, and payment-hash binding rule;
- bond terms for maker, taker, both, or neither;
- exact solver keys, selection rule, primary decision scope, appeal keys,
  appeal deadline, and conflict rule;
- timers for bond payment, seller funding, fiat payment, release, dispute,
  appeal, and recovery;
- custody dimensions and the coordinator-independent exit mode; and
- the exact evidence and finality policy for every transition.

`fee_bps` is a fill promise. The taker verifies the resulting fee against the
accepted Quote. A provider signature does not prove that the price or fee was
available elsewhere.

When price depends on an external feed, the Quote MUST pin:

```json
{
  "feed_url": "https://example.invalid/feed",
  "value_pointer": "/data/price",
  "timestamp_pointer": "/data/observed_at",
  "observed_value": "100000",
  "observed_at": "1785862700",
  "max_age_seconds": "30",
  "response_sha256": "<64-lower-hex>"
}
```

Pointers use RFC 6901. Both parties reproduce the observed value, response
digest, timestamp, age, and rounding before Order. A substitute URL or pointer
is a term violation. The feed supplies a price input and never acquires payment
or settlement authority.

### Bond terms

For each bonded side, the Quote pins:

- amount or percentage formula, atomic unit, and minimum;
- invoice type and payment-hash binding;
- lock and expiry deadline;
- the exact conditions that release or slash the bond;
- solver or arbiter authority for each slash condition;
- coordinator share, counterparty share, payout method, and claim deadline;
- the evidence needed to prove lock, release, slash, and payout; and
- loss accounting when a payout invoice is not supplied before its deadline.

A paid bond proves only the named bond was paid. It does not prove principal
escrow, fiat payment, or settlement.

### Capacity and reservation

An indicative Quote uses `reservation: none`. A firm Quote may use `soft` or
`hard`. A soft reservation pins the amount, expiry, and provider accounting
reference but remains a provider promise. A hard reservation additionally
requires an independently verifiable hold, escrow, funded output, or covenant
reserve that covers the exact Bitcoin principal and any promised bond payout.

The proof binds provider key, Quote ID, amount, asset, expiry, and uniqueness
key. One proof cannot back overlapping hard reservations unless it proves the
sum of all live amounts. Order admission consumes the exact reservation;
replay returns the same result. Expiry releases only unused capacity under the
Quote policy.

## NIP-69 and Mostro bridge

An RFQ or Quote that bridges a public order includes:

```json
{
  "source": {
    "protocol": "nip-69-mostro",
    "revision": "<exact-revision>",
    "event_id": "<64-lower-hex>",
    "source_sha256": "<64-lower-hex>",
    "mapping_version": "mkt-p2p-v1",
    "dropped_fields": [],
    "defaulted_fields": [],
    "ambiguous_fields": []
  }
}
```

The source event remains canonical. A translator MUST fail with
`mkt_p2p_unrepresentable_source` when signer authority, side, bounds, payment
method, hold terms, dispute authority, or terminal state cannot be represented
without inference. A projection never acquires the target event's signature.

Mostro public order kind `38383` and dispute kind `38386` are not replaced.
A `kind:39620` record may reference a Mostro dispute event by exact event ID,
but does not overwrite it.

## P2P Resolution (`kind:39620`)

The record is signed by one admitted solver or appeal arbiter and wrapped
separately to every party identified by a `p` tag, including the maker, taker,
coordinator, signer, and any required appeal arbiter. Bare publication is
invalid.

Required tags are:

- unique 64-lower-hex `d`;
- one `session` tag containing the NIP-MKT session ID;
- `profile`, `mkt-p2p`, `1`;
- one `e` tag marked `order`;
- for an appeal, exactly one `e` tag marked `previous` for the prior
  Resolution;
- one `p` tag each marked `maker`, `taker`, and `coordinator`, plus one for
  the author and every nonauthor recipient marked `solver` or
  `appeal-arbiter` as applicable;
- one `role` tag set to `solver` or `appeal-arbiter`;
- `expiration` when the decision or appeal window is bounded; and
- fixed `alt` text `MKT-P2P resolution`.

Content uses the NIP-MKT envelope and adds:

```json
{
  "resolution": {
    "previous_resolution_event_id": null,
    "decision": "release-to-buyer",
    "scope": "principal",
    "reason": "fiat-payment-verified",
    "effective_after": "1785862800",
    "appeal_deadline": "1785866400",
    "policy_sha256": "<64-lower-hex>",
    "evidence": [
      {
        "ref": "<event-id-or-uri>",
        "sha256": "<64-lower-hex>",
        "provenance": "verified"
      }
    ]
  },
  "loss": []
}
```

This profile extends the private recipient-role vocabulary with `maker`,
`taker`, `coordinator`, `solver`, and `appeal-arbiter`. A wrap recipient MUST
have exactly one matching role-marked `p` tag in the signed inner event. The
eight-`p` base bound still applies; a Quote requiring more recipients is not
executable version 1.

Allowed decisions are `release-to-buyer`, `refund-to-seller`,
`cooperative-cancel`, `slash-maker-bond`, `slash-taker-bond`, `dismissed`,
and `unresolved`. Scope is `principal`, `bond`, or `both`. For an initial
decision, `previous_resolution_event_id` is JSON `null` and no `previous` tag
is allowed. For an appeal, it is the exact prior Resolution event ID and MUST
equal the `previous` tag. A resolution MUST match the policy digest in the
accepted Quote and remain within the signer's scope. An appeal creates a new
`kind:39620` with a `previous` reference. It never changes prior bytes.

A Resolution is a signed decision. Settlement requires the profile's external
hold-invoice, Lightning payment, Bitcoin transaction, or other rail proof.

## Lifecycle, timers, and cancellation

The version 1 participant state machine is:

```text
RFQ -> Quote -> Order
  -> accepted
  -> bond-required -> bond-locked
  -> seller-funding-required -> seller-funding-locked
  -> fiat-payment-pending -> fiat-sent
  -> release-pending
  -> settlement_pending
  -> completed
```

Profiles extend NIP-MKT Status with these exact states:
`bond-required`, `bond-locked`, `seller-funding-required`,
`seller-funding-locked`, `fiat-payment-pending`, `fiat-sent`,
`release-pending`, `solver-pending`, `solver-taken`, and `appeal-pending`.

The exact signer and evidence law is:

| State or action            | Allowed author                 | Required predecessor and evidence                                                   | Timer and timeout result                                                    |
| -------------------------- | ------------------------------ | ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `accepted`                 | maker                          | timely Order; provider acceptance when Quote is indicative                          | Quote expiry rejects a late acceptance                                      |
| `bond-required`            | maker                          | `accepted`; exact bond request bound to Order                                       | bond deadline leads to `failed` or effective pre-fiat cancellation          |
| `bond-locked`              | bonded participant             | `bond-required`; admitted rail receipt for exact bond                               | bond expiry follows the signed bond refund/slash policy                     |
| `seller-funding-required`  | maker                          | required bonds verified                                                             | funding deadline leads to `failed` or effective pre-fiat cancellation       |
| `seller-funding-locked`    | seller                         | exact hold or script receipt verified under Quote policy                            | hold timeout uses the pre-signed or rail-native refund path                 |
| `fiat-payment-pending`     | seller                         | seller funding verified; buyer recovery package acknowledged                        | payment deadline permits the pinned pre-fiat recovery path                  |
| `fiat-sent`                | buyer                          | exact payment reference and buyer claim; verification remains a separate provenance | chargeback/finality timer remains open                                      |
| `release-pending`          | seller                         | seller acknowledges the buyer claim or admitted payment verifier confirms it        | release deadline advances to `disputed`, never to settlement by time alone  |
| `settlement_pending`       | maker or taker                 | release transaction or Lightning settlement reference                               | rail timeout advances to `failed`, `refunded`, or `unresolved` by evidence  |
| `completed`                | maker or taker                 | exact Bitcoin or Lightning finality receipt and required fiat finality              | a reorg or chargeback rolls the local verified projection back              |
| `disputed`                 | maker or taker                 | prior nonterminal state and signed dispute reason                                   | solver-take deadline advances to `unresolved`                               |
| `solver-pending`           | maker or taker                 | `disputed`; exact pinned solver set                                                 | solver deadline advances to `unresolved`                                    |
| `solver-taken`             | maker or taker                 | `solver-pending`; exact solver acceptance evidence within scope                     | decision deadline advances to `unresolved`                                  |
| `appeal-pending`           | maker or taker                 | timely appeal referencing a Resolution                                              | appeal deadline makes the prior decision final only under the pinned policy |
| Cancel `effective`         | both parties, or timeout owner | matching Cancel acceptance, or verified unilateral timeout condition                | unavailable recovery closes `unresolved`                                    |
| Close `completed/refunded` | maker or taker                 | terminal rail evidence for that signer's claimed outcome                            | no time-based promotion                                                     |
| Close `cancelled`          | maker or taker                 | effective Cancel and proof no irreversible leg remains                              | a Cancel request alone is insufficient                                      |
| Close `expired`            | maker or taker                 | signed deadline plus verified release of every hold and bond                        | expiry alone is insufficient                                                |
| Close `disputed`           | maker or taker                 | unresolved dispute or conflicting valid Resolution fork                             | remains disputed until the pinned appeal rule terminates                    |
| Close `failed/unresolved`  | maker or taker                 | exact failure or missing/conflicting evidence inventory and loss accounting         | no inferred refund or settlement                                            |

A Status authored by a role outside its row is retained as an unsupported
claim and does not advance state. Only the bonded party can claim
`bond-locked`, only the buyer can claim `fiat-sent`, and neither claim proves
the external fact without the Quote's admitted evidence.

Only the base states used in the table and the listed profile extensions are
admitted. Any other base or profile state is unsupported version 1.

Before `fiat-sent`, a participant may request cancellation. It becomes
effective when both parties accept, or when the Quote gives one signer a
timeout right and the external hold or bond has reached its verified recovery
condition. After `fiat-sent`, cancellation requires the signed dispute policy
and cannot erase the payment.

A dispute advances through `disputed -> solver-pending -> solver-taken`, then
to a Resolution. An appeal is allowed only before the pinned deadline. Two
valid conflicting decisions at one authority level are retained as a fork and
close `unresolved` unless the pinned conflict rule identifies a higher
authority.

Expiry can release a reservation or let a hold invoice expire. It never
asserts fiat payment, releases Bitcoin, slashes a bond, signs a resolution, or
settles.

Terminal Close outcomes are `completed`, `cancelled`, `expired`, `refunded`,
`disputed`, `failed`, or `unresolved`. Close lists principal, fee, and each bond
separately with `expected`, `verified`, `settled`, and `lost` amounts.

## Custody and coordinator-independent recovery

Version 1 has custody class A1. The Quote discloses these dimensions:

| Dimension              | Required value                                                                   |
| ---------------------- | -------------------------------------------------------------------------------- |
| `funds_control`        | Buyer, seller, coordinator, and bond controller for each stage                   |
| `execution_control`    | Exact party or threshold that can settle or cancel the hold                      |
| `settlement_authority` | Lightning, Bitcoin, reversible payment rail, and any escrow authority separately |
| `reversibility`        | Per leg, including chargeback window                                             |
| `recourse`             | Solver, appeal, bond, legal, or none                                             |
| `credential_exposure`  | Data class, audience, purpose, and retention                                     |

Before the buyer may send fiat, both parties MUST persist a recovery package
containing the signed RFQ through Order, current Status chain, exact invoice
and payment-hash metadata, hold expiry, direct counterparty route, solver and
appeal set, cooperative cancel/release templates, and rail-verification
policy. The package contains no invoice preimage, wallet seed, macaroon,
account credential, or private claim key.

The Quote MUST name one coordinator-independent terminal primitive and commit
to its exact script, participant keys, threshold, timelocks, transaction
templates, signature-sighash policy, artifact digests, and verifier:

- `two-party-presigned`: a two-party Bitcoin or Lightning contract has a
  cooperative completion path and a fully signed timeout refund retained by
  the funding owner before fiat moves. The authenticated direct channel
  carries only the already committed completion signatures and rail messages;
- `threshold-script`: the external output is controlled by the disclosed
  participant and arbiter key set under an exact threshold script. The
  recovery package contains the unsigned completion/refund templates, every
  already available signature, the Resolution-to-spend mapping, and the
  direct endpoint for each required cosigner; or
- `timeout-unilateral`: before fiat moves, a fully signed rail refund returns
  every locked asset and bond to its owner at the pinned timeout. This mode
  does not permit the buyer to send fiat.

The recovery verifier reconstructs the exact external spend from the retained
artifacts, checks every output and timelock against the Quote, and submits it
directly to the rail. Missing signatures, a coordinator-held key, or a refund
that still requires a provider API makes the route non-executable.

A native Mostro hold invoice whose release or refund remains controlled by the
Mostro coordinator is bridgeable for discovery and history, but is not an
executable MKT-P2P v1 route. It cannot be relabeled `two-party-presigned`.

A route that depends on one coordinator after fiat moves and has no direct or
threshold terminal path is not executable MKT-P2P v1. A client MUST refuse it
with `mkt_p2p_no_independent_exit`. If the external fiat rail itself fails or
charges back, the client records that external loss; a market record cannot
repair it.

## Privacy and PII

Public records may contain asset IDs, side bounds, public payment-method IDs,
bond-policy summaries, custody class, and policy digests. They MUST NOT contain
names, phone numbers, physical locations, bank or mobile-money account data,
payment references, invoices, credential presentations, trade-key linkage,
IP addresses, private relay URLs, or dispute evidence.

Referencing a public NIP-69 order deliberately links its public `name`,
geohash, source URL, trade key, amount range, and timing metadata to the MKT
session. Clients display that leak before creating the RFQ. A client that
cannot accept the link uses a private Offering or declines the route.

Private records minimize those fields and use purpose-bound references where
possible. Credential presentations bind audience, RFQ or Order ID, purpose,
expiry, and digest. Reusable credentials and bearer material stay outside
Nostr. Clients SHOULD use per-trade keys, independent wraps, timing jitter,
padding, and short retention.

## Rail verification and finality

The Quote selects exact adapters and policies:

- BOLT11 signature, network, amount, expiry, payment hash, and hold state;
- Lightning payment or refund state from an independent node or proof source;
- Bitcoin transaction, output, confirmation depth, reorg, and replacement
  policy when an on-chain leg is used;
- reversible-payment evidence source, staleness bound, chargeback window, and
  whether it is an observation, guarantee, or settlement authority; and
- bond lock, release, slash, and payout evidence.

Funding, `fiat-sent`, Resolution, and Close references use provenance labels
`pledged`, `observed`, `verified`, `paid`, `refunded`, or `settled`. A client
never infers a stronger label. Replacement or reorg invalidates dependent
states until the exact policy is satisfied again.

Every external coordinator order, hold invoice, bond invoice, dispute, and
payout ID is bound to the NIP-MKT Order event ID before its first effect.
Replaying an Order returns the prior result and MUST NOT create another hold,
bond, or payment instruction.

## Errors and loss states

Implementations use these machine-readable errors:

| Code                              | Meaning                                                   |
| --------------------------------- | --------------------------------------------------------- |
| `mkt_p2p_unsupported_version`     | Profile revision is not supported                         |
| `mkt_p2p_invalid_market`          | Asset identity, unit, side, or amount law failed          |
| `mkt_p2p_side_disabled`           | Selected side has `max: "0"`                              |
| `mkt_p2p_range_violation`         | Selected amount is outside signed bounds                  |
| `mkt_p2p_price_source_mismatch`   | Feed URL, pointer, digest, or age differs                 |
| `mkt_p2p_invalid_nip69_reference` | Source event or mapping is invalid                        |
| `mkt_p2p_unrepresentable_source`  | Source authority or state cannot be preserved             |
| `mkt_p2p_bond_mismatch`           | Bond amount, invoice, authority, or deadline differs      |
| `mkt_p2p_invalid_transition`      | Signer or transition is not admitted                      |
| `mkt_p2p_invalid_resolution`      | Solver, policy, scope, evidence, or appeal failed         |
| `mkt_p2p_no_independent_exit`     | Quote lacks a valid coordinator-independent terminal path |
| `mkt_p2p_idempotency_conflict`    | One logical ID was reused with different bytes            |
| `mkt_p2p_private_data_public`     | Public record contains a forbidden private field          |
| `mkt_p2p_evidence_mismatch`       | Evidence does not prove the claimed rail fact             |

Close records preserve `missing_record`, `sequence_gap`, `signer_fork`,
`resolution_fork`, `coordinator_unavailable`, `rail_unavailable`,
`chargeback_pending`, `bond_unpaid`, `payout_forfeited`, and
`evidence_unavailable` loss states. Current UI or coordinator database state
never fills them.

## Fixture manifest

An implementation of version 1 MUST replay these exact fixture case IDs:

| Case ID                                     | Expected result                                                    |
| ------------------------------------------- | ------------------------------------------------------------------ |
| `p2p-positive-fixed-complete`               | Fixed order completes with verified hold and payment evidence      |
| `p2p-positive-range-selection`              | Order selects one amount inside both signed ranges                 |
| `p2p-positive-nip69-reference`              | Exact kind-38383 source is preserved without signature upgrade     |
| `p2p-positive-bond-release`                 | Both bonds release under the signed policy                         |
| `p2p-positive-cooperative-cancel`           | Both parties cancel before fiat payment                            |
| `p2p-positive-dispute-resolution`           | Admitted solver signs `kind:39620` within scope                    |
| `p2p-positive-appeal-supersession`          | Admitted appeal arbiter references the prior decision              |
| `p2p-positive-per-trade-keys`               | Two sessions require no public identity linkage                    |
| `p2p-negative-disabled-side`                | Zero-max side is rejected                                          |
| `p2p-negative-range-outside-quote`          | Out-of-range Order is rejected                                     |
| `p2p-negative-substitute-feed`              | Alternate feed URL or pointer is rejected                          |
| `p2p-negative-nip69-silent-upgrade`         | Projection that invents target authority is rejected               |
| `p2p-negative-bond-slash-authority`         | Unauthorized slash decision is rejected                            |
| `p2p-negative-resolution-scope`             | Solver decision outside pinned scope is rejected                   |
| `p2p-negative-fiat-sent-signer`             | Non-buyer fiat claim is rejected                                   |
| `p2p-negative-public-pii`                   | Account, invoice, or credential data in public content is rejected |
| `p2p-negative-no-independent-exit`          | Post-fiat coordinator-only route is rejected                       |
| `p2p-replay-same-resolution`                | Byte-identical replay returns the prior result                     |
| `p2p-replay-resolution-conflict`            | Changed bytes under one `d` fail closed                            |
| `p2p-fork-participant-status`               | Same-signer sequence fork retains both records                     |
| `p2p-fork-resolution`                       | Conflicting equal-authority decisions close unresolved             |
| `p2p-expiry-bond-timeout`                   | Expiry applies only the quoted bond recovery rule                  |
| `p2p-privacy-independent-wraps`             | Maker, taker, and arbiter wraps contain the same signed record     |
| `p2p-recovery-coordinator-gone-before-fiat` | Timeout returns holds and bonds with no coordinator                |
| `p2p-recovery-coordinator-gone-after-fiat`  | Direct or threshold path reaches completed or refunded             |
| `p2p-loss-external-chargeback`              | Chargeback remains an explicit external loss                       |

The table is the normative version-1 case manifest. Adoption MUST publish one
canonical byte corpus containing every case, expected error code, authority
test key, descriptor event ID, and corpus SHA-256 in the implementation
contract manifest. Implementations reference that corpus rather than
generating local variants and MUST NOT advertise `mkt-p2p/1` before it passes
under the active configuration.

## References

- [NIP-MKT](MKT.md)
- Official NIP-69 and NIP-59
- [Market rails teardown](../teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)
- [tbDEX liquidity protocol teardown](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md)
