# NIP-MKT-PFI

## Credentialed Payment-Facilitator Ramps

`draft` `optional`

This document defines version 1 of the `mkt-pfi` profile for NIP-MKT. It
covers a bilateral on-ramp or off-ramp between one fiat asset and one
cryptographic asset when qualification, credentials, reversible settlement,
or regulated recourse is part of the route.

NIP-MKT supplies discovery, signed negotiation, private delivery, replay,
sequencing, cancellation, and close records. This profile supplies the fields
and rules that make a PFI route interpretable. The external rails, credential
issuers, settlement institutions, guarantors, escrow authorities, and dispute
authorities retain their own authority.

> A relay transports and validates bounded records. It never becomes a
> credential verifier, payment institution, guarantor, escrow, adjudicator,
> legal authority, or settlement authority.

## Profile identity

An executable configuration pins all of these values:

| Field                 | Value or rule                                                                                                                                                  |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `profile_id`          | `mkt-pfi`                                                                                                                                                      |
| `profile_version`     | `1`                                                                                                                                                            |
| Profile authority key | The exact author pubkey of the selected NIP-MKT `kind:39602` Profile Descriptor. Clients MUST allowlist this key; there is no universal PFI profile authority. |
| Specification digest  | The descriptor's `x` tag, equal to the SHA-256 digest of the exact bytes of this document revision.                                                            |
| Fixture corpus        | The exact manifest in [Fixture manifest](#fixture-manifest), with a corpus digest pinned by the descriptor.                                                    |

A client that has not pinned the authority pubkey, descriptor event ID,
specification digest, profile version, and fixture-corpus digest MUST treat the
profile as unsupported.

## Allocation and collision review

The `39630-39639` family is reserved for MKT-PFI. Version 1 allocates one
kind:

| Kind          | Type             | Description                                               | Publication    |
| ------------- | ---------------- | --------------------------------------------------------- | -------------- |
| `39630`       | Addressable head | PFI Qualification Policy                                  | public, no PII |
| `39631-39639` | —                | Reserved for a later separately reviewed MKT-PFI revision | unallocated    |

`kind:39630` is needed because a provider's reusable qualification rules have
a different signer, replacement cadence, and privacy schema from an Offering.
An Offering and every Quote pin one exact policy event and digest. Policy
replacement therefore cannot change a Quote already issued.

Version 1 allocates no credential, guarantee, settlement, or dispute event
kind. Credential presentations stay off-relay and are represented by private
commitments. NIP-MKT base records carry negotiation state. NIP-EV or an
external authority carries evidence. This avoids giving a profile record
authority it does not have.

The allocation was reviewed against the four pinned sources below on
2026-08-04. None assigned `39630-39639`:

| Lane                               | Reviewed commit                            | Result                           |
| ---------------------------------- | ------------------------------------------ | -------------------------------- |
| Official NIPs                      | `c53877571f96eb423661fc23c620d629d37b8f19` | unused                           |
| Block lane                         | `56003ebf98c22367fb6357f295494e26efbd8ae6` | unused                           |
| OpenAgents lane before this draft  | `1c9a957ab8275e23e7952af2395d36c681b5246d` | reserved by NIP-MKT, unallocated |
| `nostr-protocol/registry-of-kinds` | `2483e752146d171524dcb10dffd06de2aa271bf3` | unused                           |

Every later allocation in `39631-39639` requires a new three-lane and registry
review. A local implementation repeats and records the review before it
advertises this profile.

## Terms

- **PFI** — the provider that publishes the Offering and Quote and coordinates
  the route.
- **requester** — the party that requests terms and signs the Order.
- **qualification policy** — public provider-signed requirements, never a
  credential presentation or eligibility decision.
- **credential issuer** — the authority that signs an accepted credential.
- **credential verifier** — the implementation that checks the issuer,
  presentation, audience, purpose, challenge, expiry, and policy.
- **settlement institution** — a bank, payment network, chain, token system,
  custodian, or other external system that operates a leg.
- **guarantee authority** — the signer that assumes a defined failure
  obligation.
- **reserve verifier** — the authority or adapter that verifies the reserve
  evidence named by a guarantee.
- **dispute authority** — the named adjudicator or process for one route.

One actor may hold several roles only when the Quote says so. A UI MUST render
each role separately and MUST NOT infer independence.

## Common encoding

### Asset and market identity

Version 1 supports exactly one fiat asset and one cryptographic asset per
market.

- A fiat asset ID is `iso4217:` followed by exactly three uppercase ASCII
  letters, for example `iso4217:USD`.
- A cryptographic asset ID is `caip19:` followed by a canonical CAIP-19 asset
  ID. Its chain namespace and asset namespace are lowercase. The selected rail
  adapter MUST validate the complete chain and asset references against its
  pinned registry.
- A ticker, display name, symbol, icon, or unqualified contract address is not
  an asset ID.
- `market_id` is lowercase hex SHA-256 of the UTF-8 bytes
  `mkt-pfi-v1\0<fiat_asset_id>\0<crypto_asset_id>`. The two NUL bytes are
  literal `0x00` separators.

The ordered asset-ID pair identifies the market. `direction` identifies which
asset the requester pays:

| Direction  | Requester pays      | Requester receives  |
| ---------- | ------------------- | ------------------- |
| `on_ramp`  | fiat asset          | cryptographic asset |
| `off_ramp` | cryptographic asset | fiat asset          |

Each asset term also names `atomic_unit_exponent`, a canonical decimal string
from `"0"` through `"18"`, and a public `unit_registry_ref` plus
`unit_registry_digest`. Clients MUST validate the exponent against the pinned
registry or adapter. A Quote cannot redefine an asset's unit.

### Amounts, limits, fees, and time

Every amount is a string of atomic units matching
`^(0|[1-9][0-9]*)$`. JSON numbers, signs, separators, decimal points,
exponents, leading zeroes, and values above the profile implementation's
published integer bound are invalid.

Each Offering publishes `on_ramp` and `off_ramp` limits. A side contains
`pay_asset_id`, `receive_asset_id`, `min`, and `max`:

- `max="0"` disables the side and requires `min="0"`;
- an enabled side requires `0 < min <= max`; and
- an RFQ, Quote, or Order for a disabled side fails with
  `pfi_side_disabled`.

`fee_bps` is a canonical decimal string from `"0"` through `"10000"`.
It is a provider fill promise, not proof of the fee charged. A Quote also
names `fee_basis_asset_id`, `fee_basis_amount`, `fee_asset_id`, and exact
`fee_amount`. `fee_asset_id` MUST equal `fee_basis_asset_id`; a fee in another
asset uses a separate named term and conversion rule. The exact fee MUST NOT exceed
`floor(fee_basis_amount * fee_bps / 10000)`. Network, institution, escrow, and
tax amounts are separate named fields and MUST NOT be hidden in
`fee_amount`. Close records compare charged amounts to the promise.

Unix times and durations are canonical non-negative decimal strings. Every
deadline in a Quote is absolute Unix seconds. `created_at` is not a trusted
clock.

### Price-feed pinning

A Quote uses `pricing.mode="fixed"` or `pricing.mode="pinned_feed"`. Both
modes state exact `pay_amount` and `receive_amount`; an Order never computes
new terms implicitly.

`pinned_feed` additionally requires:

```json
{
  "url": "https://prices.example/market",
  "value_pointer": "/data/price",
  "payload_sha256": "<64-lower-hex>",
  "retrieved_at": "<unix-seconds>",
  "max_age_seconds": "30",
  "value": "1234.5678",
  "value_scale": "4",
  "rounding": "down",
  "pricing_commitment_sha256": "<64-lower-hex>"
}
```

The URL MUST be absolute HTTPS with no user information, fragment, bearer
token, or secret-shaped query member. `value_pointer` is an RFC 6901 JSON
Pointer. `value` is a non-negative canonical decimal string with no exponent;
`value_scale` states its decimal places. `pricing_commitment_sha256` commits
to the exact URL, pointer, payload digest, value, scale, rounding rule, asset
IDs, amounts, and fee terms.

Both parties validate the same URL and pointer before Order acceptance. A
substitute URL, redirect to another authority, stale response, pointer miss,
payload mismatch, or changed value fails closed. A price feed sets no
settlement fact and gains no payment authority.

## PFI Qualification Policy (`kind:39630`)

Address:
`39630:<provider_pubkey>:<qualification_policy_id>`.

The PFI signs this public replacement head. Required tags are:

- stable `d` equal to `qualification_policy_id`;
- `profile` tag exactly `["profile", "mkt-pfi", "1"]`;
- `status`: `active`, `paused`, or `retired`;
- positive decimal `version`;
- `published_at`;
- `x`, the SHA-256 digest of the exact content bytes; and
- fixed `alt`: `MKT-PFI qualification policy`.

An active policy SHOULD have NIP-40 `expiration`. Policy replacement uses the
same address and a larger policy `version`. A Quote pins the exact event ID,
content digest, and version. A later head never changes that Quote.

Content uses the NIP-MKT envelope and this closed object:

```json
{
  "schema": "openagents.mkt.v1",
  "profile": "mkt-pfi",
  "profile_version": 1,
  "qualification_policy_id": "retail-us-v1",
  "policy_version": "1",
  "jurisdictions": ["US"],
  "requirements": [
    {
      "requirement_id": "identity-basic",
      "credential_schema_id": "https://schemas.example/kcc/v1",
      "accepted_issuer_ids": ["did:key:<issuer>"],
      "claim_types": ["identity_verified", "jurisdiction_resident"],
      "presentation_format": "w3c-vp",
      "presentation_stage": "post_quote_pre_acceptance",
      "maximum_credential_age_seconds": "2592000"
    }
  ],
  "retention": {
    "policy_url": "https://provider.example/credential-retention",
    "policy_sha256": "<64-lower-hex>",
    "maximum_seconds": "86400",
    "deletion_request_url": "https://provider.example/privacy"
  }
}
```

The object is closed: unknown members at every policy-defined nesting level
fail with `pfi_policy_unknown_member`. Identifiers and arrays use NIP-MKT
bounds. `jurisdictions` contains ISO 3166-1 alpha-2 codes. Schema and policy
URLs are public identifiers, not retrieval capabilities. Issuer IDs identify
public issuer keys or DIDs and carry no subject identifier.

Policy content is at most 16 KiB and the event has at most 64 tags. A policy
has at most 16 requirements, 16 accepted issuers per requirement, 32 claim
types per requirement, and 32 jurisdictions. Policy URLs are at most 512
ASCII characters; policy identifiers are at most 128 ASCII characters. A
limit hit fails closed and does not truncate the interpreted policy.

`claim_types` names predicates the provider needs. It contains no result or
subject value. Version 1 requires
`presentation_stage="post_quote_pre_acceptance"`. A provider may issue a
Quote before learning whether the requester satisfies the policy. If verified
facts require different terms, the provider MUST issue a new Quote with a new
event ID; the requester must accept that exact replacement.

The public policy MUST NOT contain a name, date of birth, address, email,
phone number, government identifier, credential identifier, subject DID,
account or routing number, wallet address tied to a subject, credential bytes,
presentation bytes, access token, cookie, authorization header, retrieval
secret, bank instruction, or user-specific decision. A relay can enforce the
closed shape and reject forbidden member names and bearer-shaped values. It
cannot certify that arbitrary text is free of PII, so profile public objects
avoid free-form fields.

## Public Offering fields

A `kind:39601` Offering for this profile requires:

- `profile=mkt-pfi`, version `1`;
- a `market` tag containing `market_id`;
- one `a` reference to the `kind:39630` policy address, marked
  `qualification-policy`;
- one `e` reference to the exact policy event ID, marked
  `qualification-policy`;
- at least one `direction` tag for an enabled side;
- one or more `risk` tags naming supported route classes; and
- one or more `rail` tags naming public rail-adapter identifiers.

The profile content is closed and contains:

```json
{
  "schema": "openagents.mkt.v1",
  "profile": "mkt-pfi",
  "profile_version": 1,
  "pfi": {
    "market_id": "<64-lower-hex>",
    "fiat_asset": {
      "asset_id": "iso4217:USD",
      "atomic_unit_exponent": "2",
      "unit_registry_ref": "https://www.iso.org/iso-4217-currency-codes.html",
      "unit_registry_digest": "<64-lower-hex>"
    },
    "crypto_asset": {
      "asset_id": "caip19:bip122:000000000019d6689c085ae165831e93/slip44:0",
      "atomic_unit_exponent": "8",
      "unit_registry_ref": "https://example.invalid/asset-registry/v1",
      "unit_registry_digest": "<64-lower-hex>"
    },
    "on_ramp": {
      "pay_asset_id": "iso4217:USD",
      "receive_asset_id": "caip19:bip122:000000000019d6689c085ae165831e93/slip44:0",
      "min": "1000",
      "max": "100000"
    },
    "off_ramp": {
      "pay_asset_id": "caip19:bip122:000000000019d6689c085ae165831e93/slip44:0",
      "receive_asset_id": "iso4217:USD",
      "min": "0",
      "max": "0"
    },
    "fee_bps": "75",
    "qualification_policy_event_id": "<64-lower-hex>",
    "qualification_policy_sha256": "<64-lower-hex>",
    "credential_burden": "basic",
    "rail_ids": ["bank-transfer-us", "bitcoin-mainnet"],
    "risk_classes": ["guaranteed"],
    "jurisdictions": ["US"],
    "custody_dimensions": {
      "funds_control": "disclosed_in_quote",
      "execution_control": "provider_and_external_rails",
      "settlement_authority": "external_rails",
      "reversibility": "rail_specific",
      "recourse": "disclosed_in_quote",
      "credential_exposure": "post_quote_direct_encrypted"
    }
  }
}
```

`credential_burden` is `none`, `basic`, `enhanced`, or `institutional`. It is
a discovery label only; the pinned Qualification Policy is authoritative for
requirements. `risk_classes` lists possible Quote classifications and proves
none of them. Unknown PFI members, free-form public metadata inside `pfi`, a
ticker in place of an asset ID, or a user-specific value fail closed.

## Private negotiation

All profile records use NIP-MKT `kind:39604-39609`, the signed inner-record
transport, and base immutability rules. Credential bytes, PII, bank details,
and bearer retrieval material never enter the inner event, rumor, seal, gift
wrap, relay database, generic search, or public receipt.

### RFQ

An RFQ additionally contains:

- `market_id`, `direction`, `pay_asset_id`, and requested `pay_amount` or a
  bounded `pay_min`/`pay_max` range;
- acceptable `rail_ids` and maximum settlement deadline;
- acceptable risk classes;
- required custody and recourse constraints;
- `credential_mode="quote_first"`; and
- optional credential capability hints containing only credential schema IDs,
  issuer IDs, and presentation formats available to the requester.

The RFQ MUST NOT contain a credential presentation, subject identifier,
account detail, settlement endpoint, or eligibility claim. Capability hints
say only that the requester may be able to produce a compatible presentation.
They prove nothing.

### Quote

A Quote contains complete terms before it requests a credential presentation:

- the exact asset IDs, direction, `pay_amount`, `receive_amount`, atomic units,
  and pricing object;
- `fee_bps`, exact provider fee, every additional fee, and which party pays
  each fee;
- one selected `rail_id` per leg and the leg ordering;
- the exact `risk_classification` and its required proof fields;
- the pinned Qualification Policy address, event ID, digest, and requirement
  IDs;
- a random 64-lower-hex `credential_challenge`, provider pubkey as audience,
  purpose `mkt-pfi-order-qualification`, and `credential_deadline`;
- capacity, reservation class, reservation proof refs, and expiry;
- every timer listed in [Timers](#timers);
- custody dimensions and maximum custody duration;
- dispute, recourse, refund, chargeback, and guarantee terms;
- settlement evidence requirements and exact verifiers; and
- the coordinator-independent recovery package requirements.

A `firm` Quote may make successful qualification an explicit precondition. It
cannot make the price, fee cap, asset IDs, route classification, or recourse
terms depend on undisclosed facts. A provider that changes those terms after
qualification issues a replacement Quote; it cannot mutate or reinterpret the
old one.

Allowed Order selection is limited to:

- one `rail_id` from a Quote-declared set when every choice has identical
  amounts, fees, risk, and deadlines;
- one Quote-declared settlement-window option;
- credential-presentation commitments; and
- public-receipt consent.

Every other term is fixed by the Quote event ID.

### Credential presentation commitments

Credential presentations use a KCC-shaped separation: an issuer signs a
credential; the holder creates an audience-, purpose-, challenge-, and
expiry-bound presentation; the provider verifies it under the pinned policy.
The presentation moves over a direct end-to-end encrypted channel selected by
the parties. NIP-MKT stores only a signed commitment:

```json
{
  "presentation_id": "<64-lower-hex>",
  "presentation_sha256": "<64-lower-hex>",
  "policy_event_id": "<64-lower-hex>",
  "requirement_ids": ["identity-basic"],
  "audience_pubkey": "<provider-64-lower-hex>",
  "purpose": "mkt-pfi-order-qualification",
  "challenge": "<quote-credential-challenge>",
  "expires_at": "<unix-seconds>",
  "transport": "direct-encrypted",
  "channel_ref": "<64-lower-hex>"
}
```

`channel_ref` is a random correlation value. It is not a URL, capability,
token, or retrieval secret. The direct channel conveys any access material;
that material is never copied into a market record. The presentation digest
binds bytes without revealing them.

The Order contains commitments or references a later requester Status that
contains them. The provider reports only `qualification=accepted`,
`rejected`, `expired`, or `revoked` and a bounded machine reason. It does not
echo claims or subject data. Credential verification completes before either
party is instructed to move funds.

Credential verification authority is the selected issuer signature and
provider policy. Relay admission, successful decryption, provider Status, and
possession of a presentation commitment are not qualification proof.

### Risk classification

Every Quote assigns one route-level `risk_classification` and one class to
each leg:

| Class         | Required meaning                                                      | Required Quote fields                                                                |
| ------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `atomic`      | One enforceable consensus condition controls every value leg.         | condition, verifier, timeout, refund path, finality rule                             |
| `escrowed`    | A named party or threshold controls release.                          | escrow authority, funding proof, release/refund conditions, dispute and appeal path  |
| `reserved`    | Capacity is committed, but settlement remains sequential.             | reserve proof class, amount, authority, expiry, verifier, double-reservation rule    |
| `guaranteed`  | A named obligor covers a defined failure under enforceable terms.     | guarantee and reserve objects defined below                                          |
| `best-effort` | Settlement is a counterparty promise with evidence and recourse only. | sequential exposure, missing protection, recourse or explicit absence, recovery path |

The route-level class is no stronger than its weakest uncovered leg. An
atomic crypto leg plus reversible fiat is not an atomic route. A provider
signature, reserve claim, receipt count, reputation score, or model score
cannot create a stronger class.

#### Guarantee and reserve rules

`risk_classification="guaranteed"` requires:

- `guarantee_id` and guarantee authority identifier and signing key;
- order-binding rule and covered failure codes;
- coverage asset ID, maximum amount, expiry, exclusions, deductible, and
  claim deadline;
- governing terms URL and SHA-256 digest;
- claim submission and adjudication authority;
- payout rail, payout deadline, and appeal path;
- `reserve_class`: `onchain`, `escrow`, `covenant`,
  `custodian_attestation`, or `regulated_capital`;
- reserve amount, asset ID, expiry, evidence digest, evidence authority, and
  exact reserve verifier; and
- rules for detecting reuse, encumbrance, revocation, evidence staleness, and
  payout failure.

A covenant-enforced reserve is an admitted reserve proof class when the Quote
names the covenant, committed outpoint or state, amount, verifier, expiry, and
double-spend or replacement policy. A balance signature alone is insufficient.

The guarantee authority signs the obligation. The reserve verifier verifies
the named reserve evidence. The dispute authority decides only the claims in
its terms. The settlement institution settles the payout. None of those roles
is assigned to the relay. A provider's unsupported promise is `best-effort` or
`reserved`, not `guaranteed`. If the provider is also the obligor, the Quote
must name an independent enforcement or adjudication authority and disclose
the role overlap.

Expired, revoked, underfunded, unverified, wrong-asset, already-encumbered, or
non-order-bound reserve evidence fails with `pfi_guarantee_unavailable` before
funds move. Later reserve loss is an explicit loss state and can trigger the
dispute or refund path; it never rewrites earlier evidence.

### Custody dimensions

Every Quote states these independent fields:

- `funds_control` for each leg and phase;
- `execution_control` for each external effect;
- `settlement_authority` for each leg;
- `reversibility`, including who can reverse and until when;
- `recourse`, including an explicit `none` when absent;
- `credential_exposure`, recipients, retention, and deletion path;
- `unilateral_paths`, or `none` with the resulting exposure;
- every applicable timelock; and
- `maximum_custody_seconds` from first observed funding until release,
  settlement, or refund.

The Quote includes a phase table for `pre_funding`, `first_leg_funded`,
`second_leg_pending`, `delivery_complete`, `reversibility_open`, and
`terminal`. For each phase it names the principal controlling each asset and
the independent evidence required to advance. A single custody score is
invalid.

### Settlement evidence

Evidence references are private and use this closed shape:

```json
{
  "evidence_class": "institution_confirmation",
  "evidence_sha256": "<64-lower-hex>",
  "authority_id": "<public authority identifier>",
  "authority_key": "<authority signing key or adapter key>",
  "verifier_id": "<pinned verifier and version>",
  "observed_at": "<unix-seconds>",
  "provenance": "verified",
  "reversibility_until": "<unix-seconds-or-0>",
  "external_operation_ref": "<non-bearer opaque ref>"
}
```

Allowed evidence classes are:

- `rail_receipt` — one rail accepted an operation;
- `institution_confirmation` — the institution reports a transfer state;
- `beneficiary_attestation` — the receiving party signs an observation;
- `escrow_funding` and `escrow_release`;
- `ledger_finality` — the selected ledger verifier reached the quoted policy;
- `reversibility_window_elapsed`;
- `refund_confirmation`;
- `chargeback_confirmation`;
- `guarantee_reserve` and `guarantee_payout`; and
- `dispute_disposition`.

Each Quote leg pins a verifier object containing `adapter_id`,
`adapter_version`, external authority IDs, admitted evidence classes, funding
rule, execution rule, finality rule, confirmation policy, reorg rule,
replacement rule, chargeback rule, and maximum observation age. The selected
adapter applies those exact rules. An unpinned implementation version or a
provider-only database observation cannot advance a leg. For a rule that does
not apply to a rail, the Quote uses the literal `"not-applicable"` and states
why; it does not omit the rule.

`provenance` is `pledged`, `reserved`, `observed`, `verified`, `paid`, or
`settled`. A client does not infer a higher rung. `settled` requires the exact
Quote verifier and either irreversible finality or expiration of the disclosed
reversibility window. Bank screenshots, unverified provider database rows, and
relay retention are not settlement evidence.

An evidence reference contains no account number, counterparty name, transfer
memo, credential, access URL, or bearer token. Authorized verifiers retrieve
private evidence outside the relay by their own access control and verify its
digest.

### Dispute, recourse, refund, and chargeback terms

Every Quote contains:

- provider legal-entity public identifier, jurisdiction, governing-terms URL
  and digest;
- dispute authority identifier and signing key, intake method, evidence
  classes, opening deadline, response SLA, adjudication deadline, and appeal
  authority;
- remedies selected from `refund`, `reperformance`, `escrow_release`,
  `guarantee_claim`, `arbitration`, and `legal_claim`;
- refund authority, asset, amount rule, fee rule, destination commitment,
  deadline, and verifier;
- for each reversible rail, who can reverse, reversal reasons, chargeback
  deadline, reserve duration, evidence, allocation of loss, and recourse; and
- an explicit `recourse="none"` when the route supplies none.

Dispute records contain only a random `dispute_ref`, reason code, evidence
digests, authority refs, and deadlines. PII and narrative evidence stay in the
authority's protected channel. A dispute authority's signature proves only
its disposition under the quoted terms. A relay does not decide or enforce it.

## Signers, verifiers, and authorities

| Record or fact                               | Required signer or verifier                                                                             | Authority limit                         |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| Profile Descriptor                           | allowlisted profile authority key                                                                       | profile bytes and fixture identity only |
| Qualification Policy                         | provider key that owns the referenced Provider Profile                                                  | public requirements only                |
| Offering                                     | provider key                                                                                            | advertised availability only            |
| RFQ, Order                                   | requester key                                                                                           | request and exact acceptance only       |
| Quote, provider Status, provider Close       | provider key                                                                                            | provider terms and observations only    |
| requester Status, Cancel, Close              | requester key                                                                                           | requester intent and observations only  |
| Credential                                   | policy-accepted issuer key                                                                              | issuer claims only                      |
| Presentation                                 | holder proof plus issuer verification, bound to Quote challenge, provider audience, purpose, and expiry | qualification input only                |
| Qualification decision                       | provider credential verifier and policy version                                                         | provider eligibility decision only      |
| Rail funding/finality                        | Quote-pinned rail adapter and external settlement authority                                             | named leg only                          |
| Guarantee                                    | guarantee authority key                                                                                 | exact covered obligation only           |
| Reserve                                      | Quote-pinned reserve verifier and evidence authority                                                    | reserve evidence at observed time only  |
| Escrow release                               | named escrow authority or threshold                                                                     | escrowed leg only                       |
| Dispute disposition                          | named dispute authority; appeal authority when used                                                     | quoted dispute scope only               |
| Relay `OK`, storage, timer, or search result | relay                                                                                                   | transport and relay policy only         |

An implementation MUST reject a state, Cancel, Close, evidence rung, or
qualification result signed by an unauthorized role. It MUST retain and show
conflicting authorized claims.

## State machine

Profile records keep NIP-MKT's base `state` and add `pfi_phase`:

```text
ordered
  -> qualification_pending
  -> qualified | rejected
  -> first_leg_required
  -> first_leg_observed
  -> second_leg_executing
  -> settlement_pending
  -> delivery_complete
  -> completed
```

From any nonterminal funded phase, a permitted branch is:

```text
refund_pending -> refunded
disputed -> completed | refunded | failed | unresolved
failed -> refund_pending | disputed | unresolved
```

Before funding, `rejected`, `cancelled`, and `expired` are terminal. A firm
Quote with no qualification requirement enters `qualified` on a timely valid
Order. An indicative Quote requires provider `Status state=accepted`. A Quote
with requirements enters `qualification_pending`; only the provider's
policy-bound decision advances it.

`first_leg_observed` requires the Quote-pinned verifier for that leg.
`delivery_complete` means the promised asset reached the recipient under the
delivery rule. `completed` may still carry `paid` rather than `settled` when a
disclosed chargeback window remains open. Clients display that residual
exposure. `provenance=settled` is valid only after the profile's settlement
rule passes.

A later chargeback, reorg, guarantee failure, or evidence revocation does not
reopen the old Status chain silently. The discovering signer emits a new Status
and a corrective Close with `previous` references and explicit loss
accounting.

A provider `Status state=funding_required` contains only the digest of the
payment instructions, their expiry, and a random direct-channel correlation
ref. Account numbers, subject-linked addresses, transfer memos, and bearer
access material move through the protected direct channel. The requester
checks the received bytes against the digest before it funds.

Each author uses base sequence rules. A sequence gap remains a gap. Two Status
events by one author for one `(session, order, seq)` are a fork and neither is
selected by timestamp.

## Timers

Every Quote declares these absolute deadlines or the literal `"none"` when
the field is inapplicable:

- `quote_expiration`;
- `reservation_expiration`;
- `credential_deadline`;
- `first_leg_funding_deadline`;
- `second_leg_execution_deadline`;
- `delivery_deadline`;
- `reversibility_until` for each reversible leg;
- `cancel_deadline` before an irreversible step;
- `refund_deadline`;
- `dispute_open_deadline` and `appeal_deadline`; and
- `guarantee_claim_deadline` and `guarantee_payout_deadline`.

The Quote also states maximum accepted clock skew. A receiver uses its own
trusted clock. Expiry releases only the named reservation under its rules. It
does not qualify a user, cancel an Order, reverse a transfer, file a dispute,
claim a guarantee, refund funds, or settle a leg.

## Cancellation

NIP-MKT Cancel remains a request/answer protocol.

- Before qualification or any external effect, the requester may make Cancel
  effective for its Order. Provider reservation release remains separately
  evidenced.
- After qualification and before the first external effect, either party may
  request cancellation. The counterparty or a Quote-named cancellation rule
  signs `effective`.
- After an external effect, Cancel is effective only when the actor that can
  stop that rail supplies the required evidence. Otherwise the session enters
  refund or dispute handling.
- A transfer, release, revealed secret, final ledger operation, or fiat
  settlement is never undone by a Cancel, Close, deletion, or expiry record.

`pfi_cancel_after_irreversible` reports an attempted unsupported cancellation.
It does not discard the request.

## External-effect idempotency

Before an external effect, the adapter derives:

```text
operation_id = SHA256(
  "mkt-pfi-v1" || 0x00 ||
  order_event_id || 0x00 ||
  operation_kind || 0x00 ||
  leg_index
)
```

`order_event_id` is 64 lowercase hex, `operation_kind` is one of
`credential_verify`, `instruction_issue`, `fiat_transfer`, `crypto_fund`,
`crypto_release`, `escrow_release`, `refund`, `chargeback`, `dispute_open`,
`guarantee_claim`, or `guarantee_payout`, and `leg_index` is canonical decimal.
All string fields are UTF-8 and the separators are literal NUL bytes.

The provider or requester persists the Order ID, operation ID, exact request
digest, and prior result before retrying. Identical replay returns the prior
result. Changed bytes under one operation ID fail with
`pfi_external_effect_conflict`. Multiple gift wraps, relay replay, process
restart, or a repeated Status cannot repeat a credential check, transfer,
release, refund, dispute, chargeback, or guarantee claim.

When an external rail has no idempotency key, the adapter durably records
intent before dispatch and reconciles by the institution's immutable operation
reference before any retry. An unknown result is `pfi_external_effect_unknown`;
blind retry is forbidden.

## Privacy classification

| Data                                                                                                                                                                   | Placement                                                   |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Provider key, public entity ref, asset IDs, side limits, public rails, fee cap, supported risk classes                                                                 | public Provider Profile, Qualification Policy, or Offering  |
| Credential schema, accepted issuer IDs, required claim-type names, retention policy                                                                                    | public Qualification Policy                                 |
| RFQ amount, exact Quote, route, fees, deadlines, custody, recourse, credential commitment, Status, Cancel, Close                                                       | signed private NIP-MKT record inside NIP-59                 |
| Credential or presentation bytes, subject identity, bank/account data, payment instructions, settlement endpoint, dispute narrative, evidence bytes, access capability | direct protected channel or authority-controlled store only |
| Evidence digest, non-bearer opaque operation ref, authority and verifier IDs                                                                                           | private NIP-MKT record                                      |
| Redacted outcome permitted by every affected party                                                                                                                     | optional base Public Market Receipt                         |

Public objects use closed profile schemas with no free-form customer fields.
Private records still leak kind, approximate timing, relay choice, message
size, and network metadata to observers. Implementations use separate wraps,
timestamp randomization, padding, small inbox sets, short retention, and Tor
when the threat model requires them.

A Public Market Receipt MUST NOT include the session ID, counterparty,
market, direction, amount, fee, jurisdiction, rail, qualification burden,
credential fact, dispute details, or operation reference. It may state only a
profile/version, permitted base outcome, Close event ID, author role, and
consented public-safe evidence reference.

## Recovery and loss accounting

Before funds move, each participant persists:

- exact signed RFQ, Quote, Order, and all available Status and Cancel records;
- the pinned Qualification Policy event, descriptor, and digests;
- local credential-verification result and presentation digest, never
  unnecessary presentation bytes;
- operation IDs and external institution references;
- direct authenticated counterparty-channel information;
- every deadline, custody phase, evidence requirement, and loss-allocation
  term; and
- a route-specific recovery package.

The recovery package includes exact steps and authority refs for completion,
refund, escrow release, chargeback, dispute, guarantee claim, and payout. If a
rail supports a pre-signed or pre-derived unilateral exit, the participant
creates and verifies it before funding. If it does not, the Quote says
`unilateral_exit="none"` and names the direct counterparty, escrow, guarantee,
or recourse path that provides termination.

With every market relay, relay handler, provider HTTP API, and coordinator
offline, both parties MUST be able to use their persisted signed records, a
direct or relay-agnostic authenticated channel, and the external rails to reach
`completed`, `refunded`, or a verified unilateral exit. A unilateral exit is a
recovery disposition on a base Close outcome, not a new base outcome. The path
does not require a fresh market signature from an unavailable coordinator. A profile
implementation that cannot execute its declared path is not executable.

Credential verification completes before funding so issuer or verifier outage
cannot trap funds in a qualification step. Escrow and guarantee claim paths
address their named authorities directly. A best-effort route relies on the
counterparty direct channel and disclosed external recourse; its absence is
visible before Order and cannot be relabeled as recovery.

A Close contains `losses`, even when empty. Each loss entry contains leg ID,
asset ID, atomic amount, controlling principal, loss state, evidence state,
recovery action, responsible authority, and terminal disposition. It contains
no PII. Required loss states include:

- `credential_unavailable`, `credential_rejected`, and
  `credential_revoked_before_funding`;
- `instructions_unavailable`;
- `first_leg_unknown`, `first_leg_reversed`, and `first_leg_final`;
- `second_leg_missing`, `second_leg_reorged`, and `second_leg_replaced`;
- `chargeback_after_delivery`;
- `refund_pending`, `refund_failed`, and `refund_unknown`;
- `guarantee_revoked`, `guarantee_claim_denied`, and
  `guarantee_payout_missing`;
- `dispute_unavailable` and `dispute_unresolved`;
- `evidence_unavailable`, `sequence_gap`, and `status_fork`;
- `counterparty_unreachable`, `coordinator_unavailable`, and
  `relay_unavailable`; and
- `local_record_loss` and `external_effect_unknown`.

Missing evidence remains missing. A provider database, current UI, relay
timestamp, or later statement cannot fill a historical gap.

## Errors and unsupported behavior

Implementations expose these stable machine codes in addition to NIP-MKT base
errors:

| Code                                   | Meaning                                                                |
| -------------------------------------- | ---------------------------------------------------------------------- |
| `pfi_unsupported_version`              | profile or policy version is unsupported                               |
| `pfi_unsupported_critical_member`      | a declared critical member is unknown or unsupported                   |
| `pfi_invalid_asset_id`                 | asset identifier is noncanonical, unknown, or a ticker                 |
| `pfi_market_id_mismatch`               | market digest does not match the asset-ID pair                         |
| `pfi_noncanonical_amount`              | amount is not a canonical atomic-unit string                           |
| `pfi_side_disabled`                    | requested side has `max="0"`                                           |
| `pfi_amount_out_of_range`              | amount violates enabled limits                                         |
| `pfi_invalid_fee_promise`              | exact fee exceeds or obscures the promised cap                         |
| `pfi_price_feed_unavailable`           | exact pinned feed cannot be obtained                                   |
| `pfi_price_feed_stale`                 | feed observation exceeds the quoted age                                |
| `pfi_price_feed_mismatch`              | URL, pointer, payload, value, scale, or commitment differs             |
| `pfi_policy_missing`                   | Offering or Quote lacks an exact Qualification Policy pin              |
| `pfi_policy_unknown_member`            | public policy contains an unrecognized member                          |
| `pfi_policy_expired`                   | pinned policy was not active within the Quote rule                     |
| `pfi_public_pii_forbidden`             | public profile object contains a PII-shaped field or value             |
| `pfi_bearer_reference_forbidden`       | a market record contains a credential or evidence capability           |
| `pfi_credential_presentation_required` | qualification commitment or direct presentation is missing             |
| `pfi_credential_expired`               | presentation or credential is outside its allowed time                 |
| `pfi_credential_binding_mismatch`      | audience, purpose, challenge, policy, or digest differs                |
| `pfi_credential_issuer_unaccepted`     | issuer is not admitted by the pinned policy                            |
| `pfi_credential_verification_failed`   | issuer or holder proof failed                                          |
| `pfi_risk_classification_missing`      | Quote omits route or leg classification                                |
| `pfi_risk_overclaim`                   | claimed class is stronger than its evidence and weakest leg            |
| `pfi_guarantee_unavailable`            | guarantee or reserve evidence is absent, stale, invalid, or encumbered |
| `pfi_guarantee_binding_mismatch`       | coverage does not bind the Order, asset, amount, failure, or deadline  |
| `pfi_settlement_evidence_invalid`      | evidence shape, authority, digest, or verifier fails                   |
| `pfi_settlement_overclaim`             | a Status or Close claims a higher evidence rung than verified          |
| `pfi_unauthorized_transition`          | signer or transition is not allowed in the current phase               |
| `pfi_timer_expired`                    | an action occurs after its quoted deadline                             |
| `pfi_cancel_after_irreversible`        | cancellation cannot stop the external step                             |
| `pfi_external_effect_conflict`         | one operation ID is reused with different request bytes                |
| `pfi_external_effect_unknown`          | external rail result cannot be reconciled safely                       |
| `pfi_recovery_package_missing`         | required recovery material was not persisted before funding            |

Unknown profile versions, unknown critical members, unknown risk classes,
unknown evidence classes, and unknown transition phases fail closed. They do
not degrade to `best-effort`.

## Fixture manifest

The version 1 descriptor pins a corpus containing every named fixture below.
Each fixture contains exact signed event bytes, expected admission result,
expected profile error, local clock, external-adapter observations, and the
expected evidence rung. Fixture keys and credential subjects are deterministic
test identities with no real person or account data. The manifest includes a
Status sequence-gap case in addition to the required positive, negative,
replay, fork, expiry, privacy, and recovery classes.

| Fixture                                             | Class    | Required assertion                                                                                         |
| --------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------- |
| `pfi-v1/positive-qualification-policy.json`         | positive | provider-signed closed `39630` policy is admitted                                                          |
| `pfi-v1/positive-on-ramp-guaranteed.json`           | positive | quote-first qualification, Order, verified fiat leg, crypto delivery, guarantee fields, and Close validate |
| `pfi-v1/positive-off-ramp-escrowed.json`            | positive | escrowed off-ramp reaches paid-reversible Close with residual exposure shown                               |
| `pfi-v1/positive-best-effort-disclosed.json`        | positive | weakest class and absent protection remain explicit                                                        |
| `pfi-v1/positive-price-feed-pinned.json`            | positive | exact URL, RFC 6901 pointer, payload digest, value, amounts, and fee promise agree                         |
| `pfi-v1/positive-credentials-later.json`            | positive | Quote precedes direct encrypted presentation; relay sees only the commitment                               |
| `pfi-v1/negative-ticker-asset-id.json`              | negative | ticker matching fails with `pfi_invalid_asset_id`                                                          |
| `pfi-v1/negative-json-number-amount.json`           | negative | numeric amount fails with `pfi_noncanonical_amount`                                                        |
| `pfi-v1/negative-disabled-side.json`                | negative | RFQ against `max="0"` fails with `pfi_side_disabled`                                                       |
| `pfi-v1/negative-fee-promise-exceeded.json`         | negative | hidden or excessive fee fails with `pfi_invalid_fee_promise`                                               |
| `pfi-v1/negative-substitute-price-feed.json`        | negative | equivalent value from another URL fails with `pfi_price_feed_mismatch`                                     |
| `pfi-v1/negative-stale-price-feed.json`             | negative | stale exact feed fails with `pfi_price_feed_stale`                                                         |
| `pfi-v1/negative-policy-head-replaced.json`         | negative | Quote cannot substitute a later policy head for its pinned event                                           |
| `pfi-v1/negative-unsupported-critical-member.json`  | negative | unknown declared critical member fails with `pfi_unsupported_critical_member`                              |
| `pfi-v1/negative-credential-before-quote.json`      | negative | RFQ carrying presentation material is rejected                                                             |
| `pfi-v1/negative-credential-audience.json`          | negative | wrong provider audience fails with `pfi_credential_binding_mismatch`                                       |
| `pfi-v1/negative-credential-challenge-replay.json`  | negative | presentation from another Quote fails with `pfi_credential_binding_mismatch`                               |
| `pfi-v1/negative-guarantee-no-reserve.json`         | negative | unsupported guaranteed class fails with `pfi_guarantee_unavailable`                                        |
| `pfi-v1/negative-atomic-reversible-fiat.json`       | negative | atomic overclaim fails with `pfi_risk_overclaim`                                                           |
| `pfi-v1/negative-settlement-overclaim.json`         | negative | bank acceptance cannot become settled finality                                                             |
| `pfi-v1/negative-unauthorized-provider-status.json` | negative | wrong signer fails with `pfi_unauthorized_transition`                                                      |
| `pfi-v1/privacy-public-policy-pii.json`             | privacy  | public name/account/credential fields fail with `pfi_public_pii_forbidden`                                 |
| `pfi-v1/privacy-public-bearer-url.json`             | privacy  | bearer retrieval URL fails with `pfi_bearer_reference_forbidden`                                           |
| `pfi-v1/privacy-private-commitment-only.json`       | privacy  | relay-observable records contain digest and opaque ref, no presentation bytes                              |
| `pfi-v1/privacy-public-receipt-redaction.json`      | privacy  | receipt omits market, party, amount, rail, jurisdiction, and credential facts                              |
| `pfi-v1/replay-rewrapped-order.json`                | replay   | another wrap of identical Order returns the prior result and causes no second effect                       |
| `pfi-v1/replay-external-operation-result.json`      | replay   | same operation request returns its persisted prior result                                                  |
| `pfi-v1/replay-external-operation-conflict.json`    | replay   | changed request under one operation ID fails with `pfi_external_effect_conflict`                           |
| `pfi-v1/gap-provider-status-sequence.json`          | gap      | out-of-sequence Status is retained as a gap and cannot advance state                                       |
| `pfi-v1/fork-provider-status-sequence.json`         | fork     | both same-sequence provider Status events are retained and surfaced                                        |
| `pfi-v1/fork-reservation-equivocation.json`         | fork     | overlapping firm reservations are attributable and neither is silently selected                            |
| `pfi-v1/expiry-quote.json`                          | expiry   | late Order fails with `pfi_timer_expired` and moves no funds                                               |
| `pfi-v1/expiry-credential.json`                     | expiry   | expired presentation fails before funding                                                                  |
| `pfi-v1/expiry-guarantee.json`                      | expiry   | expired guarantee blocks a guaranteed route before funding                                                 |
| `pfi-v1/expiry-dispute-window.json`                 | expiry   | late dispute remains recorded but is not reported accepted                                                 |
| `pfi-v1/recovery-relay-and-api-down-complete.json`  | recovery | persisted records, direct channel, and external rails complete without relay/handler/provider API          |
| `pfi-v1/recovery-escrow-refund.json`                | recovery | direct escrow refund reaches verified `refunded` without coordinator                                       |
| `pfi-v1/recovery-guarantee-claim.json`              | recovery | guarantee claim and payout use pinned authorities with the coordinator absent                              |
| `pfi-v1/recovery-external-result-unknown.json`      | recovery | restart reconciles institution ref and never retries blindly                                               |
| `pfi-v1/recovery-local-record-loss.json`            | recovery | missing signed history remains an explicit terminal loss                                                   |
| `pfi-v1/recovery-late-chargeback.json`              | recovery | corrective Status and Close preserve the earlier paid-reversible claim and record new loss                 |

Implementations MAY add fixtures. They MUST NOT omit or rename a manifest
fixture while advertising version 1 under the pinned corpus digest.

## Conformance and advertisement

A relay implements the observable subset only:

- `kind:39630` tag, content, bound, replacement, and no-PII shape validation;
- MKT-PFI Offering tags and closed public content;
- private record profile/version, reference grammar, bounds, authorized
  recipient transport, immutable admission, replay, and fork surfacing; and
- bounded shapes for risk, dispute, recourse, evidence, and credential
  commitments after decryption by an authorized endpoint.

A relay does not claim credential, rail, guarantee, reserve, dispute, or
settlement verification unless a separately configured adapter names the
external authority and produces its exact evidence. Even then, the authority
remains external.

NIP-11 may advertise `nip-mkt-pfi` version `1` only when the active
configuration has passed the complete local relay-observable corpus and the
advertised verifier set is configured. Client conformance additionally
requires every fixture, transition, external-effect idempotency case, privacy
case, and recovery drill. Transport-only NIP-MKT support MUST NOT advertise an
executable PFI profile.

## Profile-contract crosswalk

1. Profile ID, version, authority key, specification digest, and fixtures are
   fixed in Profile identity and Fixture manifest.
2. Roles, eligibility, credentials, signers, verifiers, and authorities are
   fixed in Terms, Qualification Policy, Credential presentation commitments,
   and Signers, verifiers, and authorities.
3. Assets, networks, directions, units, precision, limits, and fees are fixed
   in Common encoding and Offering fields.
4. Quote fields, Order selection, capacity, and reservation accounting are
   fixed in Quote, Risk classification, and Guarantee and reserve rules.
5. Principal control, unilateral paths, timelocks, and maximum custody are
   fixed in Custody dimensions and Timers.
6. Transitions, cancellation, outcomes, disputes, refunds, chargebacks, and
   recovery are fixed in State machine, Cancellation, and Recovery and loss
   accounting.
7. Public/private classification and metadata leaks are fixed in Privacy
   classification.
8. Rail adapters and exact funding, execution, finality, reorg, replacement,
   chargeback, and settlement verifiers are fixed in Settlement evidence and
   the signer map.
9. Order-to-external-effect idempotency is fixed in External-effect
   idempotency.
10. Errors, unsupported versions, and loss states are fixed in Errors and
    unsupported behavior and Recovery and loss accounting.
11. Positive, negative, replay, sequence-gap, fork, expiry, privacy, and
    recovery fixtures are fixed by Fixture manifest.

The coordinator-independent recovery floor is normative in Recovery and loss
accounting and has three named recovery drills in addition to fault-specific
cases.

## Security considerations

- Quote-first disclosure reduces credential harvesting but does not remove
  provider-side correlation. Holders use purpose-bound presentations and the
  narrowest claim set.
- Pairwise encryption hides content from relays, not IP, timing, size, or
  recipient metadata. High-risk routes use separate keys, padding, short
  retention, and an anonymity network where appropriate.
- A provider signature makes a promise attributable. It does not prove
  liquidity, qualification, reserve, delivery, refund, guarantee payout, or
  settlement.
- Reversible fiat paired with final crypto creates asymmetric loss. The Quote
  exposes the window, protection, controlling authority, and recovery path.
- Feed pinning prevents silent oracle substitution, but the pinned feed can be
  wrong or unavailable. Its authority stops at the price input.
- Credential, settlement, reserve, and dispute evidence can contain sensitive
  data. Only digests and non-bearer refs enter market records.
- A relay or provider outage cannot be the only recovery path. Participants
  verify and retain the recovery package before funding.
- No relay or generic market binary stores seeds, preimages, macaroons, claim
  keys, refund keys, bank credentials, credential bytes, bearer references, or
  settlement signing material.

## References

- [`MKT.md`](MKT.md)
- `docs/teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md`
- `docs/teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md`
- NIP-01, NIP-17, NIP-32, NIP-39, NIP-40, NIP-44, NIP-59, NIP-73,
  NIP-85, NIP-EV
- CAIP-19, ISO 3166-1, ISO 4217, RFC 6901, W3C Verifiable Credentials

## Changelog

**v1 draft (2026-08-04)**

- Allocated `kind:39630` for a public provider-signed Qualification Policy;
  retained `39631-39639` as unallocated.
- Defined quote-first credential commitments, asset-ID pair identity,
  atomic-unit strings, disabled-side semantics, fee promises, and exact
  price-feed pinning.
- Defined route-risk, guarantee/reserve, custody, recourse, settlement,
  chargeback, idempotency, privacy, recovery, error, and fixture contracts.
