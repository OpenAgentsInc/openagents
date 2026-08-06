# NIP-MKT-SWP

## Bitcoin and Lightning Atomic Swaps

`draft` `optional`

MKT-SWP is the Bitcoin and Lightning atomic-swap profile for
[NIP-MKT](MKT.md). Version 1 covers submarine, reverse submarine, and
Bitcoin chain swaps. The profile coordinates signed terms and lifecycle
claims. Bitcoin consensus and Lightning settlement remain the execution and
settlement authorities.

The safety rule is:

> A participant verifies every invoice, lock construction, amount, hash,
> timeout, confirmation rule, and unilateral exit before it moves funds.

Relay acceptance, a provider signature, a Status, and a Close do not satisfy
that verification rule.

## 1. Profile identity, activation, and allocation

The profile tuple is:

| Field             | Value                                                          |
| ----------------- | -------------------------------------------------------------- |
| `profile`         | `mkt-swp`                                                      |
| `profile_version` | `1`                                                            |
| specification     | the exact bytes of this file                                   |
| authority         | the author of the selected NIP-MKT Profile Descriptor          |
| digest            | the descriptor's `x` SHA-256 value                             |
| fixtures          | the manifest in [Section 18](#18-conformance-fixture-manifest) |

A deployment is executable only after its configured Profile Descriptor
pins one authority pubkey, one event ID, the SHA-256 digest of these exact
specification bytes, and the digest of the complete fixture corpus. Clients
MUST allowlist that tuple. A repository path, branch, mutable URL, profile
name, or matching version number is insufficient.

### 1.1 Kind allocation

MKT-SWP owns the profile family `39610-39619` and allocates exactly one kind:

| Kind        | Type                    | Description                          | Publication                        |
| ----------- | ----------------------- | ------------------------------------ | ---------------------------------- |
| 39610       | Addressable, unique `d` | Swap Contract                        | private signed record, NIP-59 only |
| 39611-39619 | —                       | Reserved for later MKT-SWP revisions | unallocated                        |

The Swap Contract is the immutable, bilateral pre-funding commitment described
in Section 4.5. It binds the accepted Order to exact per-leg rail terms,
verifier inputs, external-effect IDs, and each funding actor's pre-signed or
pre-derived exit-package commitment. A Quote cannot serve this role because
the requester has not accepted it and both parties' final exit-package
commitments may not exist when the provider signs the Quote.

No other new kind is needed. A separate pair head would duplicate the
Offering. A reservation record would split the Quote's atomic commitment. A
swap-progress record would duplicate Status. An evidence event would duplicate
typed evidence references and the external verifier's native receipt. Future
revisions MUST repeat the three-lane and registry collision review before
assigning an exact kind from `39611-39619`.

The v1 family review found no collision at these exact source revisions:

| Lane                              | Reviewed revision                          |
| --------------------------------- | ------------------------------------------ |
| official NIPs                     | `c53877571f96eb423661fc23c620d629d37b8f19` |
| Block NIPs                        | `56003ebf98c22367fb6357f295494e26efbd8ae6` |
| OpenAgents NIPs                   | `1c9a957ab8275e23e7952af2395d36c681b5246d` |
| Nostr kind registry `schema.yaml` | `2483e752146d171524dcb10dffd06de2aa271bf3` |

The review reserves a namespace for this draft; it does not make unallocated
kinds valid wire records.

## 2. Roles, eligibility, and signer map

The roles are:

- **requester** or **taker**: requests terms, signs the Order, controls the
  requesting wallet, and verifies before funding or claiming;
- **provider** or **liquidity provider**: signs the Offering and Quote,
  controls its inventory and Lightning node, and performs its quoted leg;
- **local rail verifier**: wallet-selected code that re-derives scripts,
  parses invoices, and checks Bitcoin or Lightning state;
- **external verifier**: an allowlisted independent signer or rail adapter
  that issues a digest-bound verification receipt; and
- **coordination handler**: an optional NIP-MKT handler that accounts for
  provider-signed reservations and timers. It has no wallet or settlement
  authority.

The requester and provider MUST use distinct Nostr pubkeys. An automated
requester MUST also have any wallet, owner, guardian, amount, expiry, and
generation authority required by its environment before it signs an Order.
MKT-SWP does not grant that authority.

V1 has no identity credential requirement. An Offering MAY state a public
eligibility policy. A Quote MUST state every eligibility condition that can
prevent execution. Private eligibility material stays off-relay behind a
digest-bound, audience-restricted reference.

### 2.1 Exact signer map

| Record or claim                      | Required author               | Additional rule                                                                                            |
| ------------------------------------ | ----------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Provider Profile, Offering           | provider                      | Provider Profile address matches the Offering's provider reference.                                        |
| Profile Descriptor                   | selected profile authority    | Client pins authority, event ID, specification digest, and fixture digest.                                 |
| RFQ                                  | requester                     | Every intended provider is a wrapped recipient.                                                            |
| Quote                                | provider                      | References exactly one RFQ and commits complete terms.                                                     |
| Order                                | requester                     | References exactly one Quote and changes only allowed selection fields.                                    |
| Swap Contract (`kind:39610`)         | requester or provider         | References the accepted Order; both roles sign separate immutable records with one shared contract digest. |
| provider Status                      | provider                      | May report only provider actions or observations listed in Section 9.                                      |
| requester Status                     | requester                     | May report only requester actions, local verification results, or observations listed in Section 9.        |
| Cancel request                       | requester or provider         | Has no immediate effect.                                                                                   |
| Cancel answer                        | the other participant         | References the request.                                                                                    |
| Cancel `effective` before funds move | either participant            | References both parties' matching cancellation consent.                                                    |
| Cancel after an irreversible effect  | none                          | Invalid; the session enters completion or recovery.                                                        |
| Close                                | requester or provider         | Terminal for its signer only.                                                                              |
| Public Market Receipt                | a Close signer                | Uses only the redacted fields allowed by both parties.                                                     |
| verification receipt                 | allowlisted external verifier | The verifier is distinct from the evidence producer and names its verifier policy and rail view.           |

A participant MUST reject a Status that attributes the other participant's
local verification, wallet action, secret possession, or consent to its own
signature.

## 3. Assets, networks, directions, amounts, and fees

### 3.1 Network and asset identifiers

`network_id` is `bip122:<reference>`, where `<reference>` is the lowercase
32-character BIP-122 chain reference derived from the genesis block hash.
Deployments MUST allowlist supported references. Human names such as
`mainnet`, `testnet`, and `regtest` are display labels only.

The exact network grammar is `^bip122:[0-9a-f]{32}$`. The exact asset grammar
is one of:

```text
^swp:1:bip122:[0-9a-f]{32}:btc:(chain|lightning)$
^swp:1:bip122:[0-9a-f]{32}:elements:[0-9a-f]{64}:liquid$
```

MKT-SWP distinguishes the same bitcoin unit on different settlement rails:

```text
swp:1:<network_id>:btc:chain
swp:1:<network_id>:btc:lightning
swp:1:<network_id>:elements:<asset_id>:liquid
```

For an Elements rail, `<asset_id>` is the lowercase 32-byte display-order
asset identifier returned by the configured full node. The L-BTC identifier
MUST equal that network's `pegged_asset` from `getsidechaininfo`; a ticker,
asset label, or provider-supplied alias is insufficient. V1 Liquid execution
allowlists only that pegged asset. Other issued assets require a later
profile revision with their own issuance and freeze authority.

These exact strings are `asset_id` values. A market is the ordered
`[input_asset_id, output_asset_id]` pair. A ticker, display name, symbol, or
provider-local pair code MUST NOT be used for matching, grouping, pricing,
or replay identity.

V1 supports these `swap_type` and pair shapes:

| `swap_type` | Input rail    | Output rail   | Direction                                                                  |
| ----------- | ------------- | ------------- | -------------------------------------------------------------------------- |
| `submarine` | Bitcoin chain | Lightning     | requester locks chain; provider pays Lightning                             |
| `reverse`   | Lightning     | Bitcoin chain | requester pays a hold invoice; provider locks chain                        |
| `chain`     | Bitcoin chain | Bitcoin chain | requester locks the source network; provider locks the destination network |

For this table, a chain leg is either the Bitcoin `chain` rail or the Elements
`liquid` rail. Submarine and reverse swaps admit either chain rail. A chain
swap MUST have distinct asset IDs and V1 admits Bitcoin BTC to or from the
configured network's pegged L-BTC only. Ark, EVM, mint, non-pegged Elements
assets, and fiat legs are unsupported in v1.

### 3.2 Canonical amounts

All amounts are decimal strings in satoshis and match:

```text
^(0|[1-9][0-9]*)$
```

They MUST fit an unsigned 64-bit integer and MUST NOT use a JSON number,
sign, decimal point, exponent, whitespace, unit suffix, or leading zero.
Executable amounts MUST be positive. `"0"` is permitted only for an
Offering's `max` side-disable rule and for loss-accounting fields whose
amount is zero. An enabled side has positive `min` and `max`, with
`min <= max`.

An Offering carries one `sides` entry per ordered pair:

```json
{
  "input_asset_id": "swp:1:bip122:<reference>:btc:chain",
  "output_asset_id": "swp:1:bip122:<reference>:btc:lightning",
  "min": "10000",
  "max": "1000000",
  "fee_bps": "25"
}
```

`max="0"` disables that exact side. Omission does not disable a side. A
provider MUST NOT Quote a disabled or unadvertised side.

### 3.3 Fees and output promise

`fee_bps` is a canonical decimal string from `"0"` through `"10000"`.
It is a provider promise for a conforming fill, subject to the exact Quote.
It is not proof that execution occurred or that the fee was competitive.

Every Quote states:

- `input_amount`, `output_amount`, and `fee_bps`;
- `provider_fee`, `miner_fee_budget`, and `lightning_routing_fee_budget` as
  canonical satoshi strings;
- who pays each miner and routing fee;
- the rounding rule, which MUST be `floor_output_sats`; and
- `amount_equation`, an ASCII expression identifier from the v1 allowlist:
  `input_minus_provider_and_quoted_fees` or `one_to_one_less_quoted_fees`.

The output amount is the fill promise. A provider MUST NOT reduce it after
Order because its route or miner fee changed. A selectable input amount is
allowed only within the Quote's declared range and deterministic amount
equation.

### 3.4 Exact price-feed pinning

Bitcoin/Lightning v1 does not need an exchange-rate feed. If a provider
uses a feed to derive an indicative fee or a chain-network conversion term,
the Quote includes:

```json
{
  "price_feed": {
    "url": "https://example.invalid/feed",
    "value_pointer": "/data/value",
    "observed_value": "100000000",
    "observed_at": 1785859200,
    "max_age_seconds": 30,
    "response_sha256": "<64-lower-hex>"
  }
}
```

`url` MUST be the exact HTTPS URL fetched by both parties. It MUST have no
userinfo or fragment. Redirects are rejected. `value_pointer` is an RFC 6901
JSON Pointer. `observed_value` is a canonical decimal string. The exact
response bytes are bound by `response_sha256`.

Before Order, the requester fetches that exact URL, applies that exact
pointer, checks the digest and staleness rule, and reproduces the quoted
calculation. A substitute host, mirror, fallback endpoint, alternate JSON
path, or semantically equivalent price is a term mismatch. A firm Quote
still binds its output amount; a later feed movement cannot reduce the fill.
The feed is never a funding, execution, finality, or settlement authority.

## 4. Record schemas and allowed Order selection

All records carry the NIP-MKT base envelope and tags. Their profile content
contains `mkt_swp`, a JSON object. Unknown members inside `mkt_swp` are
retained when forwarding. An unknown member named in `critical` makes the
record fail closed.

### 4.1 Offering

An MKT-SWP Offering requires:

```json
{
  "mkt_swp": {
    "swap_types": ["submarine", "reverse", "chain"],
    "sides": [],
    "networks": [],
    "script_modes": ["taproot-musig2-script-exit"],
    "reservation_proof_classes": [],
    "confirmation_policies": [],
    "availability": "available",
    "evm_extension": "unsupported"
  }
}
```

`swap_types`, `sides`, `networks`, and `script_modes` are non-empty and
duplicate-free. `availability` is `available`, `limited`, or `unavailable`.
Live inventory, UTXOs, channel balances, invoices, addresses, scripts,
payment hashes, and reserve witnesses MUST NOT appear in a public Offering.

`networks` contains 1-8 exact `network_id` strings. `sides` contains 1-16
objects with the Section 3.2 shape. `script_modes` contains only
`taproot-musig2-script-exit` in v1. `reservation_proof_classes` contains 1-8
values from Section 5. `confirmation_policies` contains 1-8 objects:

```json
{
  "policy_id": "btc-1conf-no-rbf",
  "minimum_confirmations": "1",
  "reorg_safety_blocks": "6",
  "zero_confirmation": "forbidden",
  "rbf": "reject",
  "replacement": "reject"
}
```

Every count is a canonical decimal string. `policy_id` follows the NIP-MKT
profile identifier grammar. `zero_confirmation` is `forbidden` or `allowed`;
`rbf` and `replacement` are independently `reject` or `track`. An Offering
policy is an advertised option. The Quote copies the selected values and may
tighten them; it cannot weaken the requester's RFQ constraints.

### 4.2 RFQ

An RFQ specifies `swap_type`, the ordered asset pair, exact amount or bounded
range, maximum total fee, confirmation-policy constraints, script modes,
desired completion time, and whether a firm Quote is required. A reverse RFQ
includes only a payment-hash commitment and requester claim public key. A
submarine RFQ carries an encrypted BOLT11 invoice or an audience-restricted
invoice reference. A chain RFQ carries the requester claim and refund public
keys for the two named networks.

For reverse and chain swaps, the requester generates the preimage and reveals
only its SHA-256 hash. For submarine swaps, the requester's Lightning invoice
commits the payment hash and the requester retains invoice control. Every leg
of one swap uses that one payment hash.

The RFQ MUST NOT contain a preimage, private key, signing nonce, seed,
macaroon, NWC string, bearer token, or wallet credential.

### 4.3 Quote

A Quote commits:

- the RFQ event ID and Offering address;
- `swap_type`, ordered asset-ID pair, network IDs, and rail roles;
- exact amount and fee terms from Section 3;
- `quote=indicative|firm`, base `reservation`, and Section 5 reservation;
- exact script or Taproot construction version and verifier digest;
- payment-hash commitment and invoice digest as applicable;
- public keys used by each claim and refund path;
- confirmation, reorg, RBF, replacement, and zero-confirmation policy;
- every absolute time, chain height, CLTV value, and safety margin in the
  timeout ladder, including `hold_expiry_height` for a reverse swap;
- the cooperative MuSig2 path and unilateral script-path exits;
- evidence requirements, verifier policy, recovery channel, and exit-package
  requirements;
- cancellation boundary and terminal loss allocation; and
- the EVM-reserved object from Section 16, set to `null` for v1.

The Bitcoin script, Lightning invoice, and every chain-swap leg MUST commit
the same SHA-256 payment hash. An alternate hash or hash function is
`swp_payment_hash_mismatch`.

A Quote MUST NOT include a secret nonce, partial signature, preimage, private
claim/refund key, wallet credential, or live node credential.

### 4.4 Order

An Order selects one Quote. It MAY select only:

- one `input_amount` within a range explicitly offered by that Quote;
- one fee-payer option from a finite Quote list;
- one confirmation policy from a finite Quote list; and
- one public receipt consent value no broader than the Quote permits.

The deterministic amount equation MUST reproduce the output amount. Every
other Quote field is inherited by event-ID reference and MUST NOT be restated
with different bytes. A Quote with no `selectable` object permits no Order
selection.

The Order acknowledges that funding remains disabled until the requester and
provider have signed matching Swap Contract records and every required local
exit package passes Section 12. The package bytes stay client-local or in
audience-restricted storage.

### 4.5 Swap Contract (`kind:39610`)

The Swap Contract freezes execution inputs after Order and before funds move.
It is an immutable-by-contract, private signed event transported exactly like
NIP-MKT private records: one persistent NIP-59 `kind:1059` gift wrap per
recipient and a sender recovery copy. A bare `kind:39610` publication is
invalid. Its rumor kind is `39610`; the signed inner event survives unwrap and
is the contract authority.

Each event has a new unique `d` plus the NIP-MKT `session`, `profile`, and
`alt` tags. It has exactly one `p` for the other participant, marked with that
participant's `requester` or `provider` role. It also has exactly one `e`
marked `order`, one `e` marked `quote`, one `x` containing
`contract_sha256`, and one `role` equal to `requester` or `provider`. An
indicative Quote additionally requires one `e` marked `status` that identifies
the provider's accepted Status. The author MUST match its role in the
referenced Order and Quote. Changed bytes under the same
`(pubkey, 39610, d)` are an idempotency conflict. A Swap Contract has no
NIP-40 expiration because participants need it for recovery after every
negotiation deadline has passed.

The `mkt_swp.contract` object has this logical shape:

```json
{
  "contract": {
    "order_id": "<64-lower-hex>",
    "quote_id": "<64-lower-hex>",
    "swap_type": "submarine|reverse|chain",
    "asset_pair": ["<input_asset_id>", "<output_asset_id>"],
    "payment_hash": "<64-lower-hex>",
    "legs": [],
    "timeout_ladder": {},
    "reservation_commitment": {},
    "verifier_inputs": [],
    "effect_bindings": [],
    "exit_package_commitments": [],
    "recovery": {},
    "evm_leg": null
  },
  "contract_sha256": "<64-lower-hex>",
  "signer_role": "requester|provider"
}
```

`contract_sha256` is SHA-256 of the RFC 8785 serialization of the exact
`contract` object. The requester and provider sign separate `kind:39610`
events with different `d` values and `signer_role` values but identical
`contract` objects and `contract_sha256`. Each event is sent to the other
party and the author. A configured coordination handler receives a separate
wrap only when the Quote disclosed that audience.

Each `legs` member binds `leg_id`, rail, network ID, asset ID, amount, funding
role, receiving role, script or invoice commitment, payment hash, claim and
refund public keys, timelock or CLTV values, confirmation/RBF/replacement
policy, and verifier digest. `verifier_inputs` binds every public input a local
verifier needs to re-derive those terms. It contains no RPC credential or
private key. `effect_bindings` contains every precomputed Section 13 effect ID
and its role/leg tuple. `exit_package_commitments` binds participant role, leg,
claim or refund path, package mode, and SHA-256 digest.

For a submarine swap, the requester MAY leave `funding_transaction`,
`funding_transaction_sha256`, and `output_index` absent from the Quote’s
requester-funded `source` Bitcoin verifier because it selects the funding
inputs and change only after accepting the Quote. The bilateral Swap Contract
MUST add exactly those three members to that same verifier.
`funding_transaction` MUST be lowercase raw transaction hex;
`funding_transaction_sha256` MUST equal SHA-256 of its decoded bytes; and
`output_index` MUST select an output whose value and scriptPubKey exactly equal
the unchanged quoted `amount` and `script_pubkey`. The Swap Contract MUST
replace only that source leg’s `verifier_digest` with SHA-256 of the RFC 8785
serialization of the resolved verifier. Every other verifier byte, leg byte,
and Quote term remains unchanged. This resolution is invalid for reverse or
chain swaps, non-source legs, and provider-funded legs.

The required leg and exit signer map is:

| Swap type and leg           | Funding actor | Required requester commitment                                                                  | Required provider commitment                                                                  |
| --------------------------- | ------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| submarine chain lock        | requester     | funding template plus unilateral refund package                                                | claim template and cooperative-signing transcript commitment                                  |
| submarine Lightning payment | provider      | invoice digest and local receipt-verifier input                                                | payment effect binding and node-local recovery-handle commitment                              |
| reverse hold invoice        | requester     | payment effect binding, payment-hash commitment, and local preimage-recovery-handle commitment | hold-invoice digest, settle/cancel effect bindings, and node-local recovery-handle commitment |
| reverse chain lock          | provider      | claim template and cooperative-signing transcript commitment                                   | funding template plus unilateral refund package                                               |
| chain source lock           | requester     | funding template plus unilateral source-refund package                                         | source-claim template and cooperative-signing transcript commitment                           |
| chain destination lock      | provider      | destination-claim template and cooperative-signing transcript commitment                       | funding template plus unilateral destination-refund package                                   |

A commitment is a digest and public execution description. It does not carry
the preimage, private claim/refund key, secret nonce, seed, macaroon, NWC
string, or signer credential.

The contract becomes `contract_bound` only when both valid events are present,
their shared digest matches, their signer roles are complementary, and every
field agrees with the Quote and allowed Order selection. A third party cannot
countersign for an absent participant. Neither party may fund before
`contract_bound` and successful local verification. Status and evidence
references point to the relevant Swap Contract event IDs and shared digest.
This profile adds `contract` as an `e`-tag marker for those exact references.

## 5. Capacity and reservation accounting

A reservation identifies a provider-local capacity bucket without exposing
the bucket's inventory publicly. A reserving Quote includes:

```json
{
  "reservation_terms": {
    "reservation_id": "<64-lower-hex>",
    "capacity_bucket_id": "<1-64 profile identifier>",
    "reserved_asset_id": "<asset_id>",
    "reserved_amount": "<decimal sats>",
    "reservation_expires_at": 1785859200,
    "allocation_sequence": "42",
    "proof_class": "provider_signed",
    "proof_ref": "<digest-bound private reference>",
    "capacity_commitment_sha256": "<64-lower-hex>"
  }
}
```

`reservation_id` is unique per provider and Quote. `allocation_sequence` is
a canonical decimal string that increases within the bucket. The commitment
binds provider, bucket, asset ID, total committed capacity, active reservation
set digest, sequence, and expiry. It need not reveal the active set to the
requester.

At any instant:

```text
sum(active reserved_amount for bucket and asset) <= committed_capacity
```

Identical Quote replay returns the prior reservation result. A second Quote
with the same reservation ID and different bytes is
`swp_idempotency_conflict`. Two active reservations that claim the same
allocation sequence or make the inequality false are retained as an
attributable `swp_reservation_fork`; they are never resolved by arrival time.

Reservation proof classes are:

| Class                   | Allowed reservation               | Meaning                                                                                                    |
| ----------------------- | --------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `provider_signed`       | `soft`                            | Provider commits bytes; no independent capacity proof.                                                     |
| `handler_accounted`     | `soft` or `hard`                  | A configured handler proves its bounded accounting view, not provider solvency.                            |
| `utxo_control`          | `hard`                            | Verifier checks an unspent output, asset, amount, control proof, and encumbrance policy.                   |
| `lightning_liquidity`   | `hard`                            | Verifier checks a named node/channel liquidity method and freshness policy without receiving a macaroon.   |
| `funded_htlc`           | `hard`                            | The exact swap output already locks the quoted amount under verified terms.                                |
| `covenant_reserve`      | `hard`                            | A verifier proves a covenant locks the asset and enforces at least the quoted output for an eligible fill. |
| `third_party_guarantee` | `hard` only when policy admits it | A named guarantor binds amount, expiry, exclusions, claim authority, and payout rail.                      |

For `covenant_reserve`, the proof binds the funding outpoint or rail object,
covenant/program digest, asset ID, available amount, eligible fill predicate,
minimum output, fee rule, expiry, and verifier view. The verifier MUST check
that the same reserve unit is not counted in another active fill. A watcher
observation alone is not proof that the covenant is safe. An on-chain output
whose price must be changed by spending it is not a Quote mechanism and MUST
NOT become an on-relay orderbook.

An indicative Quote defaults to `reservation=none`. A firm Quote MUST use
`soft` or `hard` and state the effect of proof unavailability. Expiry releases
only the reservation accounting entry. It does not cancel, refund, settle, or
sign for either party.

## 6. Custody and control dimensions

Every Quote includes:

```json
{
  "custody": {
    "funds_control": [],
    "execution_control": [],
    "settlement_authority": [],
    "reversibility": [],
    "recourse": [],
    "credential_exposure": "none",
    "maximum_custody_duration_seconds": 0
  }
}
```

The arrays state each leg and phase. V1 applies these rules:

- Before funding, each principal controls its own funds.
- A funded output is controlled by the verified hash/timelock construction,
  with a cooperative path and a unilateral script path.
- A reverse hold invoice remains controlled by Lightning HTLC rules until
  preimage settlement or cancellation.
- MuSig2 cooperative execution needs both public-key participants. The
  unilateral leaf MUST remain independently executable after its timeout.
- The relay and coordination handler control no principal, inventory, claim,
  refund, invoice, or signing key.
- Bitcoin consensus is the chain settlement authority. The involved
  Lightning implementations and BOLT rules are the Lightning settlement
  authority.
- Recourse is cryptographic claim or refund plus any separately named legal
  or guarantee path. A support URL is not recourse.

`maximum_custody_duration_seconds` is the Quote's worst-case wall-clock
estimate from first funding until the last unilateral exit becomes spendable.
The Quote also carries the exact height-based bound. Clients display both and
do not convert an estimated time into consensus authority.

## 7. Verify-before-fund requirements

The requester MUST reach `verification_passed` locally before presenting a
fund action. The provider applies the same checks before it funds its leg.
Both verify the matching requester/provider Swap Contract pair and shared
contract digest. The verifier checks the exact Quote, Order, and Swap Contract
event IDs.

### 7.1 Common checks

1. Verify the Nostr signatures, causal references, profile tuple, expiry,
   asset pair, amount equation, fee promise, allowed Order selection, both
   Swap Contract authors, and shared contract digest.
2. Recompute the external-effect IDs from Section 13.
3. Parse every script and Taproot tree from bytes. Do not compare only an
   address supplied by the counterparty.
4. Recompute the output key/address from the internal keys, tree, network,
   and tweak.
5. Check payment hash, claim key, refund key, timelock, leaf version,
   sighash policy, and amount against the Quote.
6. Check the timeout inequalities in Section 8 against the current chain and
   invoice state.
7. Build and persist the applicable exit package and verify its digest against
   the matching Swap Contract pair.
8. Check confirmation, RBF, replacement, reorg, minimum relay fee, and dust
   policy exactly as quoted.
9. Refuse unknown script versions, omitted unilateral leaves, hidden
   composition, unsupported extensions, and non-null `evm_leg`.

### 7.2 Lightning invoice checks

The client parses the complete invoice locally and verifies network, payment
hash, amount, expiry, minimum final CLTV delta, route hints disclosure policy,
and description or description-hash commitment. An amountless invoice is
invalid in v1. Invoice success from the provider API is not a substitute for
the requester's node view.

For reverse swaps, the requester generates the preimage and sends only its
SHA-256 hash. The provider creates a hold invoice for that hash. Hold behavior
cannot be inferred from BOLT11 bytes. The requester's Lightning adapter must
observe that its payment remains pending while the provider prepares the chain
lock and must fail closed if the invoice settles or cancels unexpectedly.

### 7.3 MuSig2 checks

The Quote binds participant public keys, their ordering, the Taproot tweak,
the complete message/sighash, and the cooperative transaction template.
Secret nonces remain inside the signing wallet. A client MUST prevent nonce
reuse across any message. It verifies every received public nonce and partial
signature before aggregation. Cooperative failure never removes the quoted
script-path exit.

#### 7.3.1 Cooperative signing transcript

Cooperative signing uses ordinary private Status records. The Status carries
`state=executing`, `swp_state=cooperative_signing_pending`, and one
`cooperative_signing` object. It remains a signed inner record inside the
recipient-gated NIP-59 transport; the relay does not inspect it. The
requester key is participant index `0` and the provider key is participant
index `1` for every swap type and leg.

The context has this exact logical shape:

```json
{
  "schema": "openagents.mkt-swp.cooperative-signing.v1",
  "order_id": "<64-lower-hex>",
  "swap_contract_sha256": "<64-lower-hex>",
  "effect_id": "<Section 13 cooperative_sign effect ID>",
  "leg_id": "source|destination",
  "unsigned_transaction": "<lowercase raw transaction hex without witness>",
  "transaction_sha256": "<64-lower-hex>",
  "input_index": 0,
  "prevouts": [{ "amount": "<canonical sats>", "script_pubkey": "<lower-hex>" }],
  "signature_hash": "<64-lower-hex>",
  "sighash_type": "DEFAULT",
  "participant_keys": ["<requester compressed key>", "<provider compressed key>"],
  "tweaks": [{ "value": "<64-lower-hex>", "xonly": true }],
  "aggregate_key": "<64-lower-hex>",
  "exit_package_sha256": "<64-lower-hex>",
  "latest_safe_height": "500"
}
```

`context_sha256` is SHA-256 of the RFC 8785 bytes of the complete context.
Every contribution repeats that context and digest plus its
`participant_index` and one action. The receiver re-parses the unsigned
transaction, recomputes its digest and BIP-341 `SIGHASH_DEFAULT`, verifies
that the selected prevout pays the tweaked aggregate key, and verifies the
effect ID and unilateral exit-package commitment before accepting any
contribution.

The action shapes are closed:

- `nonce_commitment` carries only `nonce_commitment`, SHA-256 of the exact
  66-byte BIP-327 public nonce.
- `public_nonce` carries that same `nonce_commitment` and `public_nonce`. A
  public nonce before both participants' prior exact commitments is invalid.
- `partial_signature` carries the two `public_nonces` in participant-key
  order and the author's `partial_signature`. Both reveals must already be
  present and the partial signature must verify before it is retained or
  forwarded.
- `final_signature` carries both `public_nonces`, both
  `partial_signatures`, and the aggregate `final_signature`. Every partial
  and the aggregate signature are independently verified.
- `aborted` carries only `abort_reason` and `fallback=script_path`.
  `abort_reason` is `timeout`, `counterparty_unavailable`,
  `transcript_invalid`, or `wallet_refused`.

One participant may emit successive Status records with the same
`cooperative_signing_pending` state only when its normal Status sequence and
`previous` reference are contiguous, the context digest is unchanged, and
the action advances in the order above. A duplicate action under another
record identity, changed context under one digest, partial before both nonce
reveals, final signature before both partials, or a transcript that is both
finalized and aborted is `swp_musig_transcript_invalid`.

The signing wallet owns the secret nonce and must bind its one-time use to
`context_sha256`. Client and provider snapshots may retain public requests,
commitments, nonces, signatures, and an opaque wallet reference; they MUST
NOT retain the secret nonce. Exact signed-record replay is idempotent. A
changed replay conflicts. No transcript contribution is valid after either
participant aborts. An abort or deadline expiry destroys the local
nonce session and continues through the already-verified unilateral
script-path package without changing the lifecycle truth.

### 7.4 Funding-output checks

After funding is observed, the verifier checks transaction ID, output index,
prevout amount, exact scriptPubKey, mempool replacement state, confirmations,
and competing spends. A provider-supplied transaction ID is a lookup hint.
The local Bitcoin view determines whether the output exists.

Any failed check produces a typed error from Section 17 and leaves funding
disabled.

### 7.5 Liquid funding and exit checks

A Liquid leg uses `rail=liquid`, an allowlisted Elements network ID, and the
network's exact pegged-asset identifier from Section 3.1. The Quote and both
Swap Contracts bind the raw transaction digest, output index, asset ID,
amount, scriptPubKey, confidentiality mode, and every serialized commitment
present on the selected output. A transaction ID or provider-decoded JSON is
only a lookup hint.

The output's Taproot key, leaf scripts, control blocks, hashlock, and timelock
are re-derived under the same BIP-341/BIP-342 tree rules as the Bitcoin leg.
The Elements transaction and signature hash include Elements-specific asset,
value, nonce, issuance, and witness fields. A client MUST NOT apply the
Bitcoin transaction parser or Bitcoin Taproot sighash to those bytes. Its
Liquid adapter parses the Elements envelope, checks the exact selected output
and tree locally, and submits a completed spend to its allowlisted full node's
consensus and mempool policy before broadcast.

`confidentiality` is closed to these shapes:

```json
{"mode":"explicit","asset":"<64-lower-hex>","amount":"<canonical sats>"}
{"mode":"recipient_blinded","asset_commitment":"<66-lower-hex>","value_commitment":"<66-lower-hex>","nonce":"<66-lower-hex>","rangeproof_sha256":"<64-lower-hex>","surjectionproof_sha256":"<64-lower-hex>"}
```

For `recipient_blinded`, the receiver uses its own wallet blinding key through
its local Elements node to run `unblindrawtransaction`, then decodes that exact
result. It requires the selected output to reveal the signed asset ID and
amount, retain the signed scriptPubKey and output index, and correspond to the
commitments in the original raw transaction. Failure to unblind is failure to
fund or claim. A sender may use its locally retained blinding data to perform
the same check before broadcast.

V1 does not implement secp256k1-zkp range-proof or surjection-proof
verification in the client core and does not claim to validate an arbitrary
third-party confidential output. The allowlisted local Elements node verifies
consensus proofs and chain state. The client verifies only an output that it
authored or can unblind, and it records this authority as
`local_elementsd_unblind`, never `independent_ct_proof`. An explicit output is
valid only when the Quote permitted `explicit`; a confidential-only side
cannot be silently downgraded.

The participant that can lose Liquid principal persists a Liquid unilateral
exit package before funding. It binds the Elements transaction format,
selected asset, selected output, exact script path, timelock, fee output, and
either a complete signed transaction or a wallet/external-signer handle under
Section 12. No blinding key, value blinder, asset blinder, seed, or spend key
may enter a Swap Contract, relay artifact, provider database, or retained lab
record.

## 8. Timeout ladders

Every height is a canonical decimal string. Every Unix time is an integer.
The Quote records the chain tip hash and height observed when it calculated
the ladder. `chain_finality_blocks`, `reorg_safety_blocks`,
`broadcast_safety_blocks`, and `lightning_settlement_blocks` MUST be positive.

### 8.1 Submarine

Let:

- `H_fund` be the last height at which requester funding is accepted;
- `H_claim` be the provider's last safe cooperative-claim height; and
- `H_refund` be the first height at which the requester refund path is valid.

The Quote MUST satisfy:

```text
current_height < H_fund
H_fund + chain_finality_blocks <= H_claim
H_claim + broadcast_safety_blocks + reorg_safety_blocks < H_refund
invoice_expiration_time > expected_time(H_claim)
```

The provider MUST NOT request funding if the invoice cannot remain payable
through `H_claim`. If the provider has not paid Lightning by `H_claim`, the
requester enters refund preparation. The requester broadcasts at or after
`H_refund` when no verified claim spend exists.

### 8.2 Reverse submarine

Let:

- `H_hold_expiry` be the signed `hold_expiry_height` value in the Quote and
  matching Swap Contract records. It is the minimum acceptable value of the
  shortest incoming Lightning HTLC expiry, not an observation that an HTLC
  already exists;
- `H_observed_shortest` be the provider node's observed shortest absolute
  expiry across every incoming HTLC part accepted for the bound hold invoice;
- `H_lock_last` be the last safe provider lock height;
- `H_user_claim` be the requester's last safe chain-claim height; and
- `H_provider_refund` be the first provider-refund height.

The Quote MUST satisfy:

```text
current_height < H_lock_last
H_lock_last + chain_finality_blocks <= H_user_claim
H_user_claim + broadcast_safety_blocks + reorg_safety_blocks < H_provider_refund
H_provider_refund + broadcast_safety_blocks + lightning_settlement_blocks < H_hold_expiry
```

Before it funds the chain lock, the provider MUST observe at least one held
incoming HTLC for the bound payment and MUST verify:

```text
H_observed_shortest >= H_hold_expiry
```

For a multipart payment, the comparison uses the shortest expiry across all
accepted parts, not the longest part or an average. The provider MUST NOT fund
while the held-payment part set is incomplete. If that set changes before
funding, the provider recomputes the minimum. A value below the signed minimum
invalidates the funding gate. The provider cancels the held payment without
funding and reports `swp_timeout_ladder_unsafe`. The observed height is local
rail evidence; it does not replace or mutate the signed `hold_expiry_height`
term.

The provider cancels held Lightning HTLCs without funding if it misses
`H_lock_last`. The requester claims only after verifying the provider output.
Its claim reveals the preimage. The provider settles the hold invoice only
after it verifies that preimage. If the requester never claims, the provider
refunds at `H_provider_refund` and cancels the still-held invoice before
the observed shortest expiry. The signed `H_hold_expiry` remains the minimum
against which that observation is checked.

### 8.3 Chain swap

The requester funds the source output first. The provider then funds the
destination output. Let `H_dest_refund` and `H_source_refund` be the first
refund heights on their respective networks, expressed with the Quote's
cross-network observation and time conversion policy. The Quote MUST prove:

```text
destination funding final before requester claim is enabled
destination refund window closes before source refund window
source_refund_time >= destination_refund_time
                    + provider_claim_margin
                    + both_network_reorg_margins
                    + both_network_broadcast_margins
```

The requester claims the destination and reveals the preimage. The provider
uses it to claim the source. If the provider never funds the destination, the
requester refunds the source. If the requester never claims the destination,
the provider refunds it before the requester source refund becomes valid.
An implementation MUST reject a ladder whose worst-case graph cannot be
calculated without a mutable provider default.

### 8.4 Clock and policy changes

The maximum accepted wall-clock skew is quoted in `clock_skew_seconds` and
MUST NOT exceed 120 seconds in v1. `created_at` is not trusted time. A provider
cannot change a confirmation, zero-confirmation, RBF, replacement, or safety
margin policy after Order. A Status that announces a different policy is
`swp_terms_mismatch`.

## 9. Lifecycle state machines

Every Status carries the base `state` plus one `swp_state`. Each author has
its own base `seq`. The session projection retains both participant streams.
It advances a verified execution rung only when the required signer and
evidence exist.

`contract_pending` is a local projection after one valid Swap Contract is
present. `contract_bound` is a local projection after the complementary pair
passes Section 4.5. Neither value is established by a Status claim.

The base `state` is determined from `swp_state` as follows:

| MKT-SWP state class                                                                                                                                     | Base `state`         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `accepted`                                                                                                                                              | `accepted`           |
| any `*_terms_ready`, `*_verified`, `verification_passed`, or `hold_invoice_ready`                                                                       | `awaiting_input`     |
| `funding_required` or `source_funding_required`                                                                                                         | `funding_required`   |
| any `*_funding_broadcast` or `*_funding_observed`                                                                                                       | `funding_observed`   |
| any `*_funding_final`, `*_payment_pending`, `*_htlcs_held`, `*_claim_pending`, `*_claimed`, or `cooperative_signing_pending` before both legs are final | `executing`          |
| `lightning_settlement_pending` or `provider_source_claim_pending`                                                                                       | `settlement_pending` |
| `completed`                                                                                                                                             | `completed`          |
| any `*_refund_prepared`, `*_refund_pending`, `invoice_cancel_pending`, or `refund_prepared`                                                             | `refund_pending`     |
| `refunded`, `*_refunded`, or `invoice_cancelled` after all principal is released                                                                        | `refunded`           |
| `disputed`                                                                                                                                              | `disputed`           |
| `failed` or `unresolved`                                                                                                                                | `failed`             |

`contract_pending` and `contract_bound` have no Status mapping. A value that
matches no row is `swp_status_transition_invalid`.

### 9.1 Common terminal rules

- `completed` requires verified completion of both legs under the Quote's
  finality rules.
- `refunded` requires verified refund or release of every funded principal.
- `failed` records a terminated attempt with no remaining expected transition
  and complete loss accounting.
- `unresolved` records missing evidence, an unexecutable exit, or principal
  whose terminal disposition is unknown.
- `cancelled` is valid only when no irreversible external effect remains.
- `expired` is valid before funding, or after all funded effects have reached
  a separately proved refund/release outcome.

### 9.2 Submarine transitions

```text
ordered -> accepted -> contract_pending -> contract_bound
        -> lock_terms_ready -> requester_verification_passed
        -> funding_required -> requester_funding_broadcast
        -> funding_observed -> funding_final
        -> lightning_payment_pending -> lightning_paid
        -> provider_claim_pending -> provider_claimed -> completed
```

Recovery branches:

```text
accepted|contract_pending|contract_bound|lock_terms_ready
        |requester_verification_passed -> cancelled|expired
requester_funding_broadcast|funding_observed|funding_final
        -> refund_prepared -> refund_pending -> refunded
any funded state -> disputed|failed|unresolved
```

The provider signs `accepted`, `lock_terms_ready`,
`lightning_payment_pending`, `lightning_paid`, `provider_claim_pending`, and
`provider_claimed`. The requester signs `requester_verification_passed`,
`requester_funding_broadcast`, `refund_prepared`, and `refund_pending`.
Either may report observations, but only an admitted verifier raises their
evidence rung.

### 9.3 Reverse transitions

```text
ordered -> accepted -> contract_pending -> contract_bound
        -> hold_invoice_ready -> requester_invoice_verified
        -> lightning_payment_pending -> lightning_htlcs_held
        -> provider_lock_terms_ready -> requester_lock_verified
        -> provider_funding_broadcast -> funding_observed -> funding_final
        -> requester_claim_pending -> requester_claimed
        -> lightning_settlement_pending -> lightning_paid -> completed
```

Recovery branches:

```text
before provider_funding_broadcast -> invoice_cancel_pending
        -> invoice_cancelled -> cancelled|expired
after provider_funding_broadcast and before requester_claimed
        -> provider_refund_prepared -> provider_refund_pending
        -> provider_refunded -> invoice_cancelled -> refunded
any funded state -> disputed|failed|unresolved
```

The provider signs `accepted`, `hold_invoice_ready`,
`lightning_htlcs_held`, `provider_lock_terms_ready`,
`provider_funding_broadcast`, `lightning_settlement_pending`,
`lightning_paid`, `provider_refund_prepared`, `provider_refund_pending`, and
`provider_refunded`. The requester signs `requester_invoice_verified`,
`lightning_payment_pending`, `requester_lock_verified`,
`requester_claim_pending`, and `requester_claimed`.

### 9.4 Chain transitions

```text
ordered -> accepted -> contract_pending -> contract_bound
        -> source_lock_terms_ready
        -> requester_source_verified -> source_funding_required
        -> requester_source_broadcast -> source_funding_observed
        -> source_funding_final -> destination_lock_terms_ready
        -> requester_destination_verified -> provider_destination_broadcast
        -> destination_funding_observed -> destination_funding_final
        -> requester_destination_claim_pending
        -> requester_destination_claimed
        -> provider_source_claim_pending -> provider_source_claimed
        -> completed
```

Recovery branches:

```text
source funded, destination not funded
        -> requester_source_refund_prepared
        -> requester_source_refund_pending -> requester_source_refunded
destination funded, requester did not claim
        -> provider_destination_refund_prepared
        -> provider_destination_refund_pending
        -> provider_destination_refunded
        -> requester_source_refund_pending -> requester_source_refunded
        -> refunded
any funded state -> disputed|failed|unresolved
```

The requester signs requester-prefixed actions. The provider signs
provider-prefixed actions and `accepted`, `source_lock_terms_ready`, and
`source_funding_required`, and `destination_lock_terms_ready`. Each
participant may report a rail observation without promoting its rung.

### 9.5 Illegal transitions, gaps, and forks

A Status that skips a required action is retained as an invalid claim and
does not advance the session. A missing sequence number is
`swp_status_gap`. Two Status records from one author for the same
`(session, order, seq)` are `swp_status_fork`; clients retain and display
both. A later Status cannot erase the gap or fork.

## 10. Cancellation, disputes, and recovery

Before Order, expiry ends the invitation or Quote. After a firm Order becomes
effective and before funds move, including while the Swap Contract pair is
pending, cancellation requires a Cancel request and the other party's signed
`accepted`, followed by an `effective` Cancel that references both. An
indicative Quote can also terminate through a provider `rejected` Status.

After a funding broadcast, Cancel can request cooperation, but it cannot be
`effective`. The state machine follows claim, refund, failure, dispute, or
unresolved recovery. A deletion request, expiry, relay shutdown, provider API
failure, or Close cannot reverse an external effect.

A dispute carries the exact contested event IDs, rail objects, evidence
digests, claimed loss, and optional arbiter or guarantee process named in the
Quote. An arbiter has only its disclosed authority. It cannot sign for a
party, move a script-controlled output without its rail key, or upgrade
evidence into settlement.

## 11. Evidence and verifier authority

Every evidence reference includes:

```json
{
  "class": "bitcoin_output",
  "rung": "measured",
  "rail": "bitcoin",
  "reference": "<canonical rail reference>",
  "artifact_sha256": "<64-lower-hex>",
  "producer_pubkey": "<64-lower-hex>",
  "verifier_pubkey": "<64-lower-hex or null>",
  "verifier_policy": "<profile identifier or null>",
  "observed_at": 1785859200,
  "view": "<tip hash, height, node set, or invoice lookup scope>"
}
```

Allowed classes are `invoice`, `lightning_htlc`, `lightning_payment`,
`bitcoin_transaction`, `bitcoin_output`, `bitcoin_spend`,
`liquid_transaction`, `liquid_output`, `liquid_spend`, `reservation`,
`covenant_reserve`, `claim`, `refund`, `reorg`, and `replacement`.
`reference` is a BOLT payment hash, Bitcoin `txid:vout`, transaction ID, or
digest-bound external receipt identifier appropriate to the class. Full
private artifacts stay off-relay.

Evidence rungs are monotonic only for the exact artifact and policy:

| Rung       | Authority                                                                                              |
| ---------- | ------------------------------------------------------------------------------------------------------ |
| `pledged`  | Evidence producer signed a claim.                                                                      |
| `reserved` | A reservation proof policy admitted a capacity commitment.                                             |
| `measured` | A named observer saw the rail object in its stated view.                                               |
| `verified` | An allowlisted verifier re-derived the artifact and terms under the pinned policy.                     |
| `paid`     | The relevant Lightning node or payment proof establishes payment under the Quote.                      |
| `settled`  | Bitcoin, Liquid, or Lightning finality rules in the Quote are satisfied in the verifier's stated view. |

The local wallet is the authority for whether it enables a fund, claim, or
refund action. Bitcoin or Elements consensus and the Lightning rail determine
external settlement. A relay-signed observation, provider Status, explorer
response, API response, NIP-57 receipt, or Close cannot independently produce
`paid` or `settled`.

Reorg and replacement verification is continuous until the quoted finality
threshold. A displaced funding or claim transaction lowers the local state,
records `swp_reorg` or `swp_replacement`, and re-enters the recovery graph.
Clients retain the prior evidence and the new rail view.

### 11.1 Rail adapters

Every Quote and Swap Contract pin each adapter's identifier, version,
executable or source digest, verifier-policy digest, and supported network
IDs. Adapter code is not fetched and executed from a Profile Descriptor.

The Bitcoin adapter obtains raw transactions, outputs, spends, headers, and
chain tips from a wallet-selected full node or an allowlisted set of
Esplora-compatible sources. It verifies headers according to its configured
trust model, parses transaction bytes locally, applies the exact confirmation
and replacement policy, and continues reorg monitoring through the quoted
finality threshold. An explorer's JSON classification is not finality.

The Liquid adapter obtains raw Elements transactions, outputs, spends,
headers, and chain tips from a wallet-selected Elements full node. It requires
the configured genesis reference and `pegged_asset` to match the signed
network and asset IDs. For a confidential output it uses only the local
wallet's blinding authority described in Section 7.5. RPC JSON cannot replace
the bound raw transaction, and successful unblinding cannot upgrade local
node trust into independent range-proof verification.

The Lightning adapter uses the participant's local node or wallet to parse
the invoice and query HTLC/payment state. For reverse swaps, the provider
adapter proves hold, settle, and cancel capability without disclosing its node
credential. A provider API or NIP-57 receipt is supporting evidence only.

Credit-card or bank-style chargeback is not a v1 rail state. Bitcoin reorg and
transaction replacement use the explicit policies above. A settled Lightning
payment follows the involved Lightning implementation and BOLT rules. An
adapter that exposes administrative reversal, fiat chargeback, token freeze,
or custodian clawback is unsupported and requires a different profile or
extension with its own reversibility and recourse rules.

## 12. Exit packages and coordinator-independent recovery

Every participant that can lose access to principal MUST persist its exit
package before the corresponding funding broadcast. The matching Swap
Contract pair binds the package digest. A package is a UTF-8 JSON document
serialized with RFC 8785 and has this logical shape:

```json
{
  "schema": "openagents.mkt-swp.exit.v1",
  "profile": "mkt-swp",
  "profile_version": 1,
  "order_id": "<64-lower-hex>",
  "swap_contract_ids": ["<requester event id>", "<provider event id>"],
  "contract_sha256": "<64-lower-hex>",
  "participant_role": "requester",
  "leg_id": "source",
  "network_id": "bip122:<reference>",
  "asset_id": "<asset_id>",
  "effect_id": "<64-lower-hex>",
  "funding": {
    "transaction_id": null,
    "transaction_template_sha256": "<64-lower-hex>",
    "output_index": 0,
    "amount": "100000",
    "script_pubkey": "<lowercase hex>",
    "confirmation_policy_sha256": "<64-lower-hex>"
  },
  "exit": {
    "mode": "presigned|wallet_sign|external_signer",
    "path": "claim|refund",
    "transaction_template_sha256": "<64-lower-hex>",
    "signed_transaction": null,
    "signer_ref": null,
    "transaction_version": 2,
    "lock_time": 0,
    "input_sequence": 0,
    "sighash_type": "DEFAULT",
    "destination_script_pubkey": "<lowercase hex>",
    "earliest_broadcast_height": "0",
    "latest_safe_broadcast_height": "0",
    "fee_policy": {
      "target_blocks": 2,
      "maximum_fee": "1000",
      "bump_mode": "cpfp|replacement_forbidden"
    }
  },
  "verification": {
    "swap_tree_sha256": "<64-lower-hex>",
    "quote_id": "<64-lower-hex>",
    "verifier_digest": "<64-lower-hex>"
  },
  "secret_commitments": {
    "payment_hash": "<64-lower-hex>",
    "preimage_recovery_ref": null
  },
  "broadcast": {
    "esplora_urls": ["https://example.invalid/api"],
    "minimum_agreeing_sources": 1
  }
}
```

The package contains a fully signed unilateral transaction when the rail and
known outpoint permit it. Otherwise it contains the complete deterministic
transaction template plus a non-secret wallet or external-signer handle. It
MUST NOT contain a seed, raw private key, undeclared derivation secret,
preimage, macaroon, NWC string, bearer token, or MuSig2 secret nonce.
Encryption of local storage does not make those values valid relay or server
artifacts.

For `mode=presigned`, `transaction_id` and `signed_transaction` are lowercase
hex and `signer_ref` is `null`. For `mode=wallet_sign` or
`mode=external_signer`, `signed_transaction` is `null` and `signer_ref` is an
opaque local non-secret reference. `transaction_id` may be `null` only when
the funding transaction is not yet signed; the template digest and output
index then bind the future outpoint without changing this package.
`preimage_recovery_ref` follows the same local-reference rule and is present
only for the participant that owns the preimage. No local reference appears
in a Swap Contract; only its package digest does.

The requester additionally persists the reverse-swap preimage in its own
secret store and binds only the payment hash and a local recovery handle in
the package. The provider persists the hold-invoice lookup handle in its own
node state. Neither secret crosses the relay/server artifact boundary.

### 12.1 Doomsday drill

The mandatory recovery test permanently removes every market relay handler,
provider API, WebSocket, catalog, and coordinator after funds move. Each
participant receives only:

- its persisted signed NIP-MKT records;
- its own exit packages and local wallet or node state;
- a direct or relay-agnostic authenticated counterparty channel; and
- Bitcoin and Lightning access, including a keyless transaction broadcaster
  that can use an Esplora-compatible endpoint.

The test passes only when both parties independently derive the same funded
rail objects and reach `completed`, `refunded`, or a justified unilateral
exit. A cooperative recovery exchanges the original signed records and
public transaction material, constructs or completes the bound transaction,
and verifies every signature before broadcast. If the counterparty is gone,
each principal executes its unilateral timeout path. A provider database
snapshot or reconstructed UI state cannot fill a missing signed record.

The keyless broadcaster accepts only a complete signed transaction and public
chain parameters. It does not accept keys or signing authority. An exit
package that depends on a vanished coordinator to obtain a signature,
transaction template, fee rule, timeout, or counterparty identity is
`swp_exit_package_unusable`.

## 13. Idempotency and external-effect binding

Before an irreversible operation, its actor derives:

```text
effect_id = sha256(
  "openagents.mkt-swp.v1" || 0x00 ||
  order_event_id_bytes || 0x00 ||
  effect_role_ascii || 0x00 ||
  leg_id_ascii
)
```

Allowed `effect_role` values are `reserve`, `invoice_create`,
`invoice_pay`, `invoice_settle`, `invoice_cancel`, `chain_fund`,
`chain_claim`, `chain_refund`, `cooperative_sign`, and
`evidence_publish`. `leg_id` is `source`, `destination`, or `lightning`.

The actor persists `effect_id`, exact Order ID, request digest, external rail
identifier, and result before it reports success. An identical replay returns
the persisted result. A replay with different input is
`swp_idempotency_conflict`. One effect ID MUST bind to at most one invoice,
outpoint, claim, refund, signature transcript, or evidence artifact. A wallet
or provider crash cannot cause a second payment, lock, claim, refund, or
reservation.

Delivery retries re-wrap the same signed NIP-MKT record. They do not create a
new event, reservation, or external effect.

## 14. Privacy classification and metadata leaks

Public fields are limited to provider identity, profile version, supported
network and asset identifiers, side ranges, advertised `fee_bps`, supported
swap/script/proof/confirmation classes, availability label, terms URL, and
bounded public attestations.

Pairwise private fields include RFQ constraints, exact amounts, invoices,
payment hashes, claim/refund public keys, scripts, addresses, outpoints,
reservation references, effect IDs, status, evidence, disputes, recovery
channels, and exit-package digests. These records use the NIP-MKT signed-inner
gift-wrap transport.

The following material MUST NOT enter a relay, coordination handler, public
receipt, server log, contract export fixture, telemetry event, or provider
status artifact:

- wallet seeds or private extended keys;
- raw claim or refund private keys;
- preimages before their safe rail revelation;
- Lightning node macaroons, NWC connection strings, or bank credentials;
- MuSig2 secret nonces or unprotected signing sessions; and
- bearer credentials or raw wallet RPC payloads.

Even with gift wrap, relays can observe network addresses, wrapper timing,
recipient inboxes, traffic volume, and retention patterns. Exact swap type,
amount, assets, invoice, scripts, and evidence are hidden from a relay that is
not an intended recipient. A configured coordination handler learns the
minimum private terms required by its declared reservation or timer function.
The Quote identifies that disclosure and its retention period.

A Public Market Receipt omits session ID, counterparty, amount, asset pair,
route, payment hash, invoice, transaction ID, timing ladder, and evidence
unless both parties gave field-specific consent. The receipt outcome cannot be
stronger than the signer's Close or the highest verified evidence rung.
Allowed receipt outcomes are `completed`, `cancelled`, `expired`, `failed`,
`refunded`, `disputed`, and `unresolved`. A `rejected` Close has no MKT-SWP
public receipt because NIP-MKT does not define that public outcome.

## 15. Terminal outcomes and loss accounting

Every Close includes:

```json
{
  "loss_accounting": {
    "input_asset_id": "<asset_id>",
    "output_asset_id": "<asset_id>",
    "input_committed": "0",
    "input_recovered": "0",
    "output_received": "0",
    "provider_fee_paid": "0",
    "miner_fee_paid": "0",
    "lightning_routing_fee_paid": "0",
    "guarantee_recovery_received": "0",
    "principal_unresolved": "0",
    "reservation_released": "0",
    "evidence_refs": []
  }
}
```

Every amount is a canonical decimal string. The accounting must balance each
asset separately. Fees are not collapsed into principal. Unknown values use
an explicit `unknown_fields` list; they are not represented as zero.

| Close outcome | Required condition                                                                                             |
| ------------- | -------------------------------------------------------------------------------------------------------------- |
| `completed`   | Both legs satisfy their quoted finality rules and loss accounting is complete.                                 |
| `rejected`    | Order never became effective and no funds or reservation remain.                                               |
| `cancelled`   | Matching effective cancellation exists and no irreversible effect remains.                                     |
| `expired`     | No funded effect remains; every reservation is released.                                                       |
| `failed`      | Attempt terminated, evidence identifies the failure, and remaining principal is zero or explicitly unresolved. |
| `refunded`    | Every funded principal reached a verified refund/release path.                                                 |
| `disputed`    | A contested effect or loss remains under the quoted dispute process.                                           |
| `unresolved`  | At least one rail object, record, exit, or principal disposition is unknown.                                   |

Conflicting Close records remain visible. One party's `completed` does not
force the other's outcome.

## 16. Reserved EVM-leg vocabulary

MKT-SWP v1 does not execute EVM legs. The following object names and value
shapes are reserved so an `mkt-swp-evm` extension can add a SHA-256-compatible
contract leg without renaming Quote fields:

```json
{
  "evm_leg": {
    "chain_id": "1",
    "contract_address": "0x<40-lower-hex>",
    "token_asset_id": "<collision-resistant asset identifier>",
    "claim_signature_mode": "eip712|direct",
    "refund_signature_mode": "eip712|direct",
    "claim_address": "0x<40-lower-hex>",
    "gas_payer": "requester|provider|sponsor",
    "confirmation_policy": {
      "minimum_confirmations": "1",
      "reorg_safety_blocks": "12",
      "zero_confirmation": "forbidden|allowed",
      "replacement_policy": "reject|track"
    },
    "contract_code_sha256": "<64-lower-hex>",
    "domain_separator_sha256": "<64-lower-hex>"
  }
}
```

`chain_id` and all counts are canonical decimal strings. Addresses are
lowercase 20-byte hex with `0x`. The future extension must define token
precision, contract deployment authority, typed-data domain and message,
front-running protection, gas sponsorship, code verification, logs, reorgs,
and claim/refund finality. Arbitrary call composition is a separate critical
capability.

For v1, `evm_leg` MUST be absent or `null`, Offering
`evm_extension` MUST be `unsupported`, and any non-null value fails with
`swp_unsupported_extension`. No v1 implementation may accept the reserved
shape and silently execute it.

## 17. Errors and loss states

Profile errors are stable lowercase identifiers:

| Error                             | Meaning                                                                           |
| --------------------------------- | --------------------------------------------------------------------------------- |
| `swp_unsupported_profile`         | Profile ID is not `mkt-swp`.                                                      |
| `swp_unsupported_version`         | Profile or verifier revision is unsupported.                                      |
| `swp_unsupported_critical_member` | A critical member is unknown.                                                     |
| `swp_unsupported_extension`       | A non-v1 rail or EVM leg was requested.                                           |
| `swp_invalid_asset_id`            | Asset identifier is malformed or unallowlisted.                                   |
| `swp_invalid_pair`                | Ordered asset pair does not match the swap type.                                  |
| `swp_side_disabled`               | Offering has `max="0"` for the selected side.                                     |
| `swp_invalid_amount`              | Amount is non-canonical, zero where executable, out of range, or over `u64`.      |
| `swp_invalid_fee`                 | Fee field is malformed or exceeds policy.                                         |
| `swp_amount_equation_mismatch`    | Output does not reproduce from quoted terms.                                      |
| `swp_quote_expired`               | Quote or reservation is expired.                                                  |
| `swp_order_selection_invalid`     | Order changes a non-selectable term.                                              |
| `swp_contract_missing`            | Both complementary Swap Contract records are not present before funding.          |
| `swp_contract_signer_invalid`     | Swap Contract author or role does not match the Order and Quote.                  |
| `swp_contract_digest_mismatch`    | Party records do not commit the same RFC 8785 contract bytes.                     |
| `swp_contract_terms_mismatch`     | Swap Contract differs from the Quote or allowed Order selection.                  |
| `swp_price_feed_invalid`          | URL, pointer, value, digest, redirect, or response is invalid.                    |
| `swp_price_feed_stale`            | Exact pinned feed observation exceeds maximum age.                                |
| `swp_terms_mismatch`              | Invoice, script, amount, policy, or Status differs from Quote.                    |
| `swp_reservation_missing`         | Required reservation terms or proof are absent.                                   |
| `swp_reservation_expired`         | Reservation expired before effective Order.                                       |
| `swp_reservation_overallocated`   | Active reservation sum exceeds committed capacity.                                |
| `swp_reservation_fork`            | Provider equivocated in reservation sequence or active set.                       |
| `swp_reservation_proof_invalid`   | Proof does not establish its claimed class.                                       |
| `swp_covenant_reserve_invalid`    | Covenant, amount, fill rule, or double-use check failed.                          |
| `swp_timeout_ladder_unsafe`       | Required height/time inequality is false or not computable.                       |
| `swp_invoice_invalid`             | Invoice parse, network, amount, expiry, hash, or CLTV check failed.               |
| `swp_payment_hash_mismatch`       | Invoice, script, Quote, and local secret commitment disagree.                     |
| `swp_script_invalid`              | Script or Taproot tree is malformed or unsupported.                               |
| `swp_script_commitment_mismatch`  | Re-derived scriptPubKey/address differs.                                          |
| `swp_liquid_network_mismatch`     | Elements genesis reference or pegged asset differs from signed terms.             |
| `swp_liquid_output_invalid`       | Elements output envelope, commitments, index, or fee shape is invalid.            |
| `swp_liquid_unblind_failed`       | The participant cannot unblind its selected confidential output.                  |
| `swp_liquid_unblind_mismatch`     | Locally revealed asset, amount, script, or output differs from signed terms.      |
| `swp_musig_transcript_invalid`    | Key order, tweak, nonce, message, or partial signature check failed.              |
| `swp_confirmation_insufficient`   | Rail object has fewer confirmations than quoted.                                  |
| `swp_rbf_policy_violation`        | Funding violates replacement policy.                                              |
| `swp_replacement`                 | A tracked transaction was replaced.                                               |
| `swp_reorg`                       | A previously observed or finality-pending transaction was displaced.              |
| `swp_funding_not_authorized`      | Verify-before-fund or external wallet authority is absent.                        |
| `swp_status_signer_invalid`       | Author cannot claim the stated action.                                            |
| `swp_status_transition_invalid`   | Transition is not in the selected state machine.                                  |
| `swp_status_gap`                  | Author sequence is incomplete.                                                    |
| `swp_status_fork`                 | Author signed two records at one sequence.                                        |
| `swp_cancel_ineffective`          | Cancellation lacks consent or follows an irreversible effect.                     |
| `swp_evidence_unavailable`        | Bound evidence cannot be retrieved.                                               |
| `swp_evidence_mismatch`           | Artifact, digest, terms, or rail view differs.                                    |
| `swp_settlement_overclaim`        | Claimed rung exceeds verifier evidence.                                           |
| `swp_exit_package_missing`        | Required package was not persisted before funding.                                |
| `swp_exit_package_mismatch`       | Package digest or terms differ from the Swap Contract.                            |
| `swp_exit_package_unusable`       | Package still depends on unavailable coordination or secrets it does not possess. |
| `swp_secret_material_forbidden`   | A relay/server artifact contains prohibited custody material.                     |
| `swp_privacy_violation`           | Field audience or receipt consent was exceeded.                                   |
| `swp_external_effect_conflict`    | One effect ID maps to different external operations.                              |
| `swp_idempotency_conflict`        | Same signed-record or effect key has changed input.                               |
| `swp_refund_failed`               | A required refund was rejected, conflicted, or became unsafe.                     |
| `swp_coordinator_unavailable`     | Coordination is unavailable; client must enter direct recovery.                   |
| `swp_unresolved_loss`             | Terminal principal, fee, record, or rail state remains unknown.                   |

Unsupported versions and critical extensions fail closed. Implementations
retain the original signed record, error, observed rail view, and every
amount affected. They MUST NOT replace a missing record with current provider
database state or hide a fork behind one chosen event.

## 18. Conformance fixture manifest

The Profile Descriptor pins the digest of a corpus containing these exact
fixture names. Each event fixture includes exact signed bytes, expected event
ID, validation result, error code where applicable, and privacy classification.
Rail fixtures include deterministic Bitcoin transactions, scripts, invoices,
heights, verifier inputs, and expected evidence rung.

### 18.1 Positive

| Fixture                                    | Required result                                                                                                           |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| `swp-v1-offering-btc-ln-enabled`           | Accept canonical pair, bounds, fee promise, and `evm_extension=unsupported`.                                              |
| `swp-v1-contract-matching-bilateral`       | Accept requester/provider `kind:39610` records with one shared contract digest and complementary roles.                   |
| `swp-v1-submarine-regtest-cooperative`     | Complete invoice payment and MuSig2 provider claim after all verification gates.                                          |
| `swp-v1-submarine-regtest-script-refund`   | Provider disappears; requester broadcasts the bound refund after `H_refund`.                                              |
| `swp-v1-reverse-regtest-cooperative`       | Hold invoice, provider lock, requester claim, preimage settlement, and finality complete.                                 |
| `swp-v1-reverse-regtest-provider-refund`   | Requester never claims; provider refunds and cancels held Lightning HTLCs.                                                |
| `swp-v1-reverse-hold-expiry-at-minimum`    | Accept observed shortest incoming expiry equal to signed `hold_expiry_height`.                                            |
| `swp-v1-reverse-hold-expiry-above-minimum` | Accept observed shortest incoming expiry above signed `hold_expiry_height`.                                               |
| `swp-v1-chain-regtest-cooperative`         | Provider signs the source-funding instruction; source and destination locks, requester claim, provider claim, both final. |
| `swp-v1-chain-regtest-dual-refund`         | Destination is unclaimed; provider and requester execute the safe ordered refunds.                                        |
| `swp-v1-liquid-submarine-regtest-refund`   | Requester verifies its pegged-asset output, persists the Liquid exit, and refunds after timeout.                          |
| `swp-v1-liquid-reverse-regtest-claim`      | Requester unblinds the provider lock, verifies amount/asset/script, and claims without coordinator keys.                  |
| `swp-v1-btc-liquid-chain-regtest`          | Both chain legs verify their own asset, output, timeout, and unilateral exit before either funds.                         |
| `swp-v1-covenant-hard-reservation`         | Independent verifier admits the covenant-enforced minimum once.                                                           |
| `swp-v1-price-feed-exact-pointer`          | Exact URL, response digest, RFC 6901 pointer, and observed value reproduce terms.                                         |
| `swp-v1-public-receipt-redacted`           | Receipt exposes only consented outcome and verifier reference.                                                            |

### 18.2 Negative verification and grammar

| Fixture                                                    | Expected error                                                              |
| ---------------------------------------------------------- | --------------------------------------------------------------------------- |
| `swp-v1-negative-ticker-pair`                              | `swp_invalid_asset_id`                                                      |
| `swp-v1-negative-json-number-amount`                       | `swp_invalid_amount`                                                        |
| `swp-v1-negative-leading-zero-amount`                      | `swp_invalid_amount`                                                        |
| `swp-v1-negative-disabled-side`                            | `swp_side_disabled`                                                         |
| `swp-v1-negative-fee-promise-change`                       | `swp_terms_mismatch`                                                        |
| `swp-v1-negative-feed-redirect`                            | `swp_price_feed_invalid`                                                    |
| `swp-v1-negative-feed-substitute-host`                     | `swp_price_feed_invalid`                                                    |
| `swp-v1-negative-feed-pointer`                             | `swp_price_feed_invalid`                                                    |
| `swp-v1-negative-feed-stale`                               | `swp_price_feed_stale`                                                      |
| `swp-v1-negative-invoice-network`                          | `swp_invoice_invalid`                                                       |
| `swp-v1-negative-invoice-amountless`                       | `swp_invoice_invalid`                                                       |
| `swp-v1-negative-payment-hash`                             | `swp_payment_hash_mismatch`                                                 |
| `swp-v1-negative-taproot-tree`                             | `swp_script_commitment_mismatch`                                            |
| `swp-v1-negative-liquid-ticker-asset`                      | `swp_invalid_asset_id`                                                      |
| `swp-v1-negative-liquid-pegged-asset`                      | `swp_liquid_network_mismatch`                                               |
| `swp-v1-negative-liquid-output-commitment`                 | `swp_liquid_output_invalid`                                                 |
| `swp-v1-negative-liquid-unblind-foreign-output`            | `swp_liquid_unblind_failed`                                                 |
| `swp-v1-negative-liquid-unblind-amount`                    | `swp_liquid_unblind_mismatch`                                               |
| `swp-v1-negative-liquid-bitcoin-sighash`                   | `swp_liquid_output_invalid`                                                 |
| `swp-v1-negative-refund-key`                               | `swp_terms_mismatch`                                                        |
| `swp-v1-negative-timeout-ladder`                           | `swp_timeout_ladder_unsafe`                                                 |
| `swp-v1-negative-hold-expiry-below-minimum`                | `swp_timeout_ladder_unsafe`                                                 |
| `swp-v1-negative-rbf-forbidden`                            | `swp_rbf_policy_violation`                                                  |
| `swp-v1-negative-insufficient-confirmations`               | `swp_confirmation_insufficient`                                             |
| `swp-v1-negative-order-mutation`                           | `swp_order_selection_invalid`                                               |
| `swp-v1-negative-bare-swap-contract`                       | NIP-MKT bare-private rejection `restricted: mkt-private-requires-gift-wrap` |
| `swp-v1-negative-contract-one-signer`                      | `swp_contract_missing`                                                      |
| `swp-v1-negative-contract-role`                            | `swp_contract_signer_invalid`                                               |
| `swp-v1-negative-contract-digest-fork`                     | `swp_contract_digest_mismatch`                                              |
| `swp-v1-negative-contract-order-mismatch`                  | `swp_contract_terms_mismatch`                                               |
| `swp-v1-negative-nonnull-evm-leg`                          | `swp_unsupported_extension`                                                 |
| `swp-v1-negative-missing-exit-package`                     | `swp_exit_package_missing`                                                  |
| `swp-v1-negative-provider-claims-requester-verification`   | `swp_status_signer_invalid`                                                 |
| `swp-v1-negative-requester-claims-source-funding-required` | `swp_status_signer_invalid`                                                 |
| `swp-v1-negative-settlement-overclaim`                     | `swp_settlement_overclaim`                                                  |

### 18.3 Reservation, replay, fork, and expiry

| Fixture                                 | Required result                                                                     |
| --------------------------------------- | ----------------------------------------------------------------------------------- |
| `swp-v1-replay-identical-order`         | Return the persisted result; create no second external effect.                      |
| `swp-v1-replay-rewrapped-quote`         | Admit the same signed Quote once and preserve its reservation.                      |
| `swp-v1-replay-rewrapped-swap-contract` | Admit the same signed Swap Contract once and preserve the original contract result. |
| `swp-v1-conflict-swap-contract-d`       | Reject changed bytes under one `(pubkey,39610,d)` with `swp_idempotency_conflict`.  |
| `swp-v1-replay-effect-after-crash`      | Recover the original external identifier and result.                                |
| `swp-v1-conflict-order-idempotency`     | Reject with `swp_idempotency_conflict`.                                             |
| `swp-v1-conflict-effect-binding`        | Reject with `swp_external_effect_conflict`.                                         |
| `swp-v1-reservation-overallocation`     | Reject new allocation with `swp_reservation_overallocated`.                         |
| `swp-v1-reservation-sequence-fork`      | Retain both claims and surface `swp_reservation_fork`.                              |
| `swp-v1-covenant-double-count`          | Reject with `swp_covenant_reserve_invalid`.                                         |
| `swp-v1-status-gap`                     | Retain history and expose `swp_status_gap`.                                         |
| `swp-v1-status-fork`                    | Retain both Status records and expose `swp_status_fork`.                            |
| `swp-v1-expired-quote`                  | Refuse Order; release only reservation state.                                       |
| `swp-v1-expired-after-funding`          | Enter refund graph; do not emit effective cancellation.                             |

### 18.4 Privacy and custody

| Fixture                                       | Required result                                                                                          |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `swp-v1-privacy-public-offering`              | Secret scanner finds no exact amount, live inventory, invoice, address, script, or reserve witness.      |
| `swp-v1-privacy-wrap-recipient`               | Only requester, provider, sender recovery copy, and declared handler can decrypt their intended records. |
| `swp-v1-privacy-public-receipt-overreach`     | Reject with `swp_privacy_violation`.                                                                     |
| `swp-v1-privacy-seed-tripwire`                | Reject with `swp_secret_material_forbidden`.                                                             |
| `swp-v1-privacy-preimage-tripwire`            | Reject unsafe relay/server artifact with `swp_secret_material_forbidden`.                                |
| `swp-v1-privacy-macaroon-tripwire`            | Reject with `swp_secret_material_forbidden`.                                                             |
| `swp-v1-privacy-claim-key-tripwire`           | Reject with `swp_secret_material_forbidden`.                                                             |
| `swp-v1-privacy-musig-secret-nonce-tripwire`  | Reject with `swp_secret_material_forbidden`.                                                             |
| `swp-v1-privacy-liquid-blinding-key-tripwire` | Reject with `swp_secret_material_forbidden`.                                                             |

### 18.5 Reorg, replacement, and recovery

| Fixture                                     | Required result                                                                             |
| ------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `swp-v1-reorg-funding-before-finality`      | Lower local rung, record both views, and wait or recover.                                   |
| `swp-v1-reorg-claim-before-finality`        | Remove settlement claim and resume claim/refund monitoring.                                 |
| `swp-v1-replacement-tracked`                | Bind replacement lineage under quoted policy.                                               |
| `swp-v1-doomsday-submarine-provider-gone`   | Requester refunds from signed records and exit package with coordinator permanently absent. |
| `swp-v1-doomsday-reverse-coordinator-gone`  | Counterparties complete or provider refunds/cancels using direct recovery and rail state.   |
| `swp-v1-doomsday-chain-counterparty-gone`   | Each principal executes its unilateral timeout path in safe order.                          |
| `swp-v1-doomsday-liquid-coordinator-gone`   | Participant unblinds its own bound output and executes the persisted Liquid script path.    |
| `swp-v1-doomsday-keyless-esplora-broadcast` | Static executor broadcasts a pre-signed exit without any key material.                      |
| `swp-v1-recovery-missing-signed-record`     | Report explicit loss and refuse reconstructed history.                                      |
| `swp-v1-recovery-unusable-exit-package`     | Fail with `swp_exit_package_unusable` before funding.                                       |
| `swp-v1-recovery-mutual-close-disagreement` | Preserve both Close records and their separate evidence.                                    |

## 19. Relay and client conformance

A relay-observable MKT-SWP implementation validates Offering field grammar,
side-disable semantics, asset identifiers, fee bounds, allowed public receipt
outcomes, profile/version, and typed evidence-reference shapes. It cannot
claim to have verified encrypted rail terms it cannot decrypt.

It rejects bare `kind:39610`, preserves an admitted signed inner Swap Contract
as immutable under `(pubkey, kind, d)`, and applies the same recipient gate to
its wraps on filters, ID reads, counts, search, and live fan-out. A handler
that is an intended wrap recipient additionally validates the complementary
signers, shared contract digest, Order/Quote references, per-leg shape, and
secret-material tripwires before it reports `contract_bound`.

An optional coordination handler advertises `mkt-swp:1` only when it passes
the reservation, timer, replay, fork, evidence-observation, and privacy
fixtures under the active configuration. A transport-only relay may advertise
NIP-MKT base support without advertising executable MKT-SWP.

A conforming client implements the lifecycle, verify-before-fund, external
effect, privacy, exit-package, and doomsday fixtures. A client that only
parses records is not an executable MKT-SWP client.

A client or provider that advertises a Liquid pair additionally passes the
Section 7.5 Elements parser, pegged-asset, own-output unblinding,
verify-before-fund, unilateral-exit, reorg, and doomsday fixtures against its
configured `elementsd`. It describes confidential-transaction authority as
local-node plus own-output unblinding. It MUST NOT claim independent proof
verification unless a later profile and implementation add and test the
required secp256k1-zkp primitives.

## References

- [NIP-MKT](MKT.md)
- BIP-122 chain references
- BIP-341 and BIP-342 Taproot and tapscript
- BIP-174 PSBT and BIP-370 PSBT v2
- BOLT 11 invoices and Lightning HTLC settlement
- Elements transaction format and Confidential Assets RPC workflow
- RFC 6901 JSON Pointer
- [Boltz ecosystem teardown](../teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md)
- [Satora/LendaSwap outage teardown](../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md)
- [Arkade, solver, Mostro, Cashu, and WDK teardown](../teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)

## Changelog

**v1 Liquid addendum (2026-08-05)**

- Added Elements network and pegged-asset identifiers without allocating a
  new event kind.
- Added Liquid legs for submarine, reverse, and BTC/L-BTC chain swaps.
- Defined own-output unblinding, exact commitment binding, local `elementsd`
  authority, unilateral exits, errors, and conformance fixtures.
- Kept arbitrary third-party range-proof and surjection-proof verification
  outside v1 claims.

**v1 draft correction (2026-08-04)**

- Defined `hold_expiry_height` and `H_hold_expiry` as the signed minimum
  acceptable shortest incoming HTLC expiry. The provider compares its node's
  observed shortest expiry with `>=` before it funds the reverse-swap chain
  lock.
- Added conformance fixtures for the equality/above boundary and the
  below-minimum refusal.

**v1 draft (2026-08-04)**

- Defined BTC/Lightning submarine, reverse, and chain swap terms,
  verification, lifecycle, timeouts, reservation proofs, evidence, recovery,
  loss accounting, privacy, errors, and fixtures.
- Reserved the EVM-leg field vocabulary without enabling EVM execution.
- Assigned `kind:39610` to the private immutable Swap Contract and reserved
  `39611-39619` for later collision-reviewed revisions.
