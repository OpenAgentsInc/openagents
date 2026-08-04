# NIP-MKT-MINT

## Cashu mint and Fedimint gateway negotiation

`draft` `optional`

MKT-MINT version 1 is a thin profile of [NIP-MKT](MKT.md) for negotiating
Cashu mint/melt operations and Fedimint gateway routes. Official NIP-87 owns
mint and federation discovery. Cashu NUTs and the selected Fedimint protocol
own issuance, proof, redemption, consensus, and gateway semantics. This
profile owns comparable negotiated terms, custody disclosure, correlation,
and evidence references.

It does not define an ecash proof, mint quote, federation contract, gateway
API, or universal trust score.

## Profile identity and allocation

- Profile ID: `mkt-mint`
- Version: `1`
- Parent: NIP-MKT v0.1
- Authority: the author pubkey of the selected `kind:39602` Profile
  Descriptor. Clients MUST allowlist the exact key and pin the descriptor
  event ID and specification digest.
- Specification digest: the descriptor's `x` tag over the exact bytes of this
  file. This draft designates no universal profile authority.

| Kind        | Name                | Publication                                       | Replacement law                   |
| ----------- | ------------------- | ------------------------------------------------- | --------------------------------- |
| 39640       | Mint Route Contract | private signed record in NIP-59 `kind:1059` wraps | unique `d`; immutable-by-contract |
| 39641-39649 | Reserved            | —                                                 | unallocated                       |

`kind:39640` is the minimum profile artifact not safely represented by the
base kinds. A Quote is signed before the requester accepts and before a native
mint or gateway quote may exist. An Order accepts the Quote but cannot bind
later native quote identifiers, verifier inputs, external-effect IDs, and both
participants' recovery commitments. The Mint Route Contract supplies that
post-Order, pre-effect commitment. Native rail receipts remain evidence
references and are not copied into a generic Nostr evidence kind.

The kind extends NIP-MKT's supported private-kind set and uses the same signed
inner event, rumor, seal, per-recipient wrap, signer-equality, recipient gate,
32 KiB bound, duplicate-JSON rejection, replay, and idempotency rules. Clients
fail closed on unknown critical members.

### Collision review

The allocation was checked on 2026-08-04 against these exact revisions:

| Source                                           | Revision                                   | Result                                                                    |
| ------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------------- |
| Official NIPs                                    | `c53877571f96eb423661fc23c620d629d37b8f19` | No assignment in `39640-39649`                                            |
| Block Buzz NIPs                                  | `540b58920cef205b838da8be8442aae62bceaaa5` | No assignment in `39640-39649`                                            |
| OpenAgents NIPs                                  | `2d9b1463be6fb1ceac60c4bfabcb7b10f168d060` | SWP/PFI allocations only; no assignment in `39640-39649`                  |
| `nostr-protocol/registry-of-kinds` `schema.yaml` | `2483e752146d171524dcb10dffd06de2aa271bf3` | No assignment in `39640-39649`; nearest higher registered kind is `39701` |

Implementers MUST repeat all four checks before adoption or extension.

## Roles, eligibility, and signer map

| Role                 | Required authority                                                                                                           |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| requester            | Wallet or gateway client; signs RFQ, Order, one Route Contract, requester Status, Cancel, and Close                          |
| provider             | Mint operator, federation gateway, or routing provider; signs Offering, Quote, one Route Contract, Status, Cancel, and Close |
| Cashu mint           | Own NUT endpoints, keysets, and native rail responses; has no market authority unless it is also the provider                |
| federation           | Own consensus and module state; native threshold or projection receipts retain their own authority                           |
| gateway              | Own Lightning or chain interaction for the named federation route; native receipts prove only gateway-scoped facts           |
| independent verifier | Verifies a named public rail fact and issues the exact receipt type pinned in the Quote                                      |
| relay                | Validates public shapes and transports private records; has no proof, wallet, mint, federation, or settlement authority      |

No global eligibility or credential scheme exists. An Offering states
`credential_burden` as `none`, `access-token`, `membership-proof`, or
`external-policy`. Any access token, membership presentation, invitation code,
or account data is private, audience-bound, purpose-bound, and referenced by
digest. It never appears in public content.

