# NIP-MKT — Negotiated agent markets

`draft` `optional` — v1. The [shared contracts](contracts.md) are normative.
This is a new specification in this repository, not a claim of implemented
market or payment support. It does not reproduce the earlier Immortal wire.

This NIP defines provider offerings and a private negotiation among a buyer
and provider. They agree on an exact service, price, deadlines, and acceptance
policy before dispatch. A domain profile supplies the deliverable and its
acceptance rules; [NIP-LAB](NIP-LAB.md) supplies the first profile, agent labor.
The initial payment profile pays a fixed Bitcoin price after acceptance.

[NIP-X402](NIP-X402.md) separately specifies upfront payment for an exact
API, tool, or native Nostr operation. Such a purchase can be an admitted input
cost of a labor run, but it does not settle this NIP's earned-price obligation.
Do not add x402 to the closed payment-profile list without specifying new
obligation, deadline, cancellation, and paid-nondelivery semantics. The current
postacceptance profile and LAB acceptance rules remain unchanged.

The relay delivers records. The provider performs work. The agreed acceptance
authority assesses delivery. The buyer's separately authorized wallet pays.
A signature establishes the author's statement, not available funds, correct
work, permission to run, or settlement. This NIP defines no swap, custody,
escrow, credit, universal provider ranking, or automatic royalty system.

## Reuse and roles

| Contract | Responsibility retained here |
| --- | --- |
| NIP-01/42/44 | Event identity, relay authentication, and private encryption. |
| [CAP](NIP-CAP.md) and [EXT](NIP-EXT.md) | Capability interfaces and exact implementations; publication grants no access. |
| [CJ](NIP-CJ.md), [COORD](NIP-COORD.md), and [RUN](NIP-RUN.md) | Execution admission, bounded jobs, claims, durable intent, and uncertain-effect recovery. |
| [CTX](NIP-CTX.md) and [POL](NIP-POL.md) | Scoped evidence, disclosure, independent authority, and spend limits. |
| [EVAL](NIP-EVAL.md) | Optional workload evaluation of providers. A commercial acceptance record is not an EVAL report. |
| [NIP-47](../official/47.md) | An optional wallet transport. Its connection secret stays with the payment host. |

The **buyer** and **provider** are the exact Nostr pubkeys signing their
commercial records. The **worker** is the execution pubkey fixed in the terms;
it may differ from the provider. A **reviewer** or other acceptance authority
has only the role the domain profile and the agreed terms assign. Wallet keys,
relay identities, agent owners, and these commercial principals are distinct.
NIP-OA owner provenance and NIP-AA relay access do not grant contract, workspace,
or payment authority. A host maps its signing key to its actual authority.

An operator MAY run several roles. Attribution must not describe those roles
as independent operators merely because they use different keys. A marketplace
client need not hold an OpenAgents account; a provider MAY impose its own
admission requirements before it issues a quote.

## Kinds and discovery

| Kind | Class | Meaning |
| --- | --- | --- |
| `3192` | Regular | Immutable public offering. |
| `30192` | Addressable | Current offering head and advertised availability. |
| `3188` | Regular | Existing private artifact envelope for commercial records and terms. |

`3192` and `30192` are draft assignments, not registrations. They do not collide
with this repository's pinned official, Block, or OpenAgents allocations.
Implementers must check the kind registry before deployment. No implementation
may advertise market support merely because it stores these kinds.

### Immutable offering

A `3192` event has exactly one `t: oa:market-offering:v1` and one `x` tag equal
to the SHA-256 digest suffix of its exact content bytes. Content MUST be JCS
JSON with these required fields, plus optional inert `meta`:

| Field | Type and meaning |
| --- | --- |
| `v` | `openagents.market-offering.v1`. |
| `requires` | Empty list in v1. |
| `provider` | Pubkey equal to the event signer. |
| `offer` | Unqualified slug, stable across versions of this offering. |
| `capability` | Exact CAP DefinitionRef. |
| `profiles` | Nonempty distinct list of supported domain-profile version strings. |
| `payment_profiles` | Nonempty distinct list containing only the supported profiles defined below. |
| `networks` | Distinct list of `bitcoin`, `testnet`, or `regtest`; nonempty when a paid Lightning profile is offered, otherwise empty. |
| `summary` | Inert display text, at most 2,048 UTF-8 bytes. |
| `price_hint_msat` | Nonnegative integer or null; an advertisement, not the final price. |
| `capacity_hint` | Nonnegative integer or null; an observation, not a reservation. |
| `valid_until` | Unix-second expiry later than the event's `created_at`. |

