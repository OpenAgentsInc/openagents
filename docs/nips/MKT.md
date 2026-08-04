# NIP-MKT

## Negotiated Markets

`draft` `optional`

This NIP defines a narrow common wire for negotiated markets on Nostr:
public provider and offering discovery followed by a private, signed
RFQ/Quote/Order/Status/Close exchange.

It is the forward market base for OpenAgents. It replaces neither an
underlying settlement protocol nor the use-case-specific rules that make an
atomic swap, peer-to-peer trade, mint exchange, liquidity lease, fiat rail,
dataset license, or labor engagement correct. It replaces the practice of
putting all of those meanings into the generic NIP-90 request/result ranges.

The governing boundary is:

> NIP-MKT coordinates a negotiation. The owning profile and its external
> rail prove execution and settlement. Relay acceptance proves transport
> only.

## Scope

NIP-MKT base owns:

- provider, profile, and offering discovery;
- private RFQs, provider-authenticated quotes, requester-authenticated
  orders, sequenced status, cancellation, and terminal close records;
- exact correlation, expiry, idempotency, replay, recovery, and fork rules;
- the distinction between indicative and capacity-reserving quotes;
- a common privacy envelope and independently verifiable inner signatures;
- references to evidence, payment, and settlement without claiming their
  authority; and
- a bounded public receipt that may disclose a redacted close claim.

NIP-MKT base does **not** define:

- asset identifiers, units, prices, fees, or liquidity calculations;
- wallet commands, spend authorization, invoices, addresses, scripts, or
  transaction construction;
- custody, escrow, underwriting, credentials, disputes, refunds, or legal
  obligations;
- when a chain, Lightning payment, mint, federation, bank, provider ledger,
  or other rail is final;
- a universal reputation, risk, custody-level, or provider-ranking score; or
- a universal market authority. A bilateral market does not inherit the All
  Work authority merely because both use OpenAgents NIPs.

Those meanings belong to focused profiles. A profile that cannot identify
its actual signer, verifier, finality, recovery, and settlement authorities
is incomplete and MUST NOT present itself as executable.

## Three-lane composition review

This draft was allocated after collision and semantic review across the
official, Block, and OpenAgents lanes.

### Official NIPs

- NIP-01 supplies event IDs, signatures, tags, and addressable storage.
- NIP-09 deletion requests are not cancellation, revocation, or settlement
  reversal.
- NIP-17, NIP-44, and NIP-59 supply private delivery. NIP-MKT adds an
  independently signed inner record because a NIP-59 rumor is unsigned.
- NIP-32 and NIP-85 may carry trust assertions. They remain claims by their
  signers, not provider admission or settlement proof.
- NIP-39 may bind external identities when a profile requires them.
- NIP-40 supplies `expiration`; expiry never implies consent.
- NIP-42 protects recipient-scoped relay reads and writes.
- NIP-46 may provide remote signing and NIP-47 may provide wallet actions.
  Neither grants a market relay spend authority, and NWC connection secrets
  never enter a market record.
- NIP-51, NIP-65, NIP-66, NIP-87, NIP-89, and NIP-99 remain useful for
  curated discovery, relays, relay monitoring, mint discovery, handlers, and
  generic listings. NIP-MKT does not replace them.
- NIP-57/NIP-61 payment records, NIP-69 peer-to-peer orders, and NIP-87 mint
  announcements are rail-specific inputs to focused profiles. A profile
  preserves their authority rather than translating a relay claim into a
  payment fact.
- NIP-90 remains historical compatibility. New NIP-MKT records MUST NOT use
  new NIP-90 job kinds or `kind:7000` feedback.

### Block NIPs

- NIP-OA may prove owner-to-agent provenance. It never lets an owner silently
  sign as an agent or lets an agent silently spend for an owner.
- NIP-AA grants relay access to an owner-attested agent. It grants no market,
  wallet, provider, group, or administrative authority.
- NIP-IA is relay-scoped archive state, not a market ban or reputation
  verdict.
- NIP-AE owns private agent memory; NIP-AO and NIP-AM own live telemetry and
  durable private turn metrics. NIP-MKT records market facts, not runtime
  state.
- NIP-DV demonstrates that private derived state needs both filter-level and
  result-level authorization. Relays serving gift wraps SHOULD enforce the
  NIP-17 recipient gate at every delivery surface.

