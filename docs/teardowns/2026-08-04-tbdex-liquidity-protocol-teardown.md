# tbDEX teardown — a provider-neutral liquidity protocol, translated to Nostr

- Date: 2026-08-04
- Lane: Fast Follow research / protocol teardown
- Disposition: high-value architectural donor for a heterogeneous liquidity
  market; archived upstream implementation, so **adapt the protocol laws, not
  the runtime dependency**
- Primary local source: the `~/work/projects/tbd/` reference lane
  (32 TBD54566975 clones under `projects/tbd/repos/`, synced 2026-08-04;
  upstream has archived most of them, so the clones are frozen reference
  evidence)
- Whitepaper: `projects/tbd/repos/tbdex-whitepaper/` at
  `62c466774f36671ce89649b9507f6802a3b60475` (moved from the old
  `projects/repos/` location on 2026-08-04)
- Rust SDK pin: `projects/tbd/repos/tbdex-rs/` at `c3d4985` (v4.0.0)
- Companion synthesis:
  [Boltz ecosystem and Nostr rebuild](./2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md)

This is a read-only teardown of the local Apache-2.0 tbDEX v0.2 whitepaper,
its diagrams and source TeX, plus the public protocol and SDK repositories.
No provider was contacted, no credential was presented, and no quote, order,
payment, or settlement was attempted. Section 7 maps the full local lane to
concrete harvest targets.

## Summary

tbDEX's durable idea is not a decentralized exchange in the on-chain AMM
sense. It is a **common negotiation protocol for many independent liquidity
providers** at the boundary where cryptography cannot remove social trust:
fiat rails, physical goods, regulated counterparties, reversible payments,
and identity-dependent risk. A wallet discovers providers, broadcasts or
sends a request for quote, compares signed bids, reveals only the credentials
needed by the selected counterparty, places an order, and follows signed
status messages through closeout.

The whitepaper deliberately rejects a federation, governance token, universal
trust root, and protocol-wide anonymity rule. Different providers may offer
different prices, settlement methods, credential requirements, and custody
models. The user chooses whom to trust. That is the right market shape.

The wrong conclusion would be that a common protocol makes every route
trustless. It does not. tbDEX's own off-ramp example exposes the hard problem:
once one side is reversible fiat and the other is final cryptocurrency, no
generic smart contract makes the whole exchange atomic. Oracles, bank
monitoring, escrow, bonds, reserves, arbitration, insurance, or legal recourse
can redistribute that risk; none erase it.

The OpenAgents synthesis is therefore:

> Use Nostr and Immortal as a noncustodial identity, discovery, negotiation,
> and receipt fabric. Use tbDEX's provider/Offering/RFQ/Quote/Order/Status/Close
> vocabulary across many custody levels. For each quoted route, bind the
> strongest settlement guarantee the underlying rails can actually provide.
> Use Boltz-style scripts and preimages where atomicity is possible; expose
> residual counterparty, custody, reversibility, and dispute risk everywhere
> else.

### Product name and architecture name