## Discovery authority and market identity

Every executable RFQ or Quote references exactly one NIP-87 announcement:

- Cashu: `kind:38172` by exact `a` address and event ID; or
- Fedimint: `kind:38173` by exact `a` address and event ID.

NIP-87 `kind:38000` recommendations are user claims and cannot substitute for
the mint or federation announcement. MKT-MINT MUST NOT copy a mint URL,
federation invite code, NUT list, module list, or operator claim into a new
discovery authority and call it canonical.

The market identity is the exact `base_asset_id` and `quote_asset_id` pair.
For ecash, the asset identifier binds the rail family, mint or federation
identity, unit, and keyset or configuration family. Display units such as
`sat`, `USD`, or `EUR` are labels and are insufficient identifiers.

All amounts are canonical decimal strings matching
`^(0|[1-9][0-9]*)$`, denominated in the declared atomic unit, and bounded to
an unsigned 64-bit integer in version 1. JSON numbers are invalid.

A public Offering declares both directions:

```json
{
  "market": {
    "base_asset_id": "<ecash-asset-id>",
    "quote_asset_id": "<payment-asset-id>"
  },
  "sides": {
    "mint": { "min": "1000", "max": "1000000" },
    "melt": { "min": "0", "max": "0" }
  }
}
```

`max: "0"` disables a side and requires `min: "0"`. Omission is invalid.
An enabled side requires `0 < min <= max`.

## Offering and Quote fields

An MKT-MINT Offering adds:

| Field                | Rule                                                                    |
| -------------------- | ----------------------------------------------------------------------- |
| `nip87_ref`          | Exact announcement address, event ID, kind, and relay hints             |
| `rail`               | `cashu` or `fedimint`                                                   |
| `market`             | Exact asset-ID pair and atomic units                                    |
| `sides`              | Cashu mint/melt bounds; Fedimint deposit disabled and withdrawal bounds |
| `operations`         | Cashu `mint`, `melt`; Fedimint `withdraw-lightning`, `withdraw-onchain` |
| `protocol_revisions` | Exact supported NUT set or Fedimint module/API revision                 |
| `custody_class`      | Cashu `a3-mint`; Fedimint `a2-federation`                               |
| `credential_burden`  | Public category only; no presentation or bearer value                   |
| `gateway_policy`     | `fixed`, `requester-selectable`, or `federation-selected`               |

The Quote pins:

- the NIP-87 event ID and digest and the exact native request template;
- rail, operation, protocol revisions, network, and exact asset IDs;
- input, output, fee, reserve, and rounding amounts as decimal strings;
- `fee_bps` from `"0"` through `"10000"` as a canonical decimal string, fee
  asset, minimum fee, maximum fee, and rounding rule;
- exact mint keyset ID and public-key digest, or federation configuration,
  module, threshold, guardian-set digest, and gateway identity;
- bounds for the native quote response and expiry;
- payment method, invoice or address reference class, and confirmation policy;
- custody dimensions, withdrawal path, and maximum expected custody duration;
- accepted evidence signers and finality policy; and
- coordinator-independent recovery package requirements.

Fedimint `deposit` MUST be present with `min: "0"` and `max: "0"` in a
version-1 capability. Deposit negotiation is unsupported until a revision
defines its lifecycle, evidence, refund rules, recovery, and fixtures.

The pre-Order Quote MUST NOT claim a native quote ID or response that does not
yet exist. It commits to the canonical request template, response schema,
acceptable bounds, and digest rules. The post-Order Route Contract binds the
actual native request, response, quote ID, and expiry. An invoice, federation
invitation, gateway token, or access credential stays in a private record or
an authorized external channel. A public Offering includes only digests and
public NIP-87 references.

`fee_bps` is a fill promise. The wallet verifies the external quote and
output. A provider signature proves neither reserves nor redemption.

When a price feed converts units, the Quote pins exact `feed_url`, RFC 6901
`value_pointer`, optional `timestamp_pointer`, `max_age_seconds`, response
SHA-256, canonical `observed_value`, `observed_at`, and rounding. Both parties
reproduce those inputs before Order. A substitute feed is invalid and the feed
never becomes settlement authority.