### OpenAgents NIPs

- NIP-SKL identifies skills; NIP-SA supplies agent budgets, guardians, and
  delegation; NIP-AC may fund an exact NIP-MKT Order ID. None makes the
  market relay a wallet or settlement authority.
- NIP-DS owns dataset identity, digests, offers, and access contracts.
  Dataset negotiation may reuse this spine only through a DS-owned profile.
- NIP-LBR v1 stays historical NIP-90 compatibility. A future labor
  microstandard may reuse the spine without inheriting liquidity semantics.
- NIP-TRN owns training coordination and artifact receipts.
- NIP-EV keeps evidence, independent verification, and disposition separate.
  NIP-OC keeps accepted outcomes and closeouts separate from settlement.
  NIP-MKT Status and Close may reference them but cannot manufacture their
  authority.
- NIP-AD and NIP-SA delegations must bound capability, amount, expiry, and
  generation before an agent signs an Order.

## Roles and authority

- **requester** — asks for terms and signs an Order.
- **provider** — publishes an Offering, signs Quotes, and reports execution.
- **profile authority** — publishes a Profile Descriptor. Clients still
  choose which authority keys and revisions they trust.
- **verifier** — checks profile-specific evidence.
- **relay** — stores public records and private gift wraps, applies bounded
  validation and access policy, and transports messages.
- **settlement authority** — the external system whose rules make settlement
  final. It is named by the profile and is never implied by the relay.

A signature proves signer and bytes. It does not prove the signer owns
liquidity, controls an endpoint, holds a credential, reserved capacity,
executed an order, received funds, refunded a counterparty, or settled.

## Kind allocation

This NIP reserves these collision-reviewed addressable kinds:

| Kind        | Type                    | Description                               | Publication                        |
| ----------- | ----------------------- | ----------------------------------------- | ---------------------------------- |
| 39600       | Addressable head        | Provider Profile                          | public                             |
| 39601       | Addressable head        | Offering                                  | public                             |
| 39602       | Addressable head        | Market Profile Descriptor                 | public                             |
| 39603       | Addressable, unique `d` | Public Market Receipt                     | public, optional                   |
| 39604       | Addressable, unique `d` | RFQ                                       | private signed record              |
| 39605       | Addressable, unique `d` | Quote                                     | private signed record              |
| 39606       | Addressable, unique `d` | Order                                     | private signed record              |
| 39607       | Addressable, unique `d` | Status                                    | private signed record              |
| 39608       | Addressable, unique `d` | Cancel                                    | private signed record              |
| 39609       | Addressable, unique `d` | Close                                     | private signed record              |
| 39610       | Addressable, unique `d` | MKT-SWP Swap Contract                     | private signed record, NIP-59 only |
| 39611-39629 | —                       | Reserved for separately reviewed profiles | unallocated                        |
| 39630       | Addressable head        | MKT-PFI Qualification Policy              | public, no PII                     |
| 39631-39699 | —                       | Reserved for separately reviewed profiles | unallocated                        |

NIP-01 makes every `30000 <= kind < 40000` event addressable. The private
records use a new, unique `d` for every logical message and are
**immutable-by-contract**: correction or supersession creates a new record
referencing its predecessor. Implementations MUST reject changed bytes under
an existing `(pubkey, kind, d)` even if a generic relay would select a newer
head.

Public heads, including `kind:39630`, intentionally use stable `d` values and
NIP-01 replacement. Private profile kinds such as `kind:39610` inherit the
unique-`d`, immutable-by-contract, and NIP-59-only rules from the base private
records.

The `39600-39699` block was unused by the pinned official, Block, and
OpenAgents lanes and by the official registry-of-kinds `schema.yaml` when
checked on 2026-08-04. Official `kind:39701` remains outside the reservation.
Implementers MUST repeat a registry and three-lane collision review before
allocating a profile kind from the reserved block.

MKT-SWP and MKT-PFI repeated that review immediately before allocating
`kind:39610` and `kind:39630`. The reviewed revisions were official NIPs
`c53877571f96eb423661fc23c620d629d37b8f19`, Block Buzz
`56003ebf98c22367fb6357f295494e26efbd8ae6`, this OpenAgents tree at
`1c9a957ab8275e23e7952af2395d36c681b5246d`, and registry-of-kinds
`2483e752146d171524dcb10dffd06de2aa271bf3`. No source assigned a kind in
`39610-39699`; the only matches were this NIP's reservation text.