The required semantic tag is unique; optional `t` tags MAY describe categories.
Neither an offering nor its CAP reference starts a probe, installs a package,
reserves capacity, or discloses task data. Resolve a capability under bounded
fetch policy before proposing work. Preserve the exact signed offering used
by a quote even if its discovery head changes or the relay removes it.

A `30192` head has exactly one `d` equal to its `offer` and exactly one
`t: oa:market-head:v1`. Its content is JCS JSON with required `v:
"openagents.market-head.v1"`, `requires: []`, `provider`, `offer`, `offering`
(exact `3192` EventRef), `status` (`active`, `paused`, or `withdrawn`), and
`valid_until`, plus optional inert `meta`. Provider equals the signer and the
referenced offering's provider; offer slugs also match. Expiry must be later
than `created_at`. A reader uses only a fresh active head for new discovery.
Changing the head cannot amend or cancel an existing quote or order.

[NIP-99](../official/99.md) listings MAY advertise a human-readable link to
an offering. Their Markdown content, approximate prices, and mutable listing
coordinates are not accepted commercial terms. NIP-15 and NIP-90 remain their
own compatibility protocols; no generic job result or payment suggestion is
reinterpreted as an MKT order.

## Private records and authenticated references

All private terms and records use the shared `3188` envelope unchanged:
one recipient, random recipient-scoped `h`, `t: oa:artifact:v1`, NIP-44 v2,
authenticated author/recipient reads, no search, and visibility before COUNT
or limits. Market IDs, prices, invoices, source digests, and commercial
relationships MUST NOT be added as public routing tags. Visible sender,
recipient, and traffic timing still reveal metadata.

An inline artifact uses its exact JCS bytes for its ArtifactRef digest and
size. The envelope signer authenticates that artifact. References below are
ordinary ArtifactRefs; a consuming host MUST have an accessible signed
declaration from the required issuer, either named by the reference or retained
with the exact bytes. A reference alone is not a signature. When an original
issuer creates an authorized copy for another recipient, envelope IDs differ
but the unchanged artifact digest remains the same. A relay or buyer cannot
re-sign a provider's record and thereby replace its authority. If the needed
provenance is unavailable, the transition is unverifiable and cannot authorize
execution or payment.

The two parties retain the agreed terms and evidence outside a single relay.
Adding an evaluator or storage recipient requires disclosure authority;
neither the profile nor a signed quote can make private inputs public.
Revocation or NIP-09 deletion cannot erase an already consumed agreement,
payment liability, or copy held by another party.

Every commercial record has exactly the following fields, plus optional inert
`meta`. `body` is closed by the type table below:

| Field | Type and meaning |
| --- | --- |
| `v` | `openagents.market-record.v1`. |
| `requires` | Empty list in v1. |
| `type` | A type in the record table. |
| `market` | Buyer-generated random 64-hex negotiation ID. |
| `buyer`, `provider` | Distinct exact pubkeys, unchanged throughout this negotiation. |
| `issuer` | Either buyer or provider, equal to the declaring envelope signer. |
| `seq` | Per-issuer nonnegative sequence number within this negotiation. |
| `prev` | Prior record ArtifactRef from this issuer, or null for sequence zero. |
| `issued_at` | Observational Unix seconds; not proof of receipt time. |
| `body` | Type-specific object. |

The first record from each issuer has `seq: 0` and `prev: null`; later records
increment by exactly one and reference that issuer's preceding record.
Fingerprint a record by its JCS artifact digest. An identical digest is a
retransmission. Different bytes for `(buyer, market, issuer, seq)` are an
equivocation: retain both and stop automatic transitions until reconciled.
Missing predecessors cause incomplete history, not permission to choose the
newest timestamp. Neither party's chain orders the other party's actions;
cross-references establish dependencies. A single negotiation has one provider;
requests to multiple providers use separate market IDs.
At most one order may be confirmed in a negotiation. Another purchase needs
a new market ID. Competing quote versions do not authorize duplicate work.