The user-facing product is the **OpenAgents Liquidity Market**, one of
[Episode 213's](../transcripts/213.md) five interlocking Agent Markets
(compute, data, labor, liquidity, risk). The broader thing being built
underneath it is a **Nostr-native negotiated-market fabric**: a common protocol
by which heterogeneous providers advertise, quote, accept, progress, prove,
and close bilateral transactions. The first technical deployment is a
**noncustodial Bitcoin liquidity network** spanning atomic swaps, Lightning
liquidity, P2P/PFI routes, and mint/federation gateways.

That distinction matters. “Liquidity Market” is the right product name.
NIP-MKT is the reusable protocol family and can later support common
negotiation laws in other Agent Markets without pretending compute, data,
labor, liquidity, and risk are the same state machine. “Exchange” is too
narrow and “liquidity pool” falsely suggests pooled custody. [source]
[proposal]

## 1. Sources, status, and evidence

### 1.1 Local whitepaper

| Artifact | Identity | Evidence |
| --- | --- | --- |
| Repository | `~/work/projects/tbd/repos/tbdex-whitepaper/` | Local audited tree |
| Commit | `62c466774f36671ce89649b9507f6802a3b60475` | Exact source pin |
| Whitepaper | `whitepaper.pdf`, v0.2, 16 pages, SHA-256 `2144f4c7a764138d7a03a2af4ce9e99609a6cff7428dcfd074561fd4ff2386fc` | Rendered and visually inspected page by page |
| Source | `whitepaper.tex`, SHA-256 `976b0b94d91af12724d195569e8f0dba29eb66f0d0a3e8d8b17d81376223ac67` | Full text and diagram source |
| License | Apache-2.0 | Ideas and compatible source may be adapted with required notices |

The whitepaper is a conceptual design, not a security proof or production
specification. Its own future-work section calls for a protocol spec, SDK,
and provider node.

### 1.2 What was subsequently implemented

The later public work was materially more than a paper:

- [`TBD54566975/tbdex`](https://github.com/TBD54566975/tbdex) contains draft
  protocol and HTTP API specifications, hosted JSON schemas, shared test
  vectors, and the concrete lifecycle
  `Offering → RFQ → Quote → Order → OrderStatus → Close`.
- [`TBD54566975/tbdex-js`](https://github.com/TBD54566975/tbdex-js) published
  protocol, HTTP client, and configurable HTTP server packages. The server
  exposed handlers for offerings, exchanges, RFQs, orders, and closes.
- The protocol repository records TypeScript and Kotlin client/server support
  and client-side Swift support. It also records an example PFI and wallet.
- [`TBD54566975/tbdex-rs`](https://github.com/TBD54566975/tbdex-rs) added a
  Rust core, Kotlin bindings, Maven distribution, and signed/verified
  Offering, Balance, RFQ, Quote, Order, OrderInstructions, OrderStatus, Close,
  and Cancel models. This is stronger implementation evidence than the
  whitepaper's original promised SDK.

Both the protocol and TypeScript repositories were archived on 2024-12-12;
the last listed JavaScript protocol release is 2.2.1 from 2024-09-03. This
means tbDEX demonstrated a serious schema-and-SDK design, but it is not a
maintained foundation we should put on the critical path in 2026. [public]

### 1.3 Historical arc and adoption limit

| Period | What the primary record supports |
| --- | --- |
| 2021 | The whitepaper launched the fiat/crypto bridge thesis: social trust cannot be eliminated, so counterparties should negotiate identity, risk, and price without a federation or governance token. |
| 2022–2024 | The work matured into draft protocol/HTTP specifications, test vectors, TypeScript, Kotlin, Swift, and Rust/Kotlin SDK surfaces, example clients/providers, and published packages. |
| May 2024 | [Chipper Cash announced](https://www.chippercash.com/blog/chipper-cash-and-tbd-announce-strategic-partnership) an integration and partnership focused on African cross-border payments, remittances, DIDs, and verifiable credentials. This proves a real named integration commitment, not network-wide production volume. |
| December 2024 | The core protocol, TypeScript, and Rust repositories were archived. |
| 2025–2026 | [TBD's site](https://tbd.website/) states that the business was wound down and its remaining projects archived or transitioned to the open-source community. |

The accurate verdict is: **implemented as a substantial coordination protocol
and SDK family, but not realized as the universal, broadly adopted liquidity
network envisioned by the paper.** The protocol defined negotiation and
messages; providers and external rails still moved money. Public partnership
announcements do not establish durable production volume, liquidity depth, or
settlement reliability. [public] [limitation]

The likely adoption constraints are structurally relevant even where the
available sources cannot rank them: multi-sided network bootstrapping required
wallets, providers, credential issuers, and users at once; DID/VC integration
was heavy; direct stablecoin/payment APIs and improving Lightning/LSP tooling
offered narrower paths; and the sponsoring business ultimately ended. The
lesson is to start with a working corridor and existing liquidity ecosystem,
not a universal empty network. [inferred]

### 1.4 Evidence labels

- **`[source]`** — local whitepaper or public repository content
- **`[public]`** — current public project metadata or documentation
- **`[inferred]`** — architecture derived from multiple observations
- **`[proposal]`** — OpenAgents/Nostr design, not shipped behavior
- **`[limitation]`** — a claim the available evidence cannot establish

## 2. The original architecture

### 2.1 Participants

| Participant | tbDEX role | Nostr translation |
| --- | --- | --- |
| Wallet | User agent, policy engine, credential holder, quote selector | Local-first market router with its own Nostr keys and rail adapters |
| PFI | Independent liquidity provider | Provider daemon identified by a Nostr service key; may be atomic LP, mint, federation gateway, escrow coordinator, or regulated PFI |
| Credential issuer | Attests facts the PFI needs to price or admit risk | Independent issuer; signed assertion or selectively disclosed VC, chosen by wallet/provider policy |
| Decentralized Web Node | Message and semantic-data transport | Nostr relay set for public catalog and encrypted inboxes; local storage for secrets |
| Smart contract / settlement rail | Binds or proves a leg | Bitcoin, Lightning, Liquid, Ark, Cashu, Fedimint, bank, stablecoin, escrow, or custodian adapter |

The separation is important. The messaging network does not become the PFI,
the wallet does not become the credential issuer, and identity does not become
settlement.

### 2.2 Discovery without a global directory

The paper proposes several overlapping discovery mechanisms:

1. a person's own curated provider list;
2. a wallet publisher's list;
3. N-degree traversal through lists maintained by known parties;
4. crawling provider identifiers and their service endpoints.

There is deliberately no global canonical list. The wallet applies its own
trust policy. On Nostr this maps naturally to small NIP-51 provider lists,
NIP-65 relay lists, NIP-87 mint/federation announcements, NIP-39 external
identity proofs, NIP-85 assertion-provider choices, and private owner policy.
No relay or OpenAgents service should publish a universal provider rank.

### 2.3 Public request, private negotiation

The paper's ASK/BID model became the later RFQ/Quote lifecycle:

```text
wallet                    provider candidates                 selected provider
  | pull signed Offerings          |                                  |
  | send RFQ to each candidate ---->                                  |
  | <---------------- signed Quote |                                  |
  | compare price + trust + risk   |                                  |
  | --------------------------- Order -------------------------------> |
  | <------------------------- OrderStatus --------------------------- |
  | ---------------------------- Close ------------------------------> |
```

An RFQ identifies offered and requested assets, amounts, settlement schemes,
and constraints. A Quote binds a provider, price/cost, settlement amount and
time, expiry, and signature. The later protocol's explicit OrderStatus and
Close messages are a valuable improvement over treating acceptance as
completion.

The Nostr form should **not** be one public RFQ sprayed with account details.
Pull public Offerings; shortlist providers locally; send pairwise encrypted
RFQs; receive provider-signed expiring Quotes; reveal credentials only after
selection or when required to produce a meaningful quote.

### 2.4 Credentials as price and risk inputs

tbDEX does not require one identity system or a single KYC regime. It lets a
provider state which credential types it accepts, while a wallet chooses
which issuers it trusts. Credentials are not supposed to be included in the
public ASK. A user may obtain a better price or faster settlement by
presenting more evidence, but disclosure remains a conscious trade.

This is the right structural lesson, with two cautions:

- a Nostr pubkey proves control of a key, not a legal identity, solvency,
  authorization, or clean funds;
- public labels and generic reputation events are not a safe container for
  passports, bank details, sanctions-screening evidence, or reusable bearer
  credentials.

Use NIP-32 for namespaced public claims, NIP-39 for external account-control
proofs, and NIP-85 for wallet-selected computational assertions. Use encrypted
references or selective presentations of W3C Verifiable Credentials when a
regulated counterparty needs stronger facts. Bind every presentation to the
RFQ or Order nonce, audience, purpose, and expiry to prevent replay. Store no
PII on public relays.

## 3. The strongest laws to harvest

| Law | Why it matters |
| --- | --- |
| Providers are permissionless at protocol level | No operator controls who may offer liquidity; clients control whom they trust |
| Trust is selected, not declared away | Fiat and physical settlement retain risk; the protocol makes selection and evidence portable |
| Offerings are pullable and comparable | A wallet can compare providers without surrendering execution authority to an aggregator |
| Negotiation has explicit messages | RFQ, Quote, Order, Status, and Close are typed, signed, and replay-resistant |
| Quote first, credentials later | Minimize unnecessary identity disclosure and provider-side data exhaust |
| Credentials affect terms | Providers may price real risk without imposing one global admission policy |
| No governance token or federation is required | Interoperability comes from wire compatibility and client choice |
| Common test vectors matter | Independent implementations need shared fixtures, not prose-only compatibility |
| Close is distinct from payment | A message receipt, provider status, and settlement finality are separate facts |

## 4. Where the design is incomplete

### 4.1 The fiat atomicity gap

The on-ramp sequence can have a provider pre-fund a contract before the user
sends fiat, proving some ability to settle. The off-ramp sequence is harder:
once the user locks cryptocurrency and the provider receives it, sending fiat
is an external promise. The paper proposes bank monitoring, oracles, and
watchtowers as future work, but these detect or attest; they do not make a
reversible bank payment consensus-atomic with Bitcoin.

Our protocol must classify the actual guarantee:

- **atomic** — one secret or consensus condition controls all legs;
- **escrowed** — a named party or threshold controls release;
- **reserved** — capacity is locked, but settlement remains sequential;
- **guaranteed** — a bond, reserve, insurer, or legal obligation covers a
  defined failure;
- **best effort** — counterparty promise with evidence and dispute path only.

Anything else is marketing ambiguity.

### 4.2 Solvency and inventory proofs

A signed capacity claim is not proof of unencumbered inventory. Proof of key
control is not proof the same funds were not promised to ten other takers.
Meaningful liquidity requires quote-level reservation with an expiry and a
rail-specific proof where possible: a funded swap output, hold invoice,
channel/JIT commitment, mint quote, federation contract, escrow deposit, or
custodian reservation reference. Historical success and third-party
attestations can influence policy, but cannot replace present reservation.

### 4.3 Composed routes are not automatically atomic

A pathfinder may compose Lightning → Liquid → fiat, or Cashu → Lightning →
Bitcoin. Atomic individual legs do not make the entire path atomic. A
multi-leg route must either share an enforceable secret/timelock construction,
be fully pre-funded, carry an explicit guarantee for the intermediate
exposure, or tell the user exactly which sequential risk window remains.

### 4.4 Privacy limits

Pairwise encryption protects content from relays, but not necessarily IP,
timing, message-size, provider, or counterpart metadata. NIP-44 also lacks
forward secrecy and post-compromise security. Use ephemeral per-relationship
keys, small recipient inbox relay sets, padded messages, short retention,
Tor where appropriate, and a separate high-risk credential channel when the
threat model demands it.

## 5. A unified custody and trust gradient

The common protocol should support all of these without pretending they are
equivalent:

| Class | Example provider | Who can withhold or lose funds? | Required quote disclosure |
| --- | --- | --- | --- |
| A0: atomic self-custody | Boltz-style submarine/chain LP | Neither party after correct lock; timeout and chain risks remain | Script template, hash binding, timelocks, confirmation policy, fees |
| A1: coordinated hold/escrow | Mostro-style Lightning hold-invoice coordinator | Coordinator/arbiter controls settlement decision but does not receive spendable user balance in the ordinary path | Hold terms, solver set, bond, dispute and timeout rules |
| A2: federated custody | Fedimint and its Lightning gateways | Threshold guardian set and gateway for their respective legs | Federation ID, threshold/governance, gateway, modules, fees, withdrawal path |
| A3: mint custody | Cashu mint | Mint operator can fail, censor redemption, or become insolvent | Mint key, NUTs, reserves/attestations, redemption rails, fee and keyset policy |
| A4: regulated PFI | Bank, exchange, stablecoin issuer, payment processor | Provider and external settlement institutions | Legal identity, jurisdiction, credentials, reversibility, custody, dispute and recourse |
| A5: qualified custodian/prime service | Custodian or broker | Named custodian under its legal/operational regime | Segregation, authorization, withdrawal SLA, insurance and insolvency treatment |

The class is not a quality score. A regulated PFI can be appropriate for a
fiat corridor; an atomic LP can be inappropriate if the destination rail is
not useful to the taker. The wallet optimizes across amount, rate, latency,
privacy, custody, credential cost, finality, and recourse.

## 6. Nostr/Immortal translation

### 6.1 Relay law

Relays may be fully noncustodial. They accept, index, and deliver signed
events. They must never hold wallet seeds, NWC secrets, preimages, refund
keys, provider inventory, user balances, fiat accounts, or authority to
declare payment final. A relay's `OK`, `EOSE`, or retained event proves relay
behavior only.

One Immortal deployment is useful infrastructure, not decentralization by
itself. The target topology is:

- replicated public Offering/catalog events on several operator-independent
  relays;
- NIP-65 writer/read relay selection and NIP-66 liveness observations;
- participant-selected NIP-17/NIP-44/NIP-59 inbox relays for private RFQs;
- optional authenticated, high-integrity relays for regulated or
  organization-scoped providers;
- local wallet persistence for secrets and authoritative session recovery.

### 6.2 Reuse official NIPs where they fit

| NIP | Use |
| --- | --- |
| NIP-01 / 40 / 70 | Signed events, expiry, and protected publication |
| NIP-17 / 44 / 59 | Pairwise private RFQ, Quote, Order, Status, credential references, and rescue traffic |
| NIP-32 / 39 / 73 / 85 | Namespaced claims, external identity proofs, identifier binding, and wallet-selected assertions |
| NIP-43 / 65 / 66 / 67 | Relay admission metadata, relay sets, liveness, and complete pagination semantics |
| NIP-47 | Wallet-to-wallet-service operations without embedding node credentials in a market client |
| NIP-51 | Portable curated provider, issuer, monitor, and arbiter lists |
| NIP-69 | Existing P2P fiat-order vocabulary and ecosystem interop precedent |
| NIP-87 | Cashu mint and Fedimint discovery; do not duplicate it in a new catalog |
| NIP-89 | Discover clients that can render or handle the new market events |
| NIP-99 | Human-facing broad service listings; not the executable quote wire |

NIP-90 is explicitly unrecommended upstream and has become too generic. Do
not place a financial state machine in the DVM job ranges. Define a focused
market microstandard with exact invariants and test vectors.

### 6.3 Implement all three pinned NIP lanes; compose them by role

The Immortal repository pins official, Block, and OpenAgents lanes. The owner
has directed Immortal to implement every pinned NIP, plus new focused NIPs
needed to absorb the noncustodial Boltz/tbDEX coordination surface. This does
not mean forcing every NIP into every market event. It means implementing the
role each text actually defines—relay, client, operator, provider, or
executor—and composing only the relevant roles into the Liquidity Market.
Current non-advertisement is a conformance fact, not a scope ceiling.

**Official lane:** use the NIPs above as transport and discovery primitives.

**Block lane:** NIP-OA can bind an agent key to an owner; NIP-AA can support
explicit relay admission; NIP-AP can describe public provider-agent personas;
NIP-AO carries ephemeral private operations; NIP-AM carries durable bounded
metrics; NIP-AE/ER can keep encrypted provider memory and reminders; NIP-RS
tracks client read state; NIP-IA can archive public catalog history. NIP-MP,
GS, CW, DV, and WP remain platform/UX concerns, not settlement primitives.

**OpenAgents lane:** NIP-SKL can describe and attest rail-adapter skills;
NIP-SA and AD/AS/AV can govern provider agents; NIP-WI/WK can admit an
owner-authorized treasury action; NIP-EV carries independently verifiable
evidence; NIP-OC closes an accepted outcome; NIP-AT/GB/AL/TP can deliver
private recovery alerts; NIP-HP can describe host capacity; NIP-PP can track
claim state. NIP-AC credit and NIP-LBR labor are optional adjacent markets,
not the core liquidity wire.

### 6.4 The one new microstandard worth drafting

**Status 2026-08-04: drafted.** NIP-MKT v0 now exists at
[`docs/nips/MKT.md`](../nips/MKT.md) with the collision-reviewed kind block
`39600-39609` and `39610-39699` reserved for profiles, and it is mirrored
into the Immortal `nips/openagents/` lane. The relay implementation program
is Immortal issues #3-#9 (M10 base, M11 contract export, local dev env); the
generated TypeScript SDK is openagents#9309, the web demo is
openagents#9310, and the Omega market panel is omega#244. The shape below is
what was proposed and matches what was drafted: one focused **NIP-MKT:
Negotiated Markets** base vocabulary, then narrow profiles rather than one
giant exchange NIP:

```text
ProviderProfile       public, addressable, slow-changing identity/capabilities
Offering              public, addressable, expiring pair + rail + policy envelope
RFQ                   private, idempotent, amount + constraints + credential hints
Quote                 private, signed, expiring, reserved terms + guarantee class
Order                 private, signed acceptance of exactly one quote digest
OrderStatus           private by default, monotonic sequence + rail evidence refs
Close                 private, terminal reason + settlement/evidence refs
PublicReceipt         optional, redacted, consented, independently verifiable
```

Profiles then define mandatory fields and state machines:

- **MKT-SWP** — Boltz-class atomic swaps;
- **MKT-P2P** — Mostro/NIP-69-compatible fiat trades and disputes;
- **MKT-PFI** — tbDEX-class credentialed on/off ramps;
- **MKT-MINT** — Cashu/Fedimint gateway and redemption quotes;
- **MKT-LSP** — Lightning channel/JIT liquidity aligned with bLIP-50/51/52;
- later **MKT-RISK** — bonds, guarantees, and insurance, only after a real
  underwriter and claims authority exist.

The base standard should own correlation, idempotency, signatures, expiry,
quote reservation, cancellation, sequencing, terminal states, error codes,
privacy, and evidence references. Each profile owns rail semantics. Asset and
network identifiers should reuse existing registries rather than create a
Nostr-specific ticker ontology.

## 7. Harvest map: the full TBD lane, relative to Boltz

The `projects/tbd/` lane now holds the complete relevant TBD54566975 set —
32 clones spanning protocol, SDKs, PFI exemplars, wallets, and
discovery/compliance. Upstream archived most of them, which makes the lane
stable harvest material rather than a moving dependency. This section says
what each group contributes, where it lands in our build, and what we
deliberately leave behind.

The division of labor between the two donors:

> **Boltz is the settlement donor. tbDEX is the negotiation donor.** From
> Boltz we harvest execution physics — swap lifecycles, client-side
> verification law, HTLC/Taproot script and MuSig2 claim/refund structure,
> unilateral exit, hold-invoice semantics — which land in the MKT-SWP
> profile, the provider daemon design, and the adversarial regtest lab.
> From tbDEX we harvest market grammar and conformance assets — the message
> vocabulary, JSON schemas, parse vectors, provider/wallet role shapes,
> discovery precedent, and credential flow — which land in the NIP-MKT base,
> its fixture corpus, the generated SDK, and the later MKT-PFI profile.
> Neither donor contributes running code to the critical path; both
> contribute laws, shapes, and test material.

### 7.1 Per-group harvest

| Lane group | Repos | Harvest | Lands in |
| --- | --- | --- | --- |
| Protocol + docs | `tbdex`, `tbdex-whitepaper`, `tbdex-docs`, `tbdex-rest-api` | The message lifecycle and field vocabulary — including two messages our draft should keep in view: `Cancel` as a first-class message and `OrderInstructions` as a separate payment-direction step distinct from `Quote`. The hosted `json-schemas/*.schema.json` (balance, cancel, close, offering, order, orderinstructions, orderstatus, quote, rfq) and `hosted/test-vectors/protocol/vectors/*.json` parse vectors are the highest-value single asset: a frozen, Apache-2.0 conformance corpus for every message type. `tbdex-rest-api` documents the HTTP binding a compatibility facade would mimic. | NIP-MKT fixture translation (Immortal #7, exported corpus in #8); profile field checklists; optional compatibility facade design |
| Language SDKs | `tbdex-rs` (pinned v4.0.0), `tbdex-js`, `tbdex-kt`, `tbdex-go`, `tbdex-dart`, `tbdex-swift`, `tbdex-rb` | `tbdex-rs` is the reference: its `crates/*/src/messages` and `resources` modules show mature field naming, per-message validation, and a signature module, with bound TypeScript tests replaying the shared vectors. Harvest the model shapes, validation split (structure vs. signature vs. state), and the pattern of one canonical core with thin language bindings — the same pattern as our contract-generated SDK. | `packages/nip-mkt` codegen design (openagents#9309); Immortal domain validation structure (#3-#5) |
| Privacy mechanics | `tbdex` RFQ spec + `parse-rfq-omit-private-data.json` vector | tbDEX RFQs split private data out of the signed message and bind it by hash, so a stored RFQ can be disclosed without leaking credentials or account details. This is the same disclosure law as NIP-MKT's independently signed inner record inside a gift wrap; the omit-private-data vector is a ready-made negative fixture pattern for our privacy envelope tests. | NIP-MKT privacy-envelope fixtures; MKT-PFI credential-reference design |
| PFI exemplars | `tbdex-pfi-exemplar` (tombstone → `tbd-examples`), `example-pfi-aud-usd-tbdex`, `pfi-guide-example`, `pfi-providers-data`, `hackathon-mock-pfis`, `workshop-mock-pfis` | The provider-daemon role shape: offering publication, exchange webhook handling, quote issuance, order progression, settlement callbacks. The mock fleets show how small a credible test provider can be — directly the shape of Immortal's `dev-market-seed.sh` provider actor and the web demo's provider driver. `pfi-providers-data` shows a curated provider directory as plain data, matching our no-global-directory law (NIP-51 lists, not a registry service). | Immortal #9 seeded actors; openagents#9310 demo provider driver |
| Wallets + apps | `didpay` (Flutter), `tbdex-DIDPay-sample`, `tbdex-example`, `tbdex-example-android`, `tbdex-example-ios`, `tbdex-ussd`, `workshop-tbdex-wallet`, `workshop-tbdex-abc-wallet`, `workshop-tbdex-abc-vc-issuer` | The wallet-side UX sequence worth copying: pick corridor → compare pulled offerings → RFQ → ranked quote comparison → explicit acceptance → status timeline → closeout, with credentials requested only when a selected provider needs them. `tbdex-ussd` is a useful boundary case: the protocol survived a USSD text menu, evidence the negotiation grammar is thin enough for constrained clients. | openagents#9310 web demo flows; omega#244 panel flows |
| Discovery + compliance | `tbdex-discovery-nostr`, `known-customer-credential` (+ `kcc-js`, `kcc-rs`, `kcc-prototype-exemplar`), `trust-framework` | `tbdex-discovery-nostr` is Block's own prototype of exactly our discovery design: PFIs advertise currency pairs as Nostr events on public relays, customers subscribe and then negotiate over tbDEX — precedent, from the protocol's authors, that Nostr is the right discovery fabric where their DID-based directory was too heavy. Harvest it as validation and as a checklist of what a minimal offering advertisement needs. KCC defines the Known Customer Credential issuance flow (IDV vendor integration, what PII the PFI holds vs. the vendor) — the concrete credential input for a future MKT-PFI profile. `trust-framework` shows compliance vocabulary a regulated provider will expect. | NIP-MKT `39600`/`39601` head design (already drafted); MKT-PFI profile inputs; provider admission policy design |

### 7.2 What we deliberately do not harvest

- **The DID/VC identity core.** `did:dht`, DID resolution, and the Web5
  stack are the weight that limited adoption. Nostr keys plus NIP-32/39/85
  assertions carry discovery and reputation; W3C VC selective presentation
  appears only at the MKT-PFI boundary where a regulated counterparty
  requires it, referenced and encrypted, never on public relays.
- **JWS/JOSE signing.** Our wire is NIP-01 Schnorr signatures over canonical
  event bytes. Vector translation maps tbDEX's signature assertions to
  event-signature assertions rather than porting JOSE.
- **HTTP as the primary wire.** tbDEX ran provider REST endpoints; our
  primary wire is relay events with gift-wrapped private records. An HTTP
  compatibility facade is an optional later adapter, not the foundation.
- **DWN transport and the packaging machinery.** Decentralized Web Nodes,
  jitpack/Maven distribution, and the multi-language binding toolchain are
  replaced by the relay fabric and the contract-generated SDK process.
- **Running any archived code on the critical path.** Everything harvested
  arrives as laws, shapes, schemas, and vectors re-expressed in our own
  fixtures; Apache-2.0 permits direct adaptation where a schema or vector is
  copied, with notices preserved.

### 7.3 Boltz harvest, for contrast

The [companion Boltz teardown](./2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md)
covers the other donor in depth; the short form of what it contributes,
stated here so the two harvests compose without overlap:

- explicit submarine, reverse, and chain-swap lifecycles with their exact
  state transitions and timeout ladders → MKT-SWP state machine;
- the funding law — the client verifies lock script or Taproot tree,
  amounts, payment hash, timelocks, and claim/refund paths before money
  moves → wallet-side verification requirements in every atomic profile;
- MuSig2 cooperative key-path claims with script-path unilateral exit →
  the happy-path/exit split MKT-SWP must preserve;
- hold invoices and preimage coupling → the binding between legs;
- the operational surface (30-repo service map, incident posture) → what a
  provider daemon and its recovery duties actually contain.

Where tbDEX tells providers and wallets **how to talk**, Boltz tells the
atomic subset of them **how to be sure**. NIP-MKT base carries the first;
the MKT-SWP profile carries the second; the custody gradient in section 5
places every other route honestly between them.

## 8. Liquidity bridges into real ecosystems

| Ecosystem | Bridge | Why it matters |
| --- | --- | --- |
| Bitcoin / Liquid | Core/Electrum/Esplora/Elements watchers and script verifiers | Independent lock, confirmation, claim, refund, and reorg truth |
| Lightning | LND/CLN, BOLT11/BOLT12, NIP-47 | Invoice, hold, payment, and node-control surfaces |
| LSPs | [bLIP-50/51/52](https://github.com/lightning/blips) | Standard transport, channel purchase, and just-in-time channel negotiation |
| Ark | [Arkade `arkd`](https://github.com/arkade-os/arkd) and wallet adapters | Self-custodial VTXO liquidity and Lightning swap paths |
| P2P fiat | [Mostro protocol](https://mostro.network/protocol/) and NIP-69 | Existing Nostr order, hold-invoice, reputation, bond, and dispute semantics |
| Cashu | [Cashu NUTs](https://github.com/cashubtc/nuts) plus NIP-87 | Multi-mint e-cash quotes and Lightning redemption with explicit mint risk |
| Fedimint | [Fedimint](https://github.com/fedimint/fedimint) plus NIP-87 | Federation/gateway discovery and community-custody routes |
| Regulated fiat/stablecoins | PFI-specific adapters and selective credentials | Bank and token corridors without making one provider the protocol |
| Legacy Boltz clients | Optional Boltz-compatible REST/WSS facade | Gradual migration while Nostr owns portable discovery and provider choice |

A market-maker needs private inventory, spend authority, and rebalancing
adapters across those rails; those custody-bearing pieces remain provider
processes. Immortal will implement the noncustodial coordination half:
provider admission/catalogs, signed capacity and reservations, route and
state-machine handling, evidence verification, timers/recovery, and
compatibility APIs. This makes Immortal materially more than transport without
granting the relay custody.

## 9. Threats the protocol must make visible

| Threat | Required response |
| --- | --- |
| Stale or fake capacity | Short-lived Offerings; quote-level reservation; probe and rail proof |
| Double reservation | Provider sequence/idempotency rules; reservation digest and expiry; penalties only where enforceable |
| Public RFQ front-running | Pairwise encrypted RFQs and Quotes; no amount/account metadata in public events |
| Sybil reputation | User-selected trust roots, issuer lists, trade-key privacy modes, and no global score |
| Credential harvesting | Disclose after shortlist; audience/purpose-bound presentations; retention policy |
| Relay censorship or omission | Multi-relay publish/read, local persistence, completeness-aware pagination, direct fallback |
| Relay metadata correlation | Ephemeral keys, inbox separation, timing jitter, padding, Tor where needed |
| Reversible fiat after final crypto | Delayed release, escrow, guarantee, reserves, dispute/recourse, or explicit best-effort classification |
| Reorg/RBF/0-conf loss | Pair-specific confirmation and RBF policy verified by the taker |
| Oracle manipulation | Multiple signed sources, bounded staleness, quote binds exact source/time; price oracle never settles |
| Dispute capture | Disclose solver/arbiter set and appeal path before Order; support competing coordinators |
| Multi-hop partial completion | Shared atomic construction, pre-funding, guarantee, or explicit sequential exposure |

## 10. Current implementation snapshot and target

Reusable foundations:

- Immortal is a hardened one-binary/one-Postgres relay with signed-event
  validation, bounded queries, authentication, recipient gates, relay identity,
  Block-lane handlers, and manual conformance/deployment proofs.
- The signed workroom projection provides persist-before-publish outbox,
  deterministic tags, external signing, relay acceptance tracking, and the
  crucial rule that relay acceptance is not authority.
- OpenAgents has typed work, evidence, closeout, agent, capacity, alert, and
  skill contracts that can wrap an admitted treasury or provider operation.
- The existing liquidity and risk endpoints are explicitly inert typed
  skeletons. Their request/offer/fill/receipt vocabulary is a useful seed, not
  evidence of a live market.

Not yet implemented at this snapshot:

- no implemented NIP-MKT/MKT-SWP state machine or fixture corpus;
- no provider router, quote-reservation engine, wallet policy engine, or
  multi-relay market client;
- no rail adapters or independent settlement verifier in OpenAgents;
- no credential-presentation or provider-admission policy for this market;
- no live liquidity, underwriter, escrow, arbiter, reserve, or payout authority;
- no proof that `relay.openagents.com` alone provides operator diversity.

Those gaps remain visible for claim and rollout honesty, but they are not
exclusions. The committed destination is the complete OpenAgents Liquidity
Market, its reusable negotiated-market fabric, every pinned Immortal NIP, and
new focused NIPs wherever the three lanes do not yet express required
noncustodial behavior. Live capital, spend keys, bank/node credentials, and
final settlement authority remain with independent providers and rails.

## 11. Recommended build order

1. **Protocol vectors first.** Write NIP-MKT base fixtures and an MKT-SWP
   profile for one Bitcoin↔Lightning regtest pair. Freeze exact public/private
   fields, signatures, idempotency, timeouts, and errors.
2. **Wallet-side router.** Pull Offerings from multiple relays, apply a local
   provider policy, send private RFQs, compare signed Quotes, and persist the
   session before publishing.
3. **Independent LPs.** Run at least two separately keyed provider daemons on
   distinct relay sets. No central matcher.
4. **Boltz compatibility.** Put an owned adapter around the MIT crypto core and
   optionally expose/consume a Boltz-compatible facade. Verify every script,
   invoice, amount, timeout, and payment-hash binding client-side.
5. **Adversarial regtest.** Exercise relay partition, stale quote, double
   reservation, RBF, reorg, dropped status, LP crash, refund, and conflicting
   provider events.
6. **Mostro bridge.** Map existing NIP-69/Mostro order semantics into MKT-P2P
   without inventing a second incompatible P2P fiat network.
7. **Mint/federation and LSP profiles.** Reuse NIP-87 and bLIP semantics; expose
   custody class and exit route in every Quote.
8. **Credentialed PFI pilot.** Only with a real counterparty and legal owner;
   prove consented selective disclosure, retention, dispute, and a non-atomic
   failure journey before any production claim.

The first success criterion is not volume. It is a wallet completing the same
regtest swap against either of two providers, over independently chosen relays,
while recovering safely from one provider and one relay failure.

## 12. Central finding

tbDEX's most valuable contribution is a market grammar for **heterogeneous
trust**. Boltz contributes the strongest settlement primitive for the subset
of routes where cryptography can minimize trust. Nostr contributes portable
identity, discovery, encrypted messaging, and signed receipts. Immortal can be
one hardened noncustodial relay in that network.

The unified system should never choose between “everything is trustless” and
“trust the provider.” It should let many providers compete under a common
wire protocol, make the guarantee and custody model part of each signed quote,
verify whatever can be verified locally, and expose whatever cannot.

---

*End of teardown. Research and candidate protocol only; no market, provider,
financial authority, or deployment is created by this document.*
