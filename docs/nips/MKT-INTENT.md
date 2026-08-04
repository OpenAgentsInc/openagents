# NIP-MKT-INTENT

## Maker-funded covenant intents

`draft` `optional`

MKT-INTENT version 1 defines a standing, maker-funded offer that any filler
may execute without first negotiating with the maker. The external rail
enforces the terms with a covenant or equivalent spend constraint. This is the
Arkade solver market shape.

The profile is separate from MKT-P2P. A peer trade has identified parties,
negotiated terms, and often escrow or dispute authority. A covenant intent has
one maker, an unknown future filler, zero required interaction, and rail-
enforced terms. Folding it into MKT-P2P would either invent a counterparty
before one exists or weaken the P2P signer and dispute model.

MKT-INTENT uses the NIP-MKT profile descriptor and common vocabulary, but it
does not use RFQ, Quote, or Order. A `kind:39660` Covenant Intent is neither a
Quote nor an Order. The fill transaction remains canonical for execution.

## Profile identity and allocation

- Profile ID: `mkt-intent`
- Version: `1`
- Parent vocabulary: NIP-MKT v0.1
- Authority: the author pubkey of the selected `kind:39602` Profile
  Descriptor. Clients MUST allowlist that key and pin the descriptor event ID
  and specification digest.
- Specification digest: the descriptor's `x` tag over the exact bytes of this
  file. This draft designates no universal profile authority.

| Kind        | Name            | Publication | Replacement law                    |
| ----------- | --------------- | ----------- | ---------------------------------- |
| 39660       | Covenant Intent | public      | derived `d`; immutable-by-contract |
| 39661-39669 | Reserved        | —           | unallocated                        |

`kind:39660` is the minimum artifact for this market shape. A base Offering
advertises a provider capability and a Quote answers an RFQ. Neither safely
represents already funded, any-filler, zero-interactivity terms. Fill and
settlement do not require a new Nostr kind in version 1; they remain external
rail facts. NIP-MKT `kind:39603` requires an exact private Close and therefore
is not compatible with this no-Order profile. Version 1 defines no public
receipt kind.

### Collision review

The allocation was checked on 2026-08-04 against these exact revisions:

| Source                                           | Revision                                   | Result                                                                    |
| ------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------------- |
| Official NIPs                                    | `c53877571f96eb423661fc23c620d629d37b8f19` | No assignment in `39660-39669`                                            |
| Block Buzz NIPs                                  | `540b58920cef205b838da8be8442aae62bceaaa5` | No assignment in `39660-39669`                                            |
| OpenAgents NIPs                                  | `2d9b1463be6fb1ceac60c4bfabcb7b10f168d060` | SWP/PFI allocations only; no assignment in `39660-39669`                  |
| `nostr-protocol/registry-of-kinds` `schema.yaml` | `2483e752146d171524dcb10dffd06de2aa271bf3` | No assignment in `39660-39669`; nearest higher registered kind is `39701` |

Implementers MUST repeat all four checks before adoption or extension.

## Roles and authority

| Role                | Required authority                                                                                                      |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| maker               | Funds the constrained rail object and signs the Covenant Intent                                                         |
| filler              | Selects an unspent valid intent and constructs a conforming external fill; no pre-fill Nostr signature is required      |
| rail verifier       | Independently verifies funding, covenant, fill, finality, reorg, replacement, and refund facts                          |
| price-source signer | Supplies a price input only when the covenant or verifier policy names it; has no settlement authority                  |
| relay               | Stores and serves the public intent under bounded validation; has no reservation, wallet, fill, or settlement authority |

No coordinator, solver assignment, escrow, credential, reputation, or dispute
role is implied. A solver may act as a filler, but it receives no special
authority.

The maker signature proves the event bytes. The rail proves whether the
funding object exists, is unspent, commits to the terms, and can be filled or
refunded.

### Quote and Order non-applicability

There is no Quote, allowed Order selection, or participant reservation ledger
in version 1. The maker selects every term before funding. A filler either
accepts the complete public intent by constructing a conforming rail spend or
does nothing. Relabeling the intent as a Quote or the fill as an Order fails
closed.

## Assets, amounts, sides, and price

`base_asset_id` and `quote_asset_id` form the market identity. Each is a
collision-resistant registry identifier. Tickers and display names are labels
and MUST NOT be used for matching or price calculation.

Every amount is a canonical decimal string matching
`^(0|[1-9][0-9]*)$`, denominated in the declared atomic unit, and bounded to
an unsigned 64-bit integer in version 1. JSON numbers are invalid.

A provider capability head that advertises intent support includes both
`make-buy` and `make-sell` sides. `max: "0"` disables a side and requires
`min: "0"`; omission does not disable it. A Covenant Intent itself represents
one enabled side and requires positive funded input and minimum output.