## Common grammar

### Identifiers

- `provider_id`, `offering_id`, and `profile_id` are lowercase strings of
  1-64 ASCII characters matching `[a-z0-9][a-z0-9._-]*`.
- `session_id` is 32 random bytes encoded as 64 lowercase hex characters. It
  MUST NOT be derived solely from participant pubkeys, offering IDs, or time.
- A private record `d` is a 32-byte collision-resistant idempotency key
  encoded as 64 lowercase hex characters and unique within the author's kind.
- A Public Market Receipt uses a new unique `d` and MUST NOT reuse the private
  session ID.

### Common tags on private records

Every kind `39604-39609` signed record contains exactly one each of:

- `d`: immutable message/idempotency identifier;
- `session`: random session ID;
- `profile`: profile ID and positive decimal version, for example
  `["profile", "mkt-swp", "1"]`;
- at least one `p` counterparty tag whose fourth element is `requester` or
  `provider`; and
- `alt`: a fixed, non-sensitive description of the record kind.

Deadlines use NIP-40 `["expiration", "<unix-seconds>"]`. Exact causal
references use `e` tags whose fourth element is `rfq`, `quote`, `order`,
`previous`, `status`, `cancel`, `close`, `evidence`, or `settlement`. An
Offering reference uses an `a` tag marked `offering`. Relay hints are
recommendations, never authorities.

### Content

`content` is a UTF-8 JSON object and includes:

```json
{
  "schema": "openagents.mkt.v1",
  "profile": "<profile_id>",
  "profile_version": 1,
  "session_id": "<64-lower-hex>"
}
```

Body values MUST agree with tags. Duplicate JSON member names at any nesting
level are invalid. Unknown members are retained when forwarding and ignored
unless the selected profile marks them critical. An unknown profile,
unsupported version, or unsupported critical member fails closed.

The signed event ID commits to the exact content bytes. Later events refer to
that ID instead of re-canonicalizing or restating earlier terms.

### Bounds

Implementations MAY impose tighter published limits. Base implementations
MUST reject:

- public Provider Profile, Offering, or Descriptor content over 16 KiB;
- Public Market Receipt content over 4 KiB;
- a serialized private signed record over 32 KiB;
- more than 64 tags, 8 `p` tags, 32 causal/evidence references, 16
  advertised profiles, or 8 relay/end-point hints; or
- an identifier or profile collection over its declared bound.

Large evidence, credentials, documents, and artifacts stay off-relay behind
digests and authorized retrieval references.

## Public discovery

### Provider Profile (`kind:39600`)

Address: `39600:<provider_pubkey>:<provider_id>`.

Required tags: stable `d`, `status` (`active`, `paused`, or `retired`), each
supported `profile` and version, and `published_at`.

Recommended tags: inbox `relay` URLs, `a` references marked `offering`,
NIP-39 `i` claims, NIP-32 labels, and `expiration` when temporary.

`content` contains bounded public metadata such as name, summary, support
contact, terms URL, and public credential pointers. It MUST NOT contain live
inventory, private endpoints, bearer credentials, wallet material, customer
lists, or claims that an external verifier has not signed.

NIP-OA owner provenance or NIP-OT role references explain a relationship;
they do not make that owner or Organization the event author.

### Offering (`kind:39601`)

Address: `39601:<provider_pubkey>:<offering_id>`.

Required tags: stable `d`, exactly one `profile` and version, `status`
(`active`, `paused`, `exhausted`, or `retired`), `provider` profile address,
and `published_at`.

Recommended tags are profile-defined public discovery fields such as network,
asset class, direction, region, credential burden, minimum/maximum range,
custody dimensions, and availability. Each is an advertisement, not a
reservation or proof of current capacity.

Private prices, exact inventory, addresses, scripts, invoices, account data,
and eligibility material belong in private negotiation. An Offering may
cross-reference a NIP-99 listing, NIP-87 mint announcement, NIP-69 order, or
NIP-89 handler. Each referenced object remains canonical for its own meaning.

### Market Profile Descriptor (`kind:39602`)

Address: `39602:<profile_authority_pubkey>:<profile_id>`.