### Allowed Order selection

An Order may select only fields the Quote marks selectable:

- one amount within the Quote's inclusive bounds;
- one payment method;
- one gateway from the exact ordered gateway set; and
- one credential-presentation reference from the accepted schema set.

It cannot change mint, federation, keyset, configuration digest, operation,
asset, fee, expiry, custody class, recovery path, or finality policy.

The Quote commits to an ordered gateway set and one deterministic selection
rule. The Order selects one gateway. A switch is allowed only before a native
quote or other external effect and requires a new Quote, Order, and pair of
Route Contracts. After an external effect, gateway disappearance is an
explicit `gateway-unavailable` loss; no alternate inherits the selected
gateway's authority or operation ID.

### Capacity and reservation

An indicative Quote uses `reservation: none`. A soft reservation is a
provider-signed allocation of route capacity with amount, expiry, and
uniqueness key. A hard reservation requires an external rail artifact that
the selected verifier proves exclusively covers the quoted amount, such as a
funded gateway contract or federation contract commitment.

A Cashu mint quote, key-control proof, balance claim, or NIP-87 announcement
does not by itself prove unencumbered reserves and cannot support `hard`.
Order admission consumes the exact reservation idempotently. Expiry releases
only the provider allocation; it does not cancel a NUT quote, reverse a
federation contract, refund payment, or restore spent proofs.

## Custody disclosure

Every Quote contains all six NIP-MKT custody dimensions:

| Dimension              | Cashu requirement                                              | Fedimint requirement                                                                |
| ---------------------- | -------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `funds_control`        | Mint controls redemption; wallet controls bearer proofs        | Guardian threshold controls federation state; wallet controls notes and client keys |
| `execution_control`    | Mint endpoint and wallet for mint/melt                         | Federation module plus selected gateway for external payments                       |
| `settlement_authority` | Cashu mint for ecash state; Lightning or chain for payment leg | Federation consensus for internal state; Lightning or chain for withdrawal leg      |
| `reversibility`        | Per quote and payment rail                                     | Per module, gateway, and external rail                                              |
| `recourse`             | Operator policy, reserve/attestation, or none                  | Governance/guardian and gateway policy, or none                                     |
| `credential_exposure`  | Exact audience, purpose, fields, and retention                 | Exact audience, purpose, fields, and retention                                      |

Custody class is descriptive, not a ranking. An A2 or A3 label does not prove
solvency, availability, correct issuance, or redemption.

## Mint Route Contract (`kind:39640`)

After the native mint, federation, or gateway quote exists, the requester and
provider each sign a separate immutable Mint Route Contract with one shared
`contract_sha256`. Both records are wrapped to both participants. A recovery
verifier receives the exact signed contracts through authorized disclosure
over an authenticated channel; it is not an unlisted gift-wrap recipient.
Bare publication is invalid. No funds, bearer proofs, or external spend
authorization move before both contracts match.

Required tags are:

- unique 64-lower-hex `d`;
- one `session` tag containing the NIP-MKT session ID;
- `profile`, `mkt-mint`, `1`;
- one `e` tag marked `quote`;
- one `e` tag marked `order`;
- exactly one `p` tag for the other participant, marked `requester` or
  `provider`;
- one `rail` tag set to `cashu` or `fedimint`;
- one `role` tag set to `requester` or `provider`;
- one `x` tag containing the shared contract SHA-256;
- fixed `alt` text `MKT-MINT route contract`.

A contract for an indicative Quote also requires one `e` tag marked `status`
for the provider's accepted Status. The contract has no NIP-40 expiration
because both parties need it after the native quote expires for recovery and
loss accounting.

Content uses the NIP-MKT envelope and adds:

```json
{
  "contract": {
    "quote_event_id": "<64-lower-hex>",
    "order_event_id": "<64-lower-hex>",
    "accepted_status_event_id": null,
    "operation": "mint",
    "native_request_sha256": "<64-lower-hex>",
    "native_quote_id_sha256": "<64-lower-hex>",
    "native_quote_sha256": "<64-lower-hex>",
    "terms_sha256": "<64-lower-hex>",
    "custody_sha256": "<64-lower-hex>",
    "verifier_policy_sha256": "<64-lower-hex>",
    "external_effect_ids_sha256": "<64-lower-hex>",
    "recovery_package_sha256": "<64-lower-hex>"
  },
  "contract_sha256": "<64-lower-hex>",
  "signer_role": "provider"
}
```

`contract_sha256` is SHA-256 of the RFC 8785 serialization of the exact
`contract` object, and the `x` tag MUST equal it. `quote_event_id` and
`order_event_id` MUST equal the causal tags. `accepted_status_event_id` is the
provider's accepted Status for an indicative Quote and JSON `null` for a firm
Quote. When present, it MUST equal the `status` causal tag; when it is `null`,
that tag is forbidden. The shared object commits to the NIP-87 event, native quote response,
mint keyset or federation configuration, selected gateway, amounts, fees,
expiry, custody disclosure, verifier policy, idempotency map, and recovery
package. The requester record means those inputs passed its local policy. The
provider record means it will execute only those inputs.

Digest preimages are exact: `native_request_sha256` and
`native_quote_sha256` hash the unmodified UTF-8 native request and response
bytes; `native_quote_id_sha256` hashes the UTF-8 quote identifier with no
prefix or terminator; and every remaining `*_sha256` hashes the RFC 8785
serialization of the correspondingly named JSON object in the accepted Quote
or retained recovery package. The Quote defines the object schema and media
type. An absent preimage, a different serialization, or a digest without its
pinned schema is a contract mismatch.

The records contain only hashes and bounded public identifiers. Cashu proofs,
secrets, blinded messages, blinding factors, Fedimint notes, recovery secrets,
spend keys, preimages, macaroons, bearer tokens, and raw federation
invitations are forbidden. A changed contract requires a new Quote and Order;
it cannot supersede matching contracts after an external effect.

The route becomes `route-contract-bound` only when both events are present,
their roles are complementary, authors match the Order and Quote, and the
contract objects and digests are identical. A third party cannot countersign
for a participant. Status and evidence references use the exact contract event
IDs and shared digest.

## Lifecycle and transitions

Cashu mint flow:

```text
accepted -> quote-issued -> route-contract-pending -> route-contract-bound
  -> payment-required -> payment-observed
  -> issuance-pending -> proofs-issued -> wallet-verified -> completed
```

Cashu melt flow:

```text
accepted -> quote-issued -> route-contract-pending -> route-contract-bound
  -> proofs-submitted-to-mint
  -> settlement_pending -> melt-paid -> change-verified -> completed
```

Fedimint flow:

```text
accepted -> route-contract-pending -> route-contract-bound
  -> federation-contract-pending -> federation-contract-accepted
  -> gateway-pending -> external-settlement-pending
  -> withdrawal-verified -> completed
```

The profile extends Status with `quote-issued`, `route-contract-pending`,
`route-contract-bound`, `issuance-pending`, `proofs-issued`,
`wallet-verified`, `payment-required`, `payment-observed`,
`proofs-submitted-to-mint`, `melt-paid`, `change-verified`,
`federation-contract-pending`, `federation-contract-accepted`,
`gateway-pending`, `external-settlement-pending`, and `withdrawal-verified`.

The exact signer and evidence law is:

| State or action                        | Allowed author | Required predecessor and evidence                                           | Timer and timeout result                                                  |
| -------------------------------------- | -------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `accepted`                             | provider       | timely Order when Quote is indicative                                       | Quote expiry rejects late acceptance                                      |
| `quote-issued`                         | provider       | native response matching the request template                               | native expiry permits failure or a new Quote/Order, never silent mutation |
| `route-contract-pending`               | either party   | accepted Order and native response                                          | contract deadline leads to `failed` before funds move                     |
| `route-contract-bound`                 | either party   | both participant contracts and identical digest                             | no timeout promotes it without both signatures                            |
| `payment-required` / `gateway-pending` | provider       | bound contract and exact native payment or gateway instruction              | native expiry follows the pinned refund/loss policy                       |
| `payment-observed`                     | provider       | admitted Lightning or chain receipt                                         | observation is not issuance or settlement                                 |
| `issuance-pending` / `proofs-issued`   | provider       | Cashu mint operation bound to Order; exact native response                  | native timeout leads to retry, refund, or `unresolved`                    |
| `wallet-verified` / `change-verified`  | requester      | wallet verifies signatures, keyset, amounts, DLEQ when required, and change | verification failure leads to `failed` or `unresolved`                    |
| `proofs-submitted-to-mint`             | requester      | exact proofs submitted over the native protected channel                    | mint timeout remains an external custody loss                             |
| `melt-paid`                            | provider       | native melt response plus admitted Lightning payment evidence               | payment ambiguity closes `unresolved`                                     |
| `settlement_pending`                   | provider       | exact native melt or withdrawal operation bound to Order                    | rail timeout leads to `failed`, `refunded`, or `unresolved`               |
| `federation-contract-pending`          | requester      | bound contract and exact Fedimint operation                                 | consensus timeout remains `federation-unavailable`                        |
| `federation-contract-accepted`         | requester      | threshold consensus receipt verified locally                                | consensus rollback removes dependent finality                             |
| `withdrawal-verified`                  | requester      | federation receipt and separate chain or Lightning finality receipt         | rail rollback removes completion                                          |
| `external-settlement-pending`          | provider       | selected gateway operation and external rail reference                      | gateway or rail timeout remains an explicit loss                          |
| `completed` / `refunded` / `failed`    | either party   | exact terminal native and external rail evidence for the claimed state      | evidence rollback removes a verified terminal projection                  |
| Cancel `effective`                     | either party   | matching acceptance before irreversible input, or native refund evidence    | expiry alone does not cancel or refund                                    |
| Close `completed/refunded`             | either party   | wallet-verifiable terminal rail artifacts for that outcome                  | unavailable evidence closes `unresolved`                                  |
| Close `cancelled`                      | either party   | effective Cancel and proof no irreversible native input remains             | a Cancel request alone is insufficient                                    |
| Close `expired`                        | either party   | signed deadline and verified release of reservation or native quote only    | expiry does not imply payment refund                                      |
| Close `failed/unresolved`              | either party   | exact native failure or missing/conflicting evidence inventory and losses   | no inferred issuance, payment, or refund                                  |

A Status outside its row is retained as an unsupported claim and does not
advance state. A provider cannot promote `proofs-issued` to settlement.
`invoice-paid` does not prove proofs were issued. A mint quote does not prove
payment, issuance, redemption, solvency, or finality.

Only the base states used in the table and the listed profile extensions are
admitted. Any other base or profile state is unsupported version 1.

Cancellation is effective only before the selected external rail accepts an
irreversible input. After that point the profile uses the external quote's
refund, retry, or recovery rules. Expiry may expire a Quote or release a local
reservation; it does not erase spent proofs, refund a payment, or reverse
federation consensus.

Close outcomes are `completed`, `cancelled`, `expired`, `failed`, `refunded`,
or `unresolved`. Close lists input, output, fee, change, and refund amounts
separately, each with the strongest verified provenance.

## Rail verification and finality

### Cashu

The wallet verifies the selected NUT revision, mint URL identity, keyset ID,
keyset public keys, unit, quote ID, amount, fee, expiry, and blinded-signature
responses. When the Quote requires DLEQ or another proof extension, omission
fails closed. Proof possession remains wallet-local.

Mint completion requires the wallet to unblind and verify the issued proofs
against the pinned keyset. Melt completion requires the exact melt response,
Lightning payment evidence under the selected policy, and valid change proofs
when change is due. A spent-state response is an observation by the mint and
is evaluated under the pinned NUT policy.

### Fedimint

The wallet verifies the federation ID, configuration digest, guardian set,
threshold, module version, contract or operation ID, and consensus outcome.
Gateway evidence proves only the gateway leg. Federation consensus proves only
the federation leg. External Lightning or Bitcoin finality is verified
separately.

### Chain and Lightning