Version 1 supports one complete fill. Partial fills are invalid. A new amount,
price rule, fee, expiry, or funding object requires a new intent and `d`.

### Fees and price feeds

`fee_bps` is a canonical decimal string from `"0"` through `"10000"`, with
fee asset, rounding direction, and maximum fee amount. It is a fill promise
that both the filler and rail verifier check against the fill outputs.

Price is one of:

- `fixed-ratio`, with canonical integer numerator and denominator; or
- `pinned-feed`, with exact HTTPS URL, RFC 6901 `value_pointer`, optional
  `timestamp_pointer`, price-source public key, signature scheme and domain,
  canonical signed assertion schema, observation ID, canonical observed value,
  observation time, maximum age, response digest rule, integer scale, and
  rounding.

A substitute URL, pointer, signer, signature domain, or observation is
invalid. The funding constraint commits to the signer and assertion schema,
and the fill carries the exact signed assertion bytes. The rail adapter MUST
verify that the spend constraint authenticates that assertion and enforces the
derived minimum output. A rail that cannot enforce both conditions does not
support `pinned-feed` in version 1. If the feed is unavailable or stale, the
intent is unfillable until a valid observation exists or the maker uses the
refund path. The feed supplies an input and never settles the trade.

## Covenant Intent (`kind:39660`)

The event uses a derived 64-lower-hex `d`. It equals SHA-256 of the RFC 8785
serialization of the complete content object with `intent_id` omitted. The
published `intent_id` then equals that digest. A changed term or funding object
therefore requires a new `d`; a relay-selected replacement under the old `d`
fails derivation before admission. Reusing `(pubkey, kind, d)` with changed
bytes is also an idempotency conflict. The event is public and bounded to 16
KiB content and 64 tags.

Required tags are:

- unique `d`;
- `profile`, `mkt-intent`, `1`;
- one `asset` tag marked `base` and one marked `quote`;
- one `side` tag set to `make-buy` or `make-sell`;
- one `chain` tag with the exact network identifier;
- one `x` tag containing the covenant-template or constraint digest;
- NIP-40 `expiration` matching the refund boundary; and
- fixed `alt` text `MKT-INTENT covenant offer`.

Content uses this envelope:

```json
{
  "schema": "openagents.mkt.v1",
  "profile": "mkt-intent",
  "profile_version": 1,
  "intent_id": "<64-lower-hex>",
  "market": {
    "base_asset_id": "<asset-id>",
    "quote_asset_id": "<asset-id>",
    "side": "make-sell"
  },
  "terms": {
    "input_amount": "100000",
    "minimum_output_amount": "99000",
    "fee_bps": "10",
    "price": {
      "mode": "fixed-ratio",
      "numerator": "99",
      "denominator": "100"
    },
    "partial_fills": false
  },
  "funding": {
    "rail": "<rail-id>",
    "network_id": "<network-id>",
    "object_ref": "<outpoint-or-rail-ref>",
    "object_sha256": "<64-lower-hex>",
    "constraint_template": "<template-id>",
    "constraint_sha256": "<64-lower-hex>",
    "refund_after": "1785866400"
  },
  "verification": {
    "adapter": "<adapter-id-and-version>",
    "funding_policy": "<policy-id>",
    "finality_policy": "<policy-id>",
    "replacement_policy": "none"
  },
  "recovery": {
    "exit_package_sha256": "<64-lower-hex>",
    "executor_profile": "keyless-esplora-v1"
  },
  "critical": []
}
```

`intent_id` MUST equal the `d` tag and the derived digest. Tags and content
MUST agree. Unknown critical fields fail closed.

The funding object MUST identify one exact unspent rail object and commit to
the same constraint template, assets, amounts, fee, price rule, expiry, maker
refund path, and filler output rules. One funding object cannot back two live
intents. A relay may reject an obvious duplicate reference, but only the rail
verifier proves uniqueness and spend state.

## Fill rules

A filler performs these steps locally:

1. Verify the event ID, maker signature, profile descriptor, expiry, bounds,
   and critical fields.
2. Resolve the exact funding object through the pinned adapter.
3. Verify network, assets, value, constraint template, output rules, refund
   path, unspent state, and uniqueness.
4. Evaluate the fixed price or the exact pinned feed under the staleness and
   rounding rules.
5. Construct a fill transaction or rail action satisfying every constraint.
6. Verify the completed transaction locally before signing or broadcasting.
7. Broadcast through a wallet or rail adapter outside the relay.
8. Track replacement, conflict, confirmation, reorg, and finality under the
   intent policy.

No RFQ, Quote, Order, Status, or Cancel is required. A relay `OK`, an intent
event, a mempool transaction, or a solver claim does not prove finality.