An **OrderRef** is exactly `{market, order_id, buyer, provider, order,
confirmation}`. The first four fields identify the agreed order; order and
confirmation are ArtifactRefs to its signed `order` and confirmed `order_ack`.
They must resolve to matching identities and the same exact quote and terms.
Use OrderRef only after confirmation. Earlier records refer directly to the
order artifact. Domain-profile records bind this OrderRef.

Across this profile, display strings and refusal messages are at most 2,048
UTF-8 bytes. Profile identifiers are nonempty ASCII strings of at most 128
bytes. Lists are bounded to 64 distinct entries unless their type explicitly
permits repetition. Pubkeys, IDs, timestamps, integers, and digests follow
the shared encoding rules. Unknown semantic fields, enums, and versions refuse.

## Terms and records

Quote terms are an artifact with schema `openagents.market-terms.v1`. Its
required fields are `v: "openagents.market-terms.v1"`, `requires: []`, `profile`
(version string), `profile_terms` (ArtifactRef), `buyer`, `provider`, `worker`
(pubkeys), `price_msat`, `fee_limit_msat`, `payment_profile`, `network`,
`quote_expires_at`, `order_confirm_by`, `delivery_due_at`, `review_due_at`,
`payment_due_at`, and `retain_until`. Optional `meta` is inert. Prices and fees
are nonnegative integer Bitcoin millisatoshis; they are not fiat amounts or
token-price estimates. Network is `bitcoin`, `testnet`, or `regtest` for the
Lightning profile, and null for the free profile.

All deadlines are Unix seconds and satisfy:

`quote_expires_at <= order_confirm_by < delivery_due_at < review_due_at < payment_due_at < retain_until`.

The domain profile MUST define its terms schema, signed delivery and acceptance
schemas, authorized acceptance issuers, and complete refusal, failure, and
review behavior. Unknown profiles refuse. The profile terms cannot widen
host grants, alter the commercial price, or introduce a different payment rail.

The record's common buyer/provider always match its terms and referenced
records. All body fields listed below are required. Arrays are bounded to 64
items and references must resolve under the admitted retention and disclosure
policy. Null is valid only where explicitly stated.

| Type | Issuer | Exact body |
| --- | --- | --- |
| `rfq` | Buyer | `{offering: EventRef, profile: string, request: ArtifactRef, price_limit_msat: integer, response_due_at: timestamp, retain_until: timestamp}`. Request is the chosen profile's input schema. |
| `quote` | Provider | `{rfq: ArtifactRef, quote_id: random ID, terms: ArtifactRef}`. Terms use the schema above and price is within the RFQ limit. |
| `order` | Buyer | `{quote: ArtifactRef, terms_digest: Digest, order_id: random ID}`. Buyer accepts exactly the quoted terms. |
| `order_ack` | Provider | `{order: ArtifactRef, decision: confirmed or refused, code: refusal code or null}`. Code is null only for confirmed. |
| `profile` | Either | `{order: OrderRef, record: ArtifactRef}`. Carries a separately authenticated record defined by the domain profile. Forwarding does not confer its issuer's authority. |
| `status_request` | Either | `{order: OrderRef, known: [ArtifactRef]}`. Asks for missing records; it never creates execution. |
| `status` | Either | `{order: OrderRef, records: [ArtifactRef], complete: boolean}`. Reports that issuer's retained view, not global completeness. |
| `cancel` | Either | `{order: OrderRef, reason: string}`. Reason is inert, at most 2,048 bytes. |
| `cancel_ack` | Provider | `{cancel: ArtifactRef, order: OrderRef, stopped: true or false or null, evidence: [ArtifactRef]}`. Null means unknown. |
| `dispute` | Either | `{order: OrderRef, subject: ArtifactRef, reason: string, evidence: [ArtifactRef]}`. Reason is inert, at most 2,048 bytes. |
| `payment_instruction` | Payee | `{order: OrderRef, obligation: random ID, purpose: earned_price or agreed_refund, basis: ArtifactRef, amount_msat: integer, invoice: string, network: string, expires_at: timestamp}`. Rules below determine payer/payee and basis. |
| `payment_attempt` | Payer | `{instruction: ArtifactRef, attempt: positive integer, adapter: DefinitionRef, fee_limit_msat: integer}`. Fee limit equals the terms; persist before any wallet dispatch. |
| `payment_result` | Payer | `{attempt: ArtifactRef, state: confirmed or failed or unknown, amount_msat: integer or null, fees_msat: integer or null, evidence: [ArtifactRef]}`. This is a payer observation, not independent global proof. |
| `payment_received` | Payee | `{instruction: ArtifactRef, amount_msat: integer, evidence: [ArtifactRef]}`. Requires verified receipt under the payee's payment adapter. |
| `refund_offer` | Buyer | `{order: OrderRef, original_instruction: ArtifactRef, refund_id: random ID, amount_msat: integer, reason: string}`. Proposes a refund to the buyer; amount is positive and bounded by confirmed paid principal not already refunded or reserved for refund. |
| `refund_accept` | Provider | `{offer: ArtifactRef}`. Accepts the exact refund offer after reserving the refund principal. |
| `close` | Either | `{order: OrderRef, records: [ArtifactRef], unresolved: [ArtifactRef]}`. A proposed final inventory of relevant profile and settlement records. |
| `close_ack` | Other party | `{close: ArtifactRef}`. Acknowledges that inventory; does not rewrite any outcome. |