An on-chain leg pins network, transaction and output constraints,
confirmation depth, reorg handling, and replacement policy. A replacement
must preserve the quoted destination and amount and be admitted by the exact
policy. A reorg rolls dependent finality back until reverified.

A Lightning leg pins invoice signature, network, amount, expiry, payment hash,
and payment-proof policy. No unreleased preimage enters a market record.

Every external quote, mint operation, federation contract, gateway request,
and withdrawal is bound to the exact NIP-MKT Order event ID before its first
effect. Replay returns the previous operation result and cannot create a
second external quote or payment.

Native evidence is referenced by exact artifact digest, issuer, receipt type,
provenance label, observation time, and verifier policy. Allowed labels are
`pledged`, `observed`, `verified`, `paid`, `issued`, `refunded`, and `settled`.
The client verifies the native receipt and never infers a stronger label.

## Coordinator-independent recovery

Before any funds or bearer proofs move, the requester persists a local
recovery package:

- exact RFQ through Order and Status chain;
- NIP-87 event, mint URL or federation configuration, and their digests;
- selected NUT or module revision, keyset/configuration keys, and quote
  reference;
- direct rail endpoint and the selected gateway endpoint;
- pending-operation state needed by the wallet to resume or refund; and
- finality, retry, refund, and loss rules.

Custody-bearing fields in that package remain in the wallet's encrypted local
state. The market relay, Immortal server, provider router, and public contract
manifest receive only hashes and public-safe references.

If the market relay, handler, or routing provider disappears, the wallet uses
the persisted package directly against the Cashu mint, Fedimint guardians, or
selected gateway and reaches completed, refunded, or the rail's unilateral
withdrawal state. A Quote that requires the market coordinator to reconstruct
a quote ID, mint URL, keyset, federation configuration, withdrawal request, or
refund path is invalid.

Failure of the mint or federation itself is an external custody loss, not a
coordinator recovery success. The client records `mint-unavailable`,
`federation-unavailable`, or `gateway-unavailable` without inventing a refund.
A selected gateway cannot be replaced after an external effect; recovery uses
that gateway's native operation ID or records the loss.
A Quote that cannot be resumed through a direct native rail endpoint fails
with `mkt_mint_no_independent_recovery` before funds move.

## Privacy and server-state boundary

Public records may contain NIP-87 references, asset IDs, supported operations,
amount ranges, NUT/module revisions, custody class, and credential-burden
category. They MUST NOT contain raw mint quotes, invoices, federation invite
codes, proof material, notes, blinded messages, account identifiers, access
tokens, membership presentations, withdrawal addresses, or recovery packages.

Private market records still MUST NOT carry bearer ecash proofs, Fedimint
notes, wallet seeds, spend keys, blinding factors, preimages, macaroons, or
claim/refund private keys. Those values travel only between the wallet and the
external rail through its native protected channel.

## Errors and loss states

| Code                                 | Meaning                                                             |
| ------------------------------------ | ------------------------------------------------------------------- |
| `mkt_mint_unsupported_version`       | Profile revision is not supported                                   |
| `mkt_mint_invalid_nip87_reference`   | Announcement address, kind, event, or digest failed                 |
| `mkt_mint_discovery_duplication`     | Record attempts to replace NIP-87 authority                         |
| `mkt_mint_invalid_market`            | Asset, unit, direction, or amount law failed                        |
| `mkt_mint_side_disabled`             | Selected side has `max: "0"`                                        |
| `mkt_mint_protocol_mismatch`         | NUT, module, keyset, or configuration differs                       |
| `mkt_mint_quote_mismatch`            | External quote fields differ from the signed Quote                  |
| `mkt_mint_price_source_mismatch`     | Feed URL, pointer, digest, or age differs                           |
| `mkt_mint_route_contract_mismatch`   | Participant contracts differ or omit a committed field              |
| `mkt_mint_invalid_contract_signer`   | Contract signer is not the requester or provider                    |
| `mkt_mint_bearer_material_forbidden` | Market or server record contains custody material                   |
| `mkt_mint_invalid_transition`        | Signer or lifecycle transition is not admitted                      |
| `mkt_mint_idempotency_conflict`      | One logical ID was reused with different bytes                      |
| `mkt_mint_evidence_mismatch`         | Referenced artifact does not prove the claimed fact                 |
| `mkt_mint_no_independent_recovery`   | Quote cannot be resumed directly against the rail                   |
| `mkt_mint_settlement_overclaim`      | Quote, payment, issuance, or gateway fact was promoted beyond proof |