The fill transaction ID is the external idempotency key. Repeated broadcast of
the same bytes is replay. A different transaction spending the same funding
object is a conflict resolved only by the rail. An implementation MUST NOT
construct a second fill after it observes the object spent unless the first
spend is invalidated by the pinned reorg/replacement policy.

## Reservation and custody

The verified funding object is a `hard` reservation with proof class
`covenant-reserve`. The verifier checks amount, template, unique backing,
expiry, fill path, and maker refund path. A maker signature without a funded
object is not a reservation.

Custody dimensions are:

| Dimension              | Version 1 rule                                                                                            |
| ---------------------- | --------------------------------------------------------------------------------------------------------- |
| `funds_control`        | Maker before funding; covenant-constrained rail object after funding; filler controls only its own inputs |
| `execution_control`    | Any filler can exercise the fill path; maker can exercise refund only after the pinned boundary           |
| `settlement_authority` | Exact external rail and verifier policy                                                                   |
| `reversibility`        | Explicit mempool, replacement, confirmation, and reorg policy                                             |
| `recourse`             | Rail-enforced fill/refund; no profile dispute authority                                                   |
| `credential_exposure`  | None in version 1                                                                                         |

The relay, descriptor authority, price feed, registry, and solver never obtain
spend authority.

## Lifecycle, expiry, and terminal outcomes

The local observed state machine is:

```text
intent-observed -> funding-verified -> fill-constructed
  -> fill-broadcast -> fill-confirming -> completed
```

Branches are:

```text
funding-verified -> expired -> refund-broadcast -> refunded
fill-broadcast -> conflicting-spend -> failed-or-refunded
fill-confirming -> reorged -> fill-confirming-or-refunded
```

These are wallet projections over the event and external rail. They are not
new signed market messages.

NIP-09 deletion, a new intent, a base Cancel, relay removal, or maker silence
cannot cancel a funded intent. Before expiry, only an admitted rail spend can
consume it. After expiry, the maker uses the refund path. A profile has no
subjective dispute process because the rail decides whether a spend satisfies
the constraint. Bugs, oracle failures, and unavailable data remain explicit
losses.

Terminal outcomes are `completed`, `refunded`, `expired-unspent`, `failed`, or
`unresolved`. They are wallet projections over exact rail artifacts. Version 1
does not project them into a Nostr receipt.

## Finality, reorgs, and replacement

The intent pins:

- rail and network identifiers;
- funding-object lookup and proof algorithm;
- covenant or constraint-template verifier;
- asset and amount verifier;
- fill-transaction structural verifier;
- minimum confirmation depth;
- mempool acceptance posture;
- replacement policy; and
- reorg rollback and re-confirmation rules.

Version 1 defaults to `replacement_policy: none`. A funding or fill
replacement is valid only if the external rail makes it the canonical spend
and the pinned policy admits its exact output constraints. Any reorg removes
dependent finality until reverified. A cached projection is never stronger
than the current rail proof.

## Coordinator-independent recovery

Before the maker funds the intent, it produces and verifies an exit package
that contains the public intent bytes, funding transaction or object,
constraint proof, fully signed refund transaction, timelock, adapter version,
and public chain lookup configuration. `keyless-esplora-v1` is valid only when
the package contains every signature and witness needed after the timelock. A
static executor uses only that retained package and an Esplora-compatible
endpoint or equivalent public rail reader.

The maker's private key stays outside the package and outside market/server
state. A rail that requires a new maker signature at refund time does not
support `keyless-esplora-v1`; a signer-assisted executor is outside version 1.

The filler needs only the public intent and external rail. If the relay,
registry, market handler, maker API, and every solver disappear, the filler
can still verify and fill an unexpired intent, and the maker can still refund
an expired one. That is the version 1 doomsday drill.

## Privacy and public leakage

The intent deliberately reveals maker pubkey, market, side, amount, price
rule, fee, funding object, expiry, constraint template, and verifier policy.
This leaks inventory and strategy. Users who cannot accept that leak MUST NOT
use this public profile.

The event MUST NOT contain identity documents, payment accounts, private
endpoints, wallet descriptors, seeds, preimages, signing nonces, claim/refund
private keys, macaroons, raw recovery secrets, or bearer credentials. The
public exit-package digest cannot expose private refund material.

## Errors and loss states