For `earned_price`, the provider is payee and buyer is payer. Basis is the
domain profile's valid signed acceptance artifact for the exact OrderRef and
delivery. For `agreed_refund`, buyer is payee and provider is payer; basis is
the signed `refund_accept`, whose offer identifies the original confirmed
payment. Refund amount must equal the accepted offer. A dispute or refund
promise alone does not reverse a payment. A partial refund changes the retained
net amount, not the original payment record.

Refusal codes are the shared codes plus `unsupported_profile`, `quote_expired`,
`quote_consumed`, `confirmation_expired`, and `price_exceeded`. Unknown codes
remain an unsupported refusal and never authorize an effect. A type that
requires OrderRef cannot occur before a confirmed order. A refund offer may
be proposed without a reservation. The provider, who pays the refund, atomically
reserves its amount before signing acceptance. Accepted refunds count
against refundable principal before an invoice or payment attempt exists.

## Negotiation and admission

1. The buyer validates a fresh signed offering and its supported capability,
   then sends a private RFQ within host disclosure policy. The RFQ deadline
   and retention are future times, with retention after the response deadline.
   Its provider must equal the offering signer and declared provider. Resolve
   the exact capability DefinitionRef and verify its complete signed identity;
   neither a matching display name nor a different current head substitutes.
   RFQ profile must be present in the offering's supported profiles.
2. The provider replies to that RFQ with an immutable quote before its
   response deadline. The quote is neither capacity reservation nor admission.
   The terms' buyer/provider match the RFQ and `quote_expires_at` is after quote
   issuance. The provider may issue another quote; each has a new quote ID.
   Terms profile must equal the RFQ profile; payment profile must be offered,
   and a paid quote's network must occur in the offering's networks. A free
   quote requires null network. The resolved domain terms must satisfy the
   chosen profile and the offered capability's interface. If that capability
   pins a remote worker, it must equal the quoted worker. Any mismatch refuses.
3. The buyer signs one order naming the exact quote and terms digest. Changing
   price, worker, deadlines, rights, or acceptance terms requires a new quote
   and order. A quote may be consumed by at most one order.
4. The provider validates the order while the quote is unexpired, checks its
   own admission policy, and durably binds `(buyer, market, order_id)` and the
   quote ID to the order digest. It reserves the capacity and allowances it
   promises before confirming by `order_confirm_by`. Same identities and bytes
   retrieve the original decision; changed bytes refuse `idempotency_conflict`.
   Confirmation atomically excludes every other order in that negotiation.
   A refusal consumes no execution authority. An expired or unavailable quote
   receives a refusal; it is not silently repriced.