Required tags: stable `d`, positive `version`, `x` SHA-256 of the exact
profile-specification bytes, `r` retrieval URL, and `status` (`draft`,
`active`, `deprecated`, or `withdrawn`).

Recommended tags pin the parent NIP-MKT version, profile allocations,
fixture-corpus digest, prior compatibility, and superseded descriptor.

A Descriptor is discovery data, not executable code or global ownership of a
profile name. Clients MUST allowlist profile-authority keys, pin the event ID
and `x`, and deny fetched specifications every ambient code, network, wallet,
file, and signing capability.

### Public Market Receipt (`kind:39603`)

This is an optional, deliberately redacted signer claim about a private
Close. Required tags:

- unique `d`;
- `profile` and version;
- `outcome`: `completed`, `cancelled`, `expired`, `failed`, `refunded`,
  `disputed`, or `unresolved`;
- `x`: exact private Close event ID; and
- `role`: receipt-author role.

The receipt MAY reference public-safe NIP-EV evidence, NIP-OC closeout,
NIP-32/NIP-85 assertion, or external settlement proof. It MUST NOT reveal the
private session ID, counterparty, amount, asset, route, timing, or rail unless
every affected party and the profile permit disclosure.

It is one signer's claim. Matching receipts, high counts, or relay `OK` do
not create settlement authority.

## Private signed-record transport

RFQ through Close are fully signed Nostr events, so exact bytes and authors
can be verified independently. They MUST NOT be published bare to a public
relay. The default transport embeds each signed record inside a NIP-59 rumor
and gift-wraps it separately to every recipient and to the sender for
recovery.

Clients use the recipient's NIP-17 `kind:10050` inbox relays, preferably a
small set. A missing usable inbox list fails closed unless the parties already
agreed an authenticated restricted relay out of band. Persistent negotiations
use `kind:1059`; ephemeral `kind:21059` is insufficient for offline delivery
and recovery.

For signed record `R` of kind `39604-39609`, sender `S`, and recipient `P`:

1. Construct, hash, and sign `R` normally under `S`. Retain its exact compact
   JSON serialization.
2. Construct an unsigned NIP-59 rumor with the same kind as `R`,
   `pubkey=S`, a `p` tag for `P`, a unique `d`, and `content` equal to the
   exact compact JSON serialization of `R`.
3. Seal the rumor as NIP-59 `kind:13` and gift-wrap it as `kind:1059` to `P`
   using NIP-44 and NIP-59 timestamp randomization.
4. Repeat with independent wraps for every recipient and for `S`. Reuse the
   same signed `R`; do not create recipient-specific terms.

The seal's tags remain empty as NIP-59 requires. On receipt, a client MUST:

1. validate and decrypt the gift wrap and seal;
2. enforce `seal.pubkey == rumor.pubkey`;
3. parse `rumor.content` as one complete signed event `R` with no trailing
   bytes;
4. recompute `R.id`, verify `R.sig`, and require
   `R.pubkey == rumor.pubkey == seal.pubkey`;
5. require `R.kind == rumor.kind`, a supported private MKT kind, and the
   recipient in `R` with its expected role;
6. validate base and selected profile before admitting `R`; and
7. retain signed `R`, seal ID, receipt time, and delivery provenance locally.

The signed inner event lets a participant disclose one exact record to an
authorized verifier without disclosing a NIP-44 conversation key or unrelated
messages. A disclosed copy remains private and MUST NOT be republished without
the profile's disclosure authority.

A restricted relay or out-of-band encrypted channel MAY carry the identical
signed record, preserving every validation, privacy, idempotency, and recovery
rule. NIP-29 membership alone is not encryption.

## Private record types

### RFQ (`kind:39604`)

The requester asks one or more providers for terms. It additionally requires:

- requester signature;
- a `p` tag marked `provider` for each intended provider;
- an `a` tag marked `offering` unless the profile permits blind capability
  RFQs;
- `expiration`; and
- profile content specifying direction, bounds, constraints, credential mode,
  privacy mode, and response requirements.

An RFQ is an invitation to quote, not an Order, reservation, wallet command,
funding authorization, or eligibility proof. A multi-provider RFQ uses
separate wraps; recipients need not learn who else received it.

### Quote (`kind:39605`)

The provider answers one exact RFQ. It additionally requires:

- provider signature;
- one `e` reference marked `rfq`;
- a `p` tag marked `requester`;
- `expiration`;
- `quote`: `indicative` or `firm`;
- `reservation`: `none`, `soft`, or `hard`; and
- complete profile terms, fees, limits, qualification conditions, custody
  dimensions, execution window, cancellation boundary, evidence requirements,
  and recovery path.

`indicative` terms require provider confirmation. `firm` declares that a
conforming timely Order is accepted under stated preconditions; it does not
prove the declaration or capacity.

Reservation values mean:

- `none`: no capacity is held;
- `soft`: capacity is planned but may be reallocated under profile rules;
- `hard`: capacity is committed against a profile-defined reserve and
  equivocation is a protocol violation.

The profile defines capacity, verifier, double-reservation detection, and
expiry. A relay may track signed reservations and reject obvious replay or
over-allocation under configured policy; its record does not prove liquidity.

A corrected Quote gets a new `d` and `previous` reference. The old bytes are
never silently replaced.

### Order (`kind:39606`)

The requester accepts one exact Quote. It additionally requires requester
signature, exactly one `quote` event reference, a `provider` counterparty,
and the profile order parameters.

The Quote event ID commits accepted terms. An Order MUST NOT restate a
different price, amount, asset, fee, route, or expiry and call it acceptance.
A profile may allow a bounded subset only by defining exactly selectable
fields.

For a `firm` Quote, a valid timely Order becomes protocol-effective under the
profile preconditions. An `indicative` Quote requires provider
`Status state=accepted`. Silence, relay acceptance, an invoice, or an address
is never acceptance.

An Order authorizes coordination only. It is not an NIP-47 wallet request,
NIP-AC spend authorization, signature request, custody transfer, payment, or
settlement.

### Status (`kind:39607`)

A participant reports one ordered-session transition. It requires:

- exactly one `order` event reference;
- `seq`: non-negative decimal sequence per author, starting at `0` and
  increasing by one;
- `state`: `accepted`, `rejected`, `awaiting_input`,
  `funding_required`, `funding_observed`, `executing`,
  `settlement_pending`, `completed`, `refund_pending`, `refunded`,
  `disputed`, or `failed`; and
- for `seq > 0`, exactly one `previous` reference to that author's prior
  Status.

Profiles define allowed signers, transitions, timers, evidence, and finality,
and may narrow or extend the state set. Unknown states do not advance state.

A Status is an observation or claim. `funding_observed`, `completed`, and
`refunded` do not imply verified or settled. Evidence/settlement references
carry a profile provenance label such as `pledged`, `reserved`, `measured`,
`verified`, `paid`, or `settled`. Labels are never inferred upward.

Missing sequence numbers are displayed gaps. Two events from one author at
the same `(session, order, seq)` are an equivocation fork; retain and surface
both instead of choosing by time.

### Cancel (`kind:39608`)

A requester or provider asks to stop, answers a request, or records effective
cancellation. It requires:

- one `order` reference;
- `action`: `request`, `accepted`, `rejected`, or `effective`;
- profile-defined machine-readable `reason`; and
- when answering, a `cancel` reference to the request.

`action=request` changes nothing. Cancellation becomes effective only through
profile signer, timing, irreversible-step, and recovery rules. It cannot undo
external payment, revealed preimage, broadcast transaction, final fiat
settlement, or custody by assertion. A terminal cancellation gets a Close.
NIP-09 deletion is not cancellation.

### Close (`kind:39609`)

One participant's terminal reconciliation. It requires:

- one `order` reference;
- `outcome`: `completed`, `rejected`, `cancelled`, `expired`, `failed`,
  `refunded`, `disputed`, or `unresolved`;
- `terminal_at`; and
- profile final state, loss accounting, evidence inventory, recovery
  disposition, and public-receipt consent.

Recommended references include final Status, effective Cancel, verifier
evidence, NIP-EV receipts, NIP-OC closeout, NIP-AC receipt, and external
settlement proof.

A Close is terminal for its signer and Order. Correction uses a new `d` and
`previous` reference. Counterparties may sign matching or conflicting Close
records; agreement is not inferred and disagreement is not hidden.

Close does not move funds and is not an NIP-OC Accepted Outcome, owner
Disposition, refund, or settlement receipt. Clients display the narrowest
rung that exact evidence proves.

## State, replay, and recovery