Close preserves `missing_record`, `sequence_gap`, `signer_fork`,
`quote-expired`, `invoice-paid-no-proofs`, `proofs-rejected`,
`melt-unresolved`, `mint-unavailable`, `federation-unavailable`,
`gateway-unavailable`, `chain-reorg`, `transaction-replaced`,
`refund-unavailable`, and `evidence-unavailable`.

## Fixture manifest

An implementation MUST replay these exact case IDs:

| Case ID                                           | Expected result                                                   |
| ------------------------------------------------- | ----------------------------------------------------------------- |
| `mint-positive-cashu-mint`                        | Wallet verifies issued proofs against the pinned keyset           |
| `mint-positive-cashu-melt`                        | Melt payment and change proofs verify independently               |
| `mint-positive-fedimint-withdraw`                 | Federation and gateway evidence remain separate                   |
| `mint-positive-nip87-cashu-ref`                   | Exact kind-38172 event remains discovery authority                |
| `mint-positive-nip87-fedimint-ref`                | Exact kind-38173 event remains discovery authority                |
| `mint-positive-gateway-selection`                 | Order selects one gateway from the signed set                     |
| `mint-positive-route-contract`                    | Requester and provider bind one native route before effects       |
| `mint-negative-disabled-side`                     | Zero-max side is rejected                                         |
| `mint-negative-json-number-amount`                | Numeric amount is rejected                                        |
| `mint-negative-nip87-recommendation-as-authority` | Kind-38000 claim cannot replace announcement                      |
| `mint-negative-keyset-substitution`               | Unquoted Cashu keyset is rejected                                 |
| `mint-negative-federation-config-substitution`    | Changed configuration digest is rejected                          |
| `mint-negative-substitute-feed`                   | Alternate URL or JSON pointer is rejected                         |
| `mint-negative-quote-is-settlement`               | External quote cannot close completed                             |
| `mint-negative-invoice-paid-is-issued`            | Payment cannot prove proof issuance                               |
| `mint-negative-gateway-is-federation`             | Gateway claim cannot prove federation consensus                   |
| `mint-negative-route-contract-mismatch`           | Differing native quote or recovery digest is rejected             |
| `mint-negative-contract-signer`                   | Third party cannot sign a participant contract                    |
| `mint-negative-public-invite`                     | Federation invitation in public content is rejected               |
| `mint-negative-bearer-proof`                      | Cashu or Fedimint bearer material is rejected everywhere          |
| `mint-replay-same-contract`                       | Byte-identical replay returns prior admission result              |
| `mint-replay-contract-conflict`                   | Changed bytes under one `d` fail closed                           |
| `mint-fork-status`                                | Same-signer sequence fork remains visible                         |
| `mint-expiry-quote`                               | Expiry does not refund or settle an external rail                 |
| `mint-privacy-independent-wraps`                  | Each signed contract is wrapped independently to required parties |
| `mint-recovery-market-coordinator-gone`           | Wallet resumes directly against the external rail                 |
| `mint-recovery-gateway-gone`                      | Post-effect gateway loss remains explicit and is not reassigned   |
| `mint-loss-mint-unavailable`                      | Custody loss remains unresolved without false refund              |
| `mint-loss-chain-reorg`                           | Dependent finality rolls back and re-verifies                     |

The table is the normative version-1 case manifest. Adoption MUST publish one
canonical byte corpus containing every case, expected error, authority test
key, descriptor event ID, and corpus SHA-256 in the implementation contract
manifest. Implementations reference that corpus rather than generating local
variants and MUST NOT advertise `mkt-mint/1` before it passes under the active
configuration.

## References

- [NIP-MKT](MKT.md)
- Official NIP-87 and NIP-59
- [Market rails teardown](../teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)
- [tbDEX liquidity protocol teardown](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md)