5. Bilateral commercial agreement exists only with the quote, the buyer's
   order, and the provider's confirmed acknowledgment. Either host may still
   refuse execution whose independent grants or required enforcement fail.
   The confirmed terms determine how that failure is reported; confirmation
   must never manufacture a missing credential, wallet grant, or sandbox.
6. The labor profile binds a separately admitted CJ execution to OrderRef.
   Its exact request, attempt, worker, input, and RUN records identify actual
   work. Order confirmation is not CJ admission, and CJ admission is not proof
   of dispatch, verification, commercial acceptance, or payment.

Hosts declare allowed clock skew and use their own receipt time when applying
live expiry. A sender cannot backdate a record to force a new admission.
Offline readers may verify declared timestamps but cannot infer unrecorded
receipt times. Late copies of an existing confirmed agreement remain history;
they do not start another job. Each host persists its decision before reporting
it and enforces one active dispatch identity per admitted order under the
domain profile. These idempotency checks do not cover a different provider.

## Completion, cancellation, and disputes

Keep commercial agreement, execution, verification, acceptance, and payment
as separate states. Delivery follows the domain profile. No `status`, final
prose, exit code, relay `OK`, or payment receipt substitutes for valid acceptance.
Buyer silence at `review_due_at` is unresolved review, never implicit acceptance.
Worker silence at `delivery_due_at` is missing or unknown delivery, never proof
that it did not execute. Expiring an order does not terminate a running worker.
Passing `payment_due_at` makes an unpaid obligation overdue; it does not erase
it or prove a dispatched payment failed. Paying an overdue obligation still
requires current wallet authority and a valid invoice.

Cancellation records intent. The provider records it durably, blocks queued
dispatch, and requests cancellation through CJ for admitted work. `stopped:
true` needs retained evidence that the relevant work stopped; it does not
erase effects already produced. Missing evidence is null, and uncertain
execution or spending stays reserved pending reconciliation. A cancellation
after acceptance does not extinguish an earned payment. Before acceptance,
cancellation alone produces no payable instruction. The owning profile still
assesses already-delivered work and can establish acceptance and the full
price after the cancellation request. Only a supported final rejection
establishes zero labor price; unresolved review remains unresolved. The
provider bears attempted-work cost when the final outcome earns no payment.

A dispute names the precise delivery, acceptance, payment, or refund record
in question. Retain both claims and the cited evidence. Domain-profile rules
determine any revised acceptance; neither side may unilaterally amend agreed
requirements. A payer wallet alone establishes its own payment observations,
not the counterparty's position. This v1 has no default arbitrator or automatic
debit. Mutual refund agreement and its actual transfer are recorded separately.
If the parties cannot resolve a dispute, show it as unresolved.

A close/close_ack pair agrees on a retained inventory. It cannot turn unknown
effects into failures, mark an unpaid accepted job paid, discard a disputed
record, or release a reservation without evidence. Unresolved items remain
visible after closure. Neither peer disappearance nor missing relay history
counts as a clean close.

## Fixed-price Bitcoin settlement

### Supported payment profiles

`free-v1` requires `price_msat: 0`, `fee_limit_msat: 0`, and `network: null`.
It generates no payment instruction or wallet call. Its state is `not_required`,
not a zero-value claimed Bitcoin payment. Use it for a no-spend rehearsal.

`lightning-bolt11-fixed-postacceptance-v1` requires positive `price_msat`.
The provider explicitly bears credit risk: accepted work may remain unpaid.
Neither a quote nor a local budget reservation proves that funds are held for
the provider. This profile provides no escrow or atomic exchange of a patch
for payment. Tests cannot create those guarantees.