```text
Provider Profile + Offering
          -> RFQ -> Quote -> Order
          -> Status(0..n) / Cancel
          -> Close
```

This is not a universal settlement state machine. Profiles own branches and
external transitions.

### Idempotency

For fixed `(pubkey, kind, d)`:

- identical signed bytes are replay and return the previous result;
- a different event ID or body is `idempotency-conflict` and fails closed;
- providers do not reserve or execute twice because multiple wraps arrive;
  and
- delivery retry re-wraps the same signed record rather than re-signing it.

Profiles bind exact Order ID to every external operation ID before an
irreversible effect.

### Expiry and clocks

Writers do not create an RFQ, Quote, or bounded Order after referenced expiry.
Receivers reject late observations unless the profile defines explicit clock
skew. `created_at` is author-controlled, not trusted time.

Expiry releases only profile-defined reservation. It never signs Cancel,
accepts, settles, refunds, publishes, or transfers custody. Relays may retain
expired events, so clients enforce NIP-40 themselves.

### Recovery

Every sender receives its own wrap and persists the signed record. On
reconnect, participants replay local causal history and request missing
records through authorized inboxes. Recovery returns original signed records,
not new projections pretending to be history.

Lost records, sequence gaps, conflicting forks, expired wraps, and unavailable
evidence are explicit loss states. Never fill them from current UI or provider
database state.

## Profile contract

Every executable profile defines:

1. ID, version, authority key, specification digest, and fixtures;
2. roles, eligibility, credentials, and exact signer map;
3. assets, networks, directions, units, precision, limits, and fees;
4. quote fields, allowed Order selection, capacity, and reservation accounting;
5. custody dimensions: principal control at each stage, unilateral paths,
   timelocks, and maximum custody duration;
6. transitions, timers, cancellation, outcomes, disputes, refunds, recovery;
7. private/public field classification and deliberate metadata leaks;
8. rail adapters and exact verifier for funding, execution, finality, reorgs,
   replacement, chargebacks, and settlement;
9. idempotency between Order ID and every external effect;
10. errors, unsupported-version behavior, and loss accounting; and
11. positive, negative, replay, fork, expiry, privacy, and recovery fixtures.

### Coordinator-independent recovery

Item 6's recovery definition has a mandatory floor. Every executable
profile MUST define, and fixture, a coordinator-independent terminal path:
both parties reach a correct terminal state — completed, refunded, or
unilaterally exited — using only their persisted signed records, a direct
or relay-agnostic counterparty channel, and the external rail, with no
market relay handler, provider API, or other coordinator alive. Where the
rail supports it, the profile requires a pre-signed or pre-derived
unilateral-exit artifact to be produced and retained before funds move.
The Boltz and Satora coordinator outages of 2026-08 are the motivating
record: in both, the settlement rail kept working while the coordination
service did not.

### Profile field vocabulary

Profiles SHOULD follow these field laws unless they record a reason not
to:

- an asset's identity is a collision-resistant asset identifier; a market
  is identified by its asset-ID pair. Tickers and display names are
  unverified labels and MUST NOT be used for matching, grouping, or
  pricing;
- amounts are canonical decimal strings of the asset's atomic unit
  (`^(0|[1-9][0-9]*)$` bounded by the profile), never JSON numbers, which
  lose integer precision past 2^53;
- a direction or side a provider does not serve is disabled explicitly,
  never implied by omission;
- an advertised fee or spread is a fill promise constrained by the
  profile's evidence rules, not a proven fact; and
- when an external price feed is part of quoted terms, the Quote pins the
  exact feed URL and value-extraction rule both parties use. Pricing from
  a substitute feed is a term violation, and a feed never acquires
  settlement authority.

Custody is described by independent dimensions, not one reassuring score. At
minimum profiles disclose `funds_control`, `execution_control`,
`settlement_authority`, `reversibility`, `recourse`, and
`credential_exposure`. A provider that never receives unilateral spend
authority may declare a noncustodial path; a relay can validate, route,
reserve provider-signed capacity, run timers, and verify evidence while
remaining noncustodial.

Expected focused profiles include:

- **MKT-SWP** — Boltz-class submarine, reverse, and chain swaps with scripts,
  trees, invoices, timeouts, claim/refund paths, and chain/Lightning proof.
  The draft SHOULD reserve cross-chain contract-leg vocabulary (chain
  identifier, contract address, token asset ID, claim/refund signature
  mode, confirmation policy as a quoted term) so an EVM-leg extension can
  land without a breaking revision;