| Code                                  | Meaning                                                        |
| ------------------------------------- | -------------------------------------------------------------- |
| `mkt_intent_unsupported_version`      | Profile or adapter revision is unsupported                     |
| `mkt_intent_invalid_market`           | Asset identity, unit, side, or amount failed                   |
| `mkt_intent_side_disabled`            | Capability head disables the selected side                     |
| `mkt_intent_partial_fill_unsupported` | Version 1 intent permits only complete fill                    |
| `mkt_intent_price_source_mismatch`    | Feed signer, assertion, pointer, digest, scale, or age differs |
| `mkt_intent_funding_missing`          | Named funding object does not exist                            |
| `mkt_intent_funding_spent`            | Funding object is already spent                                |
| `mkt_intent_funding_mismatch`         | Value, asset, template, or refund path differs                 |
| `mkt_intent_duplicate_reserve`        | One funding object backs multiple live intents                 |
| `mkt_intent_expired`                  | Fill was attempted after refund boundary                       |
| `mkt_intent_fill_mismatch`            | Constructed fill violates an output or amount constraint       |
| `mkt_intent_replacement_refused`      | Replacement is outside the pinned policy                       |
| `mkt_intent_idempotency_conflict`     | One `(pubkey, kind, d)` has different bytes                    |
| `mkt_intent_no_exit_package`          | Maker has not verified the recovery package                    |
| `mkt_intent_custody_material_public`  | Event exposes forbidden private material                       |
| `mkt_intent_settlement_overclaim`     | Event, mempool, or cached claim exceeds rail proof             |

Wallets preserve `registry-unavailable`, `relay-unavailable`, `feed-stale`,
`funding-unavailable`, `conflicting-spend`, `fill-rejected`,
`replacement-observed`, `chain-reorg`, `refund-pending`,
`exit-package-unavailable`, and `evidence-unavailable` loss states.

## Fixture manifest

An implementation MUST replay these exact case IDs:

| Case ID                                   | Expected result                                                  |
| ----------------------------------------- | ---------------------------------------------------------------- |
| `intent-positive-fixed-fill`              | Any filler completes an exact fixed-price intent                 |
| `intent-positive-pinned-feed-fill`        | Signed assertion, rail binding, age, scale, and rounding verify  |
| `intent-positive-covenant-reserve`        | Funding amount, template, uniqueness, and refund path verify     |
| `intent-positive-expiry-refund`           | Maker refunds after the exact boundary                           |
| `intent-negative-quote-label`             | Standing intent cannot be parsed as a Quote                      |
| `intent-negative-order-label`             | Fill cannot be parsed as an Order                                |
| `intent-negative-json-number-amount`      | Numeric amount is rejected                                       |
| `intent-negative-disabled-side`           | Capability zero-max side is rejected                             |
| `intent-negative-partial-fill`            | Partial fill is rejected in version 1                            |
| `intent-negative-substitute-feed`         | Alternate feed signer, URL, pointer, or assertion is rejected    |
| `intent-negative-stale-feed`              | Feed older than pinned maximum is rejected                       |
| `intent-negative-unfunded`                | Maker signature without funding is not executable                |
| `intent-negative-template-mismatch`       | Funding constraint differs from event digest                     |
| `intent-negative-derived-d-mismatch`      | Content whose canonical digest differs from `d` is rejected      |
| `intent-negative-duplicate-funding`       | One object cannot back two live intents                          |
| `intent-negative-early-refund`            | Maker refund path before expiry is rejected                      |
| `intent-negative-fill-output`             | Wrong asset, amount, fee, or destination is rejected             |
| `intent-negative-unadmitted-replacement`  | Replacement outside policy is rejected                           |
| `intent-negative-public-custody-material` | Private refund or signing material is rejected                   |
| `intent-replay-same-event`                | Byte-identical replay returns prior result                       |
| `intent-replay-event-conflict`            | Changed bytes under one `d` fail closed                          |
| `intent-fork-conflicting-spend`           | Both spends remain visible until rail resolution                 |
| `intent-expiry-no-implicit-refund`        | Expiry alone does not claim refund settlement                    |
| `intent-privacy-public-leak-manifest`     | Required deliberate public leaks match the manifest              |
| `intent-recovery-all-coordinators-gone`   | Filler fills and maker refunds using rail only                   |
| `intent-recovery-keyless-exit-executor`   | Static executor drives the retained package with a public reader |
| `intent-loss-reorg`                       | Finality rolls back and re-verifies after reorg                  |

The table is the normative version-1 case manifest. Adoption MUST publish one
canonical byte corpus containing every case, expected error, authority test
key, descriptor event ID, and corpus SHA-256 in the implementation contract
manifest. Implementations reference that corpus rather than generating local
variants and MUST NOT advertise `mkt-intent/1` before it passes under the
active configuration.

## Decision record

The maker-funded any-filler shape becomes MKT-INTENT. It does not fold into
MKT-P2P because there is no negotiated counterparty, escrow role, or
subjective dispute authority. It does not stay only rail-native because a
portable signed discovery artifact, collision-reviewed field grammar,
cross-implementation fixtures, and explicit recovery policy let independent
relays and clients discover the same externally enforced intent without
turning a git registry or one operator into the market authority.

Execution, reservation, and settlement remain rail-native. The Nostr event
adds discovery and signed term provenance only.

## References

- [NIP-MKT](MKT.md)
- [Market rails teardown](../teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)
- [tbDEX liquidity protocol teardown](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md)