One earned-price obligation is permitted per order. An earned-price instruction
must name valid profile acceptance and exactly the fixed `price_msat`.
The invoice is a BOLT11 string at most 8,192 UTF-8 bytes, signed and parseable
under the payment host's supported network. The payee's signed instruction
binds that invoice's actual Lightning payee to this obligation; its wallet
pubkey need not equal its Nostr pubkey. Verify invoice signature, network,
nonzero exact amount, expiry, and payment hash before dispatch. `expires_at`
must equal the decoded invoice expiry. Amountless invoices refuse in this
profile. A profile acceptance for another order, altered artifact, rejected
delivery, or unrecognized acceptance issuer cannot make an invoice payable.
The payment host must apply the domain profile's supersession and conflict
rules before dispatch. A known superseded acceptance is not a payment basis;
known conflicting final records suspend automatic settlement until their
supported reconciliation resolves them. An unavailable required history stays
unverifiable. No participant can claim complete global history from one relay.

### Authorization and payment attempts

The payer independently authorizes the exact invoice, amount, fee bound,
network, obligation, and payment adapter under host policy. Provider text,
RFQs, generated code, and a market signature cannot access the wallet. Wallet
secrets, NWC connection URIs, bearer tokens, and spend credentials must stay
outside worker context, task workspaces, relays' plaintext, logs, and receipts.

Persist a payment attempt and its durable reservation before a wallet request.
The payment ledger's key is `(order_id, obligation, payer)` under the exact
market identity. It tracks invoice hashes, attempts, known paid principal,
fees, and outstanding liabilities. An identical attempt is replayed as state;
a changed request under its identity refuses. Another invoice or payment hash
for that obligation cannot bypass its outstanding or settled state.
The complete attempt key is `(buyer, market, order_id, obligation, payer,
attempt number)`. Numbering starts at one and increments across replacement
invoices; it never resets for a new hash. Its fingerprint binds the exact
payment-attempt and instruction bytes, decoded invoice identity, adapter, and
fee bound. The host additionally pins the effective wallet authorization in
its protected ledger. Payer `payment_result` evidence names the attempt's
adapter. Payee `payment_received` evidence instead names that payee's admitted
receiving-wallet adapter; both bind the same instruction, hash, and amount.

The host must enforce `fee_limit_msat` for each payment and bound aggregate
retry fees under separately authorized policy. The pinned NIP-47 `pay_invoice`
method has no standard maximum-fee argument: NWC support alone cannot satisfy
this bound. Use a wallet/adapter with independently enforced limits, or refuse
the requested payment. Missing fee evidence stays unknown, never zero.

A payee may replace an expired invoice only for the same obligation and
amount after every earlier payment attempt is reconciled as unpaid. Retain
the replaced instruction. An expired invoice or a new payee instruction
does not prove an earlier payment failed. Do not pay a replacement while a
prior attempt is pending, unknown, or confirmed.
The replacement invoice must retain the order's network and exact amount.

### Evidence and reconciliation

Payment evidence is an ArtifactRef whose schema is
`openagents.market-payment-evidence.v1`. The closed body has `v` with that
value, `requires: []`, `order: OrderRef`, `obligation`, `instruction`
(ArtifactRef), `observer` (pubkey), `adapter` (DefinitionRef), `network`,
`payment_hash` (64-hex), `state` (`confirmed`, `failed`, or `unknown`),
`amount_msat` (integer or null), `fees_msat` (integer or null), `observed_at`,
and `source` (ArtifactRef to the retained wallet response or authoritative
lookup), plus optional inert `meta`. The observer's signed envelope and local
trust policy authenticate the observation; it is not remote attestation.
The source is private and must omit secrets; a redacted representation records
its derivation and limitations under the shared evidence contract.

Verify the configured wallet identity, exact wallet-request binding, decoded
invoice identity, amount, and any returned preimage against the payment hash.
A preimage alone does not identify the payer or prove that a dishonest recipient
did not reveal it without payment. A `confirmed` payer result requires validated
evidence from the payer's admitted payment adapter and a known principal equal
to the instruction. The payee independently checks receipt before issuing
`payment_received`. Disagreement remains visible; clients distinguish payer
confirmation from payee acknowledgment and state their assurance source.
An unknown observation may later acquire conclusive evidence. A confirmed
payment remains in the ledger; a conflicting later result requires explicit
reconciliation and cannot silently replace or reverse it.