- **MKT-P2P** — NIP-69-compatible peer discovery and profile-defined escrow,
  reputation, fiat, or cash rails;
- **MKT-PFI** — tbDEX-style provider discovery, qualification, credentials,
  RFQ/Quote/Order, fiat/crypto settlement, and regulated recourse;
- **MKT-MINT** — Cashu/Fedimint discovery and exchange using NIP-87 and the
  mint/federation's own proof and custody model; and
- **MKT-LSP** — channel, lease, inbound-liquidity, and service-provider
  negotiation with exact capacity and finality evidence.

An **MKT-RISK** profile is valid only with an actual guarantor/underwriter,
reserves, coverage, exclusions, claims, adjudication, and settlement. A model
score or receipt count is not insurance.

One market shape is deliberately not covered by the base: the maker-funded
standing offer that any counterparty may fill without negotiation, with
terms enforced by a covenant or equivalent rail construct (the Arkade
intent/solver model). That shape needs no RFQ/Quote exchange because the
rail carries the term enforcement this base provides through signed
records. Whether it becomes an MKT-INTENT profile, folds into MKT-P2P, or
stays rail-native is an open profile-drafting decision; a profile claiming
it MUST NOT relabel a standing offer as a Quote or an anonymous fill as an
Order.

DS, LBR, compute, training, and future markets keep their own domain
primitives; spine reuse must not erase distinct verification or settlement.

## Legacy translation

No NIP-90, NIP-69, NIP-99, tbDEX, Boltz, wallet, mint, or provider record is
silently upgraded into NIP-MKT.

A translator names source protocol/revision, emits a deterministic mapping
version, preserves source digest, lists every dropped/defaulted/ambiguous
field, and fails closed when it cannot represent a source authority or state.
A projection does not acquire target signature, reservation, custody,
execution, or settlement semantics.

## Relay and provider conformance

A relay may implement transport only or add noncustodial handlers. An
advertised handler MUST:

- validate base shape and bounds before persistence or fan-out;
- return `OK` only after durable commit for stored public/gift-wrap events;
- require NIP-42 for recipient reads and enforce NIP-17 `kind:1059` gating
  for filters, ID queries, search, and live fan-out;
- exclude wrapper contents from search and generic projections;
- rate-limit discovery per IP/pubkey and wraps per IP, sender, recipient, and
  session;
- reject idempotency conflicts and surface reservation/status forks;
- keep ephemeral kinds out of storage and never label `396xx` ephemeral or
  regular; and
- advertise only configured executable profile revisions.

A noncustodial handler MAY route RFQs, apply policy, reserve provider-signed
capacity, run timers, verify public evidence, and bridge compatibility APIs.
It MUST NOT hold balances, seeds, private keys, NWC strings, node macaroons,
bank credentials, unreleased preimages, signing nonces, bearer credentials,
or private claim/refund keys. It does not claim final settlement without the
profile's actual authority.

Conformance requires fixtures for every base kind and profile, including
malformed events, duplicate JSON keys, unsupported profile/version, changed
bytes under one `d`, rewrapped replay, quote supersession, double reservation,
expired order, sequence gap/fork, unauthorized state/cancel/close,
wrapper/inner signer mismatch, bare-private publication, evidence mismatch,
recovery loss, and settlement overclaim.

## Security considerations

- **Metadata:** bare private records expose parties, kinds, time, amounts, and
  routes. Use NIP-59, small inbox lists, randomized wrapper times,
  recipient-gated relays, and separate wraps. Relays still see network data.
- **Gift-wrap spam:** random wrapper keys weaken reputation controls. Apply
  NIP-59 proof-of-work or access policy and per-recipient quotas.
- **Phantom capacity:** signatures make contradictory reservations
  attributable, not impossible. Profiles need reserve accounting and
  independent evidence.
- **Addressable replacement:** use unique `d`, pin exact IDs, reject reuse,
  and retain participant copies. Only public heads intentionally replace.
- **Credentials:** keep PII, bank data, account metadata, and bearer material
  in the smallest audience or an external presentation.
- **Key compromise:** NIP-09 cannot erase copies or undo settlement. Profiles
  define rotation, relationship reevaluation, outstanding-order recovery, and
  when the external rail controls funds.
- **Identity laundering:** profiles, handlers, labels, and owner attestations
  are signer claims with separate authorities. Interfaces identify each
  asserter.
- **Settlement overclaim:** `funding_observed`, `completed`, `refunded`, and
  public receipts do not become `verified` or `settled` without exact proof.
- **Automation:** agents stay within capability, amount, expiry, and
  generation. Automation cannot bypass wallet, guardian, credential,
  reservation, custody, or settlement gates.
- **Irreversibility:** Cancel, Close, expiry, deletion, or shutdown cannot
  reverse an external irreversible step.
- **Profile supply chain:** descriptors are data, not code. Pin digests,
  allowlist authorities, sandbox adapters, and display unsupported revisions.

## Implementations

This section is informative and records interoperability evidence, not
authority.

- **Immortal relay** (<https://github.com/OpenAgentsInc/immortal>) —
  implements the base as of 2026-08-04: public discovery-head validation
  (`39600-39603`), immutable-by-contract admission for `39604-39609`,
  bare-private-publication rejection, recipient-gated wrapped transport on
  every read surface, rate limits, a fixture corpus covering the relay and
  client conformance cases, and a deterministic machine-readable contract
  export (`immortal contract`) for SDK generation. It repeated and
  recorded the `39600-39699` collision review before admitting the kinds,
  advertises the nonnumeric `nip-mkt` extension only under authenticated
  recipient transport, and implements **no executable profile** — base
  discovery and transport only. Its server contract and adoption record
  live in that repository under `docs/protocol/`.

## References

- NIP-01, NIP-09, NIP-17, NIP-32, NIP-39, NIP-40, NIP-42, NIP-44,
  NIP-46, NIP-47, NIP-51, NIP-57, NIP-59, NIP-61, NIP-65, NIP-66,
  NIP-69, NIP-85, NIP-87, NIP-89, NIP-90, NIP-99
- Block NIP-OA, NIP-AA, NIP-AE, NIP-AO, NIP-AM, NIP-DV, NIP-IA
- OpenAgents NIP-SKL, NIP-SA, NIP-AC, NIP-DS, NIP-LBR, NIP-TRN,
  NIP-AD, NIP-EV, NIP-OC
- [Nostr registry of event kinds](https://github.com/nostr-protocol/registry-of-kinds)
- [`NIP90-MIGRATION.md`](NIP90-MIGRATION.md)
- [`MKT-SWP.md`](MKT-SWP.md)
- [`MKT-PFI.md`](MKT-PFI.md)
- `docs/teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md`
- `docs/teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md`
- `docs/teardowns/2026-08-04-satora-lendaswap-outage-teardown.md`
- `docs/teardowns/2026-08-04-ark-solver-mostro-cashu-rails-teardown.md`

## Changelog

**v0.2 (2026-08-04)**

- Recorded the fresh three-lane and registry review for the first focused
  profiles.
- Allocated private immutable `kind:39610` to the MKT-SWP Swap Contract and
  public no-PII `kind:39630` to the MKT-PFI Qualification Policy; every other
  kind in `39610-39699` remains unallocated pending separately reviewed
  profiles.
- Linked the complete MKT-SWP and MKT-PFI profile contracts.

**v0.1 (2026-08-04)**

- No change to base kinds, tag grammar, content envelope, transport, or
  relay admission rules; the implemented base is unaffected.
- Profile contract: added the mandatory coordinator-independent recovery
  floor (motivated by the Boltz and Satora coordinator outages) and the
  profile field-vocabulary laws (asset-ID pair identity, decimal-string
  amounts, explicit side-disable, fee-as-promise, pinned price feeds).
- Expected profiles: MKT-SWP notes reserved cross-chain contract-leg
  vocabulary; recorded the maker-funded intent-market shape as an open
  profile-drafting decision outside the base.
- Added the informative Implementations section (Immortal relay base) and
  the Satora and market-rails teardown references; corrected the tbDEX
  teardown filename.

**v0 (2026-08-04)**

- Initial draft: collision-reviewed `39600-39699` reservation, public
  discovery, independently signed private negotiation, NIP-59 transport,
  idempotency, reservation, sequencing, closeout, profiles, and noncustodial
  relay boundaries.