An absent response, connection loss after dispatch, wallet error with ambiguous
effect, or crash produces `unknown`. Preserve the reservation and query the
same wallet/payment identity before another attempt. Failure evidence must
establish that the payment did not settle and cannot still settle; a transport
error or elapsed timeout alone is insufficient. If the adapter cannot establish
that, keep the obligation unknown for deliberate reconciliation. A new attempt
increments the attempt number only after the old one is reconciled and fresh
host admission authorizes its possible fees.

[NIP-57](../official/57.md) explicitly treats a zap receipt as a claim by the
recipient's wallet, not independent proof of payment. It MAY be attached as
supporting evidence but cannot alone satisfy this profile. [NIP-60](../official/60.md)
wallet state and [NIP-61](../official/61.md) Cashu receipts have different
semantics and do not settle this Lightning profile. A future payment profile
must define its own rail, trust, denomination, fee, expiry, and recovery rules.

Refunds use a new obligation, the signed `refund_accept` artifact as basis, and the
same authorization and recovery procedure with payer/payee reversed. Keep the
original confirmed payment intact. Retain the reservation made when the refund
was accepted through invoice preparation and dispatch. The sum of confirmed,
accepted-but-not-dispatched, and unresolved refunds cannot exceed original
confirmed principal. No automatic refund follows a failed check or disputed claim.

## Retention and host boundaries

Retain all quoted and confirmed terms, author declarations, invoices, wallet
observations, profile decisions, CJ identities, and unresolved liabilities
through the accepted `retain_until`. Required storage must be admitted before
confirmation; the number in an envelope is not a relay retention promise.
Unresolved liabilities require reconciliation or an explicit retained
unresolved disposition before ordinary history is pruned. Expired/missing
history is `content_unavailable` or unknown and cannot authorize replay.

Commercial waiting is persisted market state. It does not suspend a live CJ
request, extend its deadline, or promise generic durable PRG continuation.
An incoming authenticated reply causes a new host-admitted processing action.
Execution beyond an old job's deadline needs the domain's explicit rework or
new-order path and fresh CJ admission. Retain unresolved earlier effects.

Relays enforce only their advertised public syntax and private-envelope roles.
They cannot validate encrypted commercial transitions. Clients bound total
history, fetches, schema evaluation, quotes, outstanding orders, and wallet
attempts. Refuse unsupported semantics and oversized input instead of silently
truncating terms. Sharing history across relays preserves exact signed records;
relay failover never changes a counterparty, accepted price, or disclosure scope.

## Conformance

Advertise `nip-mkt-v1` only with explicit roles: `offering-publisher`,
`market-client`, `market-provider`, `settlement-lightning`, or `relay-envelope`.
State supported domain and payment profile versions. These are named
extensions, not numeric NIP-11 `supported_nips`. A relay-envelope role is not
a working labor marketplace. Each role needs fixtures for the semantics it
claims; real Bitcoin settlement additionally needs adapter and recovery evidence.

The conformance set must cover at least:

- Canonical bytes, malformed/unknown fields, signatures, public head mismatch,
  expired offers, private ACLs, unavailable provenance, and unauthorized copies.
- Complete RFQ/quote/order/confirmation flow; altered terms; reused quote;
  duplicate records; sequence gaps, forks, stale clocks, and late acknowledgments.
- Admission refusal after quotation; no dispatch from discovery; missing
  capability/grant; order/CJ/worker mismatch; separate technical and commercial
  acceptance; buyer silence; failed and unknown delivery; stale requirements.
- Cancellation before and after dispatch, cancellation after acceptance,
  provider restart, relay replacement, late results, and unresolved close.
- Zero-cost rehearsal with no wallet call; wrong invoice network, payee binding,
  amount, signature, expiry, or acceptance; unsupported fee enforcement;
  unknown fees; secret-free receipts; dishonest payment claims.
- Crash before/after wallet dispatch, duplicate payment instructions, changed
  invoices for one obligation, uncertain errors, authoritative reconciliation,
  failed payment, unpaid accepted work, partial refunds, duplicate refunds,
  and refund reservations that would exceed confirmed principal.

Passing fixtures proves the stated implementation behavior. It does not prove
market liquidity, provider competence, profitable operation, or universal
correctness of the agreed labor checks.
