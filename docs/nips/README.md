# OpenAgents NIP Drafts

These living draft specs were restored from the last shipped pre-Bun-rebuild
tree at `f5919c766^:crates/nostr/nips/`. They are protocol docs, not runtime
implementations, and publishing them does not create a new public product
claim. A capability is live only when the matching implementation, receipts,
and settlement records exist.

Outside Nostr and Bitcoin developers can use these drafts to implement
compatible market, agent, credit, training, work-coordination, and security-
hardening rails against the same event model.

> **NIP-90 direction (2026-08-04):** the official Nostr repository now marks
> NIP-90 unrecommended and asks implementers to prefer use-case-specific
> microstandards. Existing OpenAgents NIP-90 events and receipts remain
> readable and verifiable, but the profiles are frozen for new design. See
> [`NIP90-MIGRATION.md`](NIP90-MIGRATION.md) for the compatibility policy and
> the NIP-LBR v2, DS-private, compute, and NIP-MKT replacement map. NIP-MKT
> is drafted as the forward negotiated-market base, and its base (discovery
> heads, immutable private records, wrapped transport) has a first relay
> implementation in [Immortal](https://github.com/OpenAgentsInc/immortal)
> with a fixture corpus and machine-readable contract export. Focused
> MKT-SWP, MKT-P2P, MKT-PFI, MKT-MINT, MKT-LSP, and MKT-INTENT are complete
> upstream drafts with collision-reviewed profile kinds. Implementation and
> deployment claims belong to each consuming repository's conformance record;
> these documents do not grant an executable-profile claim.

| Spec       | Market stream                                                              | Kinds                                                     | File                             |
| ---------- | -------------------------------------------------------------------------- | --------------------------------------------------------- | -------------------------------- |
| NIP-MKT    | Negotiated-market base; public discovery, private signed execution control | 39600-39609; profile block 39610-39699                    | [`MKT.md`](MKT.md)               |
| MKT-SWP    | BTC, L-BTC, Ark, and Lightning submarine, reverse, and chain swaps         | 39610                                                     | [`MKT-SWP.md`](MKT-SWP.md)       |
| MKT-P2P    | NIP-69/Mostro-compatible peer trades and dispute resolution                | 39620                                                     | [`MKT-P2P.md`](MKT-P2P.md)       |
| MKT-PFI    | Credentialed provider/fiat-interface negotiation                           | 39630                                                     | [`MKT-PFI.md`](MKT-PFI.md)       |
| MKT-MINT   | Cashu mint/melt and Fedimint gateway negotiation                           | 39640                                                     | [`MKT-MINT.md`](MKT-MINT.md)     |
| MKT-LSP    | LSPS1 channel purchase and LSPS2 just-in-time liquidity                    | 39650                                                     | [`MKT-LSP.md`](MKT-LSP.md)       |
| MKT-INTENT | Maker-funded, any-filler covenant intents                                  | 39660                                                     | [`MKT-INTENT.md`](MKT-INTENT.md) |
| NIP-DS     | Data market; core forward, DS-DVM compatibility only                       | 30404-30407, legacy 5960/6960                             | [`DS.md`](DS.md)                 |
| NIP-LBR    | Labor market; v1 compatibility profile                                     | legacy 5930-5936, 6930-6936, 7000                         | [`LBR.md`](LBR.md)               |
| NIP-SKL    | Skills registry                                                            | 33400/33401, 33410/33411                                  | [`SKL.md`](SKL.md)               |
| NIP-SA     | Sovereign agents                                                           | 39200-39203, 39210-39213, 39220-39221, 39230-39231, 39260 | [`SA.md`](SA.md)                 |
| NIP-AC     | Agent credit                                                               | 39240-39246                                               | [`AC.md`](AC.md)                 |
| NIP-TRN    | Training                                                                   | 39500-39530                                               | [`TRN.md`](TRN.md)               |

[`PROPOSED.md`](PROPOSED.md) indexes the proposed next wave: the All Work NIP
program that encodes the Linear-class planning, agent delegation, coding,
review, evidence, and outcome system on Nostr, in the spirit of the Block Buzz
extension NIPs. All five layers are drafted:

| Spec   | Role                                 | Reserved kinds       | File             |
| ------ | ------------------------------------ | -------------------- | ---------------- |
| NIP-WK | Work root object                     | 32170-32173          | [`WK.md`](WK.md) |
| NIP-WI | Work Intents and Admission           | 32180-32182          | [`WI.md`](WI.md) |
| NIP-EV | Evidence, Verification, Dispositions | 32190-32193          | [`EV.md`](EV.md) |
| NIP-OT | Organizations and Teams              | 32100-32104          | [`OT.md`](OT.md) |
| NIP-PI | Project Issue projection             | 32200                | [`PI.md`](PI.md) |
| NIP-WR | Work Relations                       | 32210                | [`WR.md`](WR.md) |
| NIP-WS | Workflow States and Labels           | 32215-32217          | [`WS.md`](WS.md) |
| NIP-PG | Planning Graph                       | 32220-32227          | [`PG.md`](PG.md) |
| NIP-RP | Release Planning                     | 32240-32243          | [`RP.md`](RP.md) |
| NIP-DD | Documents and Decisions              | 32250-32252          | [`DD.md`](DD.md) |
| NIP-CN | Customers and Needs                  | 32260-32262          | [`CN.md`](CN.md) |
| NIP-AD | Assignment and Delegation            | 32270-32271          | [`AD.md`](AD.md) |
| NIP-AS | Agent Sessions                       | 32280                | [`AS.md`](AS.md) |
| NIP-AV | Agent Activity                       | 32290                | [`AV.md`](AV.md) |
| NIP-RC | Repository Work Claims               | 32300-32302          | [`RC.md`](RC.md) |
| NIP-CC | Code Context and Coding Sessions     | 32310-32311          | [`CC.md`](CC.md) |
| NIP-RV | Reviews                              | 32320-32322          | [`RV.md`](RV.md) |
| NIP-WA | Workroom Activity                    | 32150-32163 (pinned) | [`WA.md`](WA.md) |
| NIP-GB | Guidance Bundles                     | 32330-32331          | [`GB.md`](GB.md) |
| NIP-AL | Automation Loops                     | 32340-32341          | [`AL.md`](AL.md) |
| NIP-TP | Triage Proposals                     | 32350                | [`TP.md`](TP.md) |
| NIP-AT | Attention and Notifications          | 32360-32361          | [`AT.md`](AT.md) |
| NIP-HP | Hosts and Placement                  | 39560-39562          | [`HP.md`](HP.md) |
| NIP-OC | Outcome Closeout                     | 39580-39581          | [`OC.md`](OC.md) |
| NIP-PP | Product Promises                     | 32440-32442          | [`PP.md`](PP.md) |

The Bitcoin OSS hardening application adds five drafted protocol modules.
They remain design records until an implementation decision, fixtures, and
runtime evidence land:

| Spec   | Role                                     | Reserved kinds | File             |
| ------ | ---------------------------------------- | -------------- | ---------------- |
| NIP-SP | Scan Profiles and Pre-Registration       | 32450-32451    | [`SP.md`](SP.md) |
| NIP-SC | Source Completeness and Coverage         | 32460-32462    | [`SC.md`](SC.md) |
| NIP-FD | Findings, Verdicts, and Disclosure       | 32470-32473    | [`FD.md`](FD.md) |
| NIP-SI | Security Invariants and Regression Watch | 32480-32482    | [`SI.md`](SI.md) |
| NIP-BT | Bounties and Contribution Credit         | 32490-32492    | [`BT.md`](BT.md) |

## Per-spec summaries

### NIP-MKT — Negotiated Markets

The forward common control plane for provider markets: public Provider
Profiles, Offerings, and digest-pinned Profile Descriptors lead into private,
independently signed RFQ, Quote, Order, Status, Cancel, and Close records.
NIP-59 gift wraps hide negotiation metadata while a signed inner event keeps
each term independently verifiable. Exact event IDs bind terms; unique `d`
values provide idempotency; dense per-signer Status sequences expose gaps and
forks; expiry never implies consent; and Close is a claim rather than
settlement. The collision-reviewed base uses `39600-39609`; focused profiles
allocate `39610` to MKT-SWP, `39620` to MKT-P2P, `39630` to MKT-PFI, `39640`
to MKT-MINT, `39650` to MKT-LSP, and `39660` to MKT-INTENT. Family gaps remain
unallocated. Profiles own assets, custody, reservation physics, credentials,
rail evidence, finality, disputes, refunds, and recovery. Relays may validate,
route, reserve provider-signed capacity, run timers, and verify evidence while
remaining noncustodial; they never inherit wallet or settlement authority.
v0.1 adds the mandatory
coordinator-independent recovery floor for profiles, the profile
field-vocabulary laws, and an informative implementation record (the
Immortal relay base).

### MKT-SWP — Bitcoin, Lightning, Liquid, and Ark Atomic Swaps

Defines the BTC, Lightning, Liquid, and operator-bound Ark v1 swap contract
over the NIP-MKT negotiation spine: submarine, reverse, and chain lifecycles;
verify-before-fund rules; reservation proof classes; timeout ladders; external
evidence and finality authorities; and coordinator-independent claim/refund
exits. Private immutable `kind:39610` binds the accepted Order, exact rail
verifier inputs, and exit-package commitments without carrying seeds,
preimages, private keys, or node credentials. The remaining `39611-39619`
family is unallocated.

### MKT-PFI — Credentialed Provider and Fiat Interfaces

Defines quote-first/credentials-later on/off-ramp negotiation with encrypted
credential references, reversible-rail risk classes, explicit guarantee and
recourse authorities, settlement evidence, chargeback handling, and no PII on
relays. Public `kind:39630` is a provider-signed, digest-pinned Qualification
Policy with a closed no-PII schema; all credential presentations and private
terms remain inside recipient-gated records or off-relay stores.

### MKT-P2P — Peer-to-Peer Bitcoin Trades

Bridges NIP-69 and Mostro lifecycle semantics into NIP-MKT without replacing
their public order network. Private `kind:39620` carries a solver or appeal
arbiter Resolution under the exact authority set pinned in the Quote.

### MKT-MINT — Cashu and Fedimint Routes

Keeps NIP-87 and the native Cashu/Fedimint protocols authoritative while a
bilateral private `kind:39640` Route Contract binds the post-Order native
quote, custody terms, verifier inputs, external effects, and recovery package.

### MKT-LSP — Lightning Service Providers

Maps bLIP-50/51/52 channel-purchase and JIT-liquidity terms onto NIP-MKT.
Client and LSP sign matching private `kind:39650` Service Contracts before
payment, preimage release, or funding.

### MKT-INTENT — Covenant Intents

Defines a public maker-funded, any-filler, zero-interactivity market shape.
Public `kind:39660` derives its address from canonical terms; the external
constraint rail remains execution and settlement authority.

### NIP-DS — Datasets

Makes datasets, artifacts, and reusable context bundles first-class tradable
objects on Nostr. DS core defines a canonical dataset listing (`kind:30404`)
anchored by a mandatory SHA-256 digest, draft/inactive listings
(`kind:30405`), access offers (`kind:30406`, open, targeted, quote-only, or
subscription), and a durable per-buyer access contract (`kind:30407`) that
tracks payment-required, paid, delivered, revoked, expired, and refunded
state. Historical optional profiles reuse NIP-15/NIP-99 for storefront and
classified discovery, NIP-90 (`5960`/`6960`) for request/quote/delivery
compatibility, NIP-28 for public negotiation, and NIP-17/NIP-44/NIP-59 for
private terms and delivery pointers. New work keeps DS core and replaces the
DS-DVM profile with a DS-specific private negotiation microstandard. Clients
must verify delivered payloads against the listing digest before treating them
as authentic.

### NIP-LBR — Agentic Labor

A strictly ref-only v1 labor compatibility contract for agentic coding work
over NIP-90. A
requester publishes a budgeted `kind:5934` work request, providers quote via
`kind:7000` feedback, the requester accepts exactly one quote with escrow
held in the platform ledger, and the provider delivers an output-only
`kind:6934` result carrying artifact and receipt refs. The relay is transport
only: identity, assignment, escrow, acceptance, and settlement authority live
in platform receipt systems, and events must never contain raw prompts,
credentials, private repository content, payment secrets, or wallet material.
Includes the OpenAgents Forum bridge profile that mirrors work requests,
offers, acceptances, and lifecycle receipts through public Forum APIs. New
labor design targets standalone NIP-LBR v2 rather than extending NIP-90.

### NIP-SKL — Agent Skill Registry

The identity and trust layer for agent skills. Defines addressable skill
manifests (`kind:33400`) with a mandatory canonical payload hash and explicit
capability allowlists (runtimes must not grant undeclared permissions), an
append-only version log (`kind:33401`), NIP-32 attestations with assurance
tiers (self-assessed, third-party-evaluated, red-team-tested), and NIP-09
same-pubkey revocation with an emergency pre-signed kill practice. Optional
profiles add an ephemeral proof-of-possession challenge/response pair
(`33410`/`33411`) and NIP-99 listings. The old NIP-90 skill-search flow is
compatibility-only; new clients query manifests, curated lists, and handlers
directly. SKL is
the registry that NIP-SA fulfillment and NIP-AC credit both reference through
`33400:<pubkey>:<d-tag>:<version>` scope identifiers.

### NIP-SA — Sovereign Agents

The largest spec: autonomous agents with their own Nostr identity that
operators delegate to rather than wield. Agent keys use threshold protection
(2-of-3 FROST shares across runtime enclave, marketplace, and guardian) so no
single party, including the operator, can extract them. Covers agent
profiles with security-posture declarations (`39200`), encrypted state
(`39201`), schedules and triggers (`39202`), public goals (`39203`), the
tick request/result execution loop with budget rails (`39210`/`39211`),
guardian approval requests and decisions for high-spend actions
(`39212`/`39213`), skill licenses and gift-wrapped delivery
(`39220`/`39221`), streaming trajectory sessions and events for audit trails
and training data (`39230`/`39231`), and scoped sub-agent delegation
(`39260`). Marketplace-mediated threshold ECDH enforces skill licensing, and
the SA-Guardian profile integrates Fedimint federations and out-of-band
approval hardware.

### NIP-AC — Agent Credit

Bitcoin-native, outcome-scoped credit for agents that start with zero
capital. Instead of free-floating loans, issuers grant Outcome-Scoped Credit
Envelopes (`kind:39242`) bound to one verifiable scope (a NIP-90 job, an
L402 resource, or a pinned SKL skill version) with a hard cap and expiry.
`nip90` is the legacy scope; new envelopes bind the canonical order/outcome
digest defined by the owning use-case microstandard.
The flow runs intent (`39240`), offer (`39241`), envelope, ephemeral spend
authorization (`39243`), settlement receipt (`39244`), and default notice
(`39245`), with an optional cancel-spend event (`39246`) inside a declared
reversibility window. Repayment and spending rails cover Lightning bolt11/
bolt12, Cashu, and Fedimint. Failure is handled through reputation decay via
NIP-32 labels rather than slashing, guardian-gated envelopes require
co-approval above a spend threshold, and negative SKL safety labels act as a
revocation trigger for envelopes scoped to the flagged skill.

### NIP-TRN — AI Model Training Coordination

The coordination and publishing layer for multi-party model training. The
design rule: TRN carries names, links, checksums, pointers, and receipts —
big training files stay off Nostr. Defines network contracts (`39500`), node
records (`39501`), training windows (`39510`), generic append-only receipts
(`39511`), validator verdicts (`39512`), signed artifact locators for
checkpoints, weights, local updates, aggregates, proofs, and scores
(`39520`), and contribution closeouts covering rewarded, held, quarantined,
refused, and slashed outcomes (`39530`). Runs are recoverable and forkable
from relay history alone: if a coordinator disappears, another operator can
find the accepted checkpoint lineage and continue. The TRN-DiLoCo profile
covers local-update rounds, weighted aggregation, and checkpoint promotion
for periodically synchronized training.

### NIP-WK — Work (proposed layer 0)

The root object of the All Work program. An authority-signed Work Record
head (`32170`) carries domain, state, revision, owner, and participant/
planning/session/outcome refs for one bounded objective in any Work Domain.
Append-only Work Events (`32171`) use unique-`d` addressable records with a
dense sequence so a missing number is an explicit, displayed gap. Versioned
digest-bound Objectives (`32172`) let plans and evidence name the exact
intent revision they were made against, and the Outcome Record (`32173`)
indexes evidence, verification, disposition, and settlement refs without
becoming a verdict. Only the Organization's declared authority key makes
these kinds canonical; participants change Work through NIP-WI.

### NIP-WI — Work Intents and Admission (proposed layer 0)

The command wire: a client-signed event is a proposal with product meaning
only after an authority-signed admission exists. Work Intents (`32180`)
bind actor, operation, idempotency key, argument digest, expected revision,
generation, and expiry. Admission Results (`32181`) answer exactly one
intent with `admitted` (accepted revision plus resulting Work Event refs)
or `refused` (typed reason codes), under five laws: fail closed, idempotent
replay, one decision per intent, full provenance chain, and no inference
from silence. Work Candidates (`32182`) carry untrusted public intake —
the strict-bug pattern — which becomes Work only through an explicit
triage admission.

### NIP-EV — Evidence, Verification, and Dispositions (proposed layer 0)

The trust wire. Producer-signed Evidence Receipts (`32190`) bind exact
bytes to criteria by digest. Verification Receipts (`32191`) record an
executed evaluation under a named policy, with a hard independence floor:
the verifier key must differ from every evaluated producer key, and model
agreement is never verification. Owner Dispositions (`32192`) record the
accountable human's separate accept/reject/waive decision with explicit
per-criterion waivers, and Receipt Edges (`32193`) connect the records
into a traversable receipt graph. The proof-rung ladder is normative:
evidence, then verification, then acceptance, then release, then
settlement — and no rung implies the next.

### NIP-OT — Organizations and Teams (proposed layer 0)

The administrative scope and the trust bootstrap. The Organization Record
(`32100`) declares the All Work authority pubkey that makes every other
record canonical, with a two-sided key-rotation rule. Team Records
(`32101`) carry workflow policy and workroom refs; Membership Projections
(`32102`) bind principals to scopes with generation fencing, tombstoned
revocation, and NIP-OA attestation refs for agent members; Role
Definitions (`32103`) are display vocabulary that grants nothing; and
Workroom Bindings (`32104`) connect scopes to relay-qualified NIP-29
groups while stating plainly that relay `private` is access policy, not
encryption. Membership is visibility, never capability.

### NIP-PI — Project Issue (proposed layer 1)

The Issue projection of Work (`32200`): the concrete tracking view that
lists, boards, cycles, triage, and search render. An Issue shares its
Work's identity, revision, and Event history — never a second writable
record — carrying the team identifier (`CORE-142`), title, Workflow State,
priority, estimate, assignee/delegate display refs, labels, relations, and
planning placement. Every mutation is a NIP-WI intent on the underlying
Work, and the rendering contract lets any Nostr client build a full
tracker from standard filters.

### NIP-WR — Work Relations (proposed layer 1)

Typed directed edges between Work (`32210`): parent/child, blocks,
duplicate, related, supersedes, split-from, merged-into, plus
confidence-carrying prior-work lineage (`occurrence_of`, `same_cause`).
One authority-signed record per logical edge in a canonical direction,
acyclic hierarchy enforced at admission, duplicate chains flattened to the
surviving Work, and removals published as records so edge history stays
auditable. An edge is structure — it never mutates its endpoints.

### NIP-WS — Workflow States and Labels (proposed layer 1)

The configuration vocabulary: Team-scoped Workflow States (`32215`) with a
mandatory mapping onto the NIP-WK baseline so cross-team boards stay
coherent, scoped Labels (`32216`) with bounded hierarchy and exclusive
groups, and SLA Policies (`32217`) whose timers are projections over
canonical Work Event timestamps. All of it is policy data: a state named
`Done` is a category, and completion truth stays with the NIP-EV chain.

### NIP-PG — Planning Graph (proposed layer 1)

The portfolio layer: Initiatives (`32220`), Roadmaps (`32221`), Projects
(`32222`) with configured Project Statuses (`32223`), Project Milestones
(`32224`), Cycles (`32225`), and authored Project/Initiative Updates
(`32226`/`32227`). Every layer is optional per Work Domain, Work joins
Projects and Cycles through its own refs, and planning is context, not
authority — health colors and progress rows are authored or projected
values with source and freshness, never evidence.

### NIP-RP — Release Planning (proposed layer 1)

Release Planning Records (`32240`), Pipelines (`32241`), Stages
(`32242`), and Scope Links (`32243`) — planning data only, and the
boundary is the point: a pipeline flagged production, a stage named
`published`, or a `committed` scope link cannot create a Release
Candidate, pass a Deployment Gate, or authorize publication. Scope
changes stay append-visible, target commits are unverified intent, and
actual releases live with the release authority, NIP-EV evidence, and the
future NIP-PP promise registry.

### NIP-DD — Documents and Decisions (proposed layer 1)

The knowledge layer: versioned Documents (`32250`) with append-only
digest-bound revision archives (`32251`) so agents can pin the exact
revision they consumed, and Decision records (`32252`) capturing
question, decider, alternatives, rationale, and supersession — a
discussion message is not a Decision until recorded, and a Decision is
not admission. Comments reuse NIP-22 and attachments reuse NIP-94/
Blossom; prose in either kind grants nothing.

### NIP-CN — Customers and Needs (proposed layer 1)

The privacy-inverted stakeholder layer: opaque-by-default Customer
records (`32260`) with NIP-44-encrypted identity, Customer Needs
(`32261`) linking demand to Work with satisfaction gated on Accepted
Outcomes, and consent-classed Customer Signals (`32262`) carrying
digest-bound source evidence. Public relays see refs, links, and digests
only; customer identity never creates a commitment, grant, or Work
State.

### NIP-AD — Assignment and Delegation (proposed layer 2)

The structural Assignee/Delegate split. Assignment Records (`32270`) bind
the accountable human — exactly one per Work, person-only, and the valid
decider for Owner Dispositions. Delegation Grants (`32271`) authorize one
Agent Member per Work with named issuer, capabilities, tool policy,
budget, privacy class, expiry, and generation; revocation fences every
session and late intent on the old generation, and repository-domain
grants carry their NIP-RC claim/lease pair. Prose anywhere cannot widen a
grant, and no composition of grants makes an agent the accountable human.

### NIP-AS — Agent Sessions (proposed layer 2)

The durable record of one agent engagement (`32280`), kept strictly apart
from the Work, the Thread, and the Run. Sessions carry grant and
generation, context-manifest refs, requested-versus-effective runtime
identity (never collapsed, per no-silent-substitution), an activity
cursor, and a state machine where `awaiting_input` names its blocking
question, `stale` is declared rather than assumed, terminal states are
terminal, and continuation is a new session with a predecessor ref.
Provider `complete` implies nothing about verification or acceptance.

### NIP-AV — Agent Activity (proposed layer 2)

The typed, redacted, append-only supervision stream (`32290`): progress,
plan, elicitation, action, artifact, result, verification, error, and
disposition activities with dense sequence, digest-bound off-relay
material, and explicit loss accounting. Elicitations are answer-fenced —
question, answerer, decision, and admission are separate signed records.
Never raw chain of thought or provider payloads, and narration is not
effect: an evidence-less `result` renders as unverified. The actor-signed
workroom twin is NIP-WA `32156`; the AV record wins for product meaning.

### NIP-RC — Repository Work Claims (proposed layer 2)

The multi-agent collision ledger: Work Packets (`32300`) naming canonical
Work, owned paths, hot files, and hot contracts with explicit collision
classes; Repository Work Claims (`32301`) with holder, generation,
evidence heartbeats, and explicit release; and append-only Claim Audit
Entries (`32302`). Claims refuse before colliding, takeover requires both
90-plus evidence-less minutes and a recorded process audit, release
asserts nothing about landing, and a claim is never an Assignee, Lease,
merge authority, or proof of progress.

### NIP-CC — Code Context and Coding Sessions (proposed layer 2)

Binds Work to exact code without ambient access. Code Context (`32310`)
records repository refs, pinned commits, focus paths, and the effective-
access intersection — listing a repository narrows scope and never grants
it. Coding Sessions (`32311`) companion NIP-AS records with a mandatory
pinned base commit, opaque worktree identity, containment profile, held
claim, verification evidence, and bounded diffstat; catch-up to a newer
base is explicit with fresh compatibility proof, and Git/NIP-34 remains
the sole authority for PR, check, and merge state.

### NIP-RV — Reviews (proposed layer 2)

Review Requests (`32320`) whose state is a policy fold over admitted
reviewer-signed Verdicts (`32321`), each bound to the exact subject
revision so approve-then-push races surface as staleness. Self-review is
labeled and cannot satisfy independence-requiring folds; agent verdicts
run under grants and never satisfy human-required approvals. Review
Guides (`32322`) are regenerable evidence-linked explanations with
exactly zero authority — a verdict contributes evidence toward NIP-EV
verification and is never merge authority or acceptance.

### NIP-WA — Workroom Activity (proposed layer 2, kinds pinned)

Formalizes the implemented `openagents.signed-workroom.v2` profile on the
pinned range `32150-32163`: fourteen actor-signed collaboration
projections (membership through revocation) with deterministic tags,
empty content, and payload digests. Covers direct principals versus
grant-bound organizational/device/agent signers, the two-phase
prepare/commit external-signing lane where the authority fixes the exact
unsigned bytes, causal-parent and generation admission, audience-scoped
server-owned relay sets, and persist-before-publish delivery whose
receipts fix `relayAcceptanceIsAuthority: false`. Projection transport,
never command or product authority.

### NIP-GB — Guidance Bundles (proposed layer 3)

Versioned standing instructions on kinds `32330`/`32331`: scoped bundles
(organization through work-item) with a deny-wins-downward precedence
chain, fail-closed same-layer conflicts, and append-only digest-bound
revision archives. Every agent run records the exact guidance revisions
it consumed in its context manifest, so behavior change without a
revision change is a defect. Guidance instructs and constrains; natural
language never mints capability.

### NIP-AL — Automation Loops (proposed layer 3)

Bounded recurring agent workflows on kinds `32340`/`32341`: immutable
published revisions, typed trigger selectors that confer eligibility but
never bypass admission, mandatory suggest mode before policy-admitted
auto mode, and five loop laws — structural self-trigger exclusion checked
at publish, idempotent triggering, bounded concurrency and budget,
circuit breaking to a typed paused state, and revision-pinned runs. Loop
runs are ordinary Agent Sessions on ordinary Work.

### NIP-TP — Triage Proposals (proposed layer 3)

Reviewable suggestions on kind `32350`: routing, duplicates, labels,
priority, ownership, readiness, and candidate dispositions, each with
proposer-asserted confidence, evidence refs, and expiry. A proposal never
mutates Work — a human disposition or an admitted auto-apply policy
converts it into a NIP-WI intent, conflicting proposals suspend each
other, and because proposals, dispositions, and applied intents are all
signed records, precision is measurable from the wire and gates auto
promotion.

### NIP-AT — Attention and Notifications (proposed layer 3)

The private attention layer on kinds `32360`/`32361`: authority-signed
Attention Items encrypted to a blinded recipient with typed reasons,
grouping keys, and required-action refs, plus principal-signed
self-encrypted Notification Subscriptions that can suppress delivery but
never widen authorization. Dismissing an item changes nothing about its
subject, and the Block lane keeps its jobs: NIP-RS read state, NIP-ER
reminders, NIP-PL push wake-ups with no content transiting platform push.

### NIP-HP — Hosts and Placement (proposed layer 4)

Where work runs, on kinds `39560-39562`: owner-signed Host Records for
every placement class (local machine through production target) with
generation fencing and observed-not-assumed health, owner-signed Capacity
Statements with counted slots, flexibility classes, and freshness, and
authority-signed Dispatch Decisions binding the exact policy revision and
input refs. The three boundaries hold everywhere: reachability is not an
execution grant, a capacity advertisement is not a Dispatch Decision, and
a decision is not proof the work ran. Extends the NIP-TRN node-record
pattern to general Work.

### NIP-OC — Outcome Closeout (proposed layer 4)

The terminal economics, on kinds `39580-39581`: Accepted Outcomes exist
only by derivation from an admitted `accepted` Owner Disposition, carry
receipt-backed contributor and component attribution through the NIP-EV
graph, and label every economic value with its provenance rung
(modeled through settled, never inferred upward). Closeouts reconcile
every ending — accepted, rejected, cancelled, refunded — so acceptance
rate, rework, cost per accepted outcome, and accepted-outcomes-per-
kilowatt-hour are computable from the wire by any authorized reader.
Settlement stays with its designated ledger; this NIP only references it.

### NIP-PP — Product Promises (proposed layer 4)

The public trust registry, on kinds `32440-32442`: one Product Promise
record per named capability with claim text, scope, state
(green/yellow/red/degraded/planned), evidence and blocker refs, and
outside-verification guidance; dense append-only Promise Transitions that
are evidence-gated upward and deliberately fast downward; and a
digest-bearing Registry Head that lets any reader prove they hold the
complete registry and detect omission. Nothing but an authority-signed
transition changes a state — roadmaps, marketing, screenshots, and demos
inherit nothing. Makes the live `openagents.com/promises` contract
portable and independently verifiable.

### NIP-SP — Scan Profiles and Pre-Registration (hardening application)

Defines a digest-bound Scan Profile (`32450`) as the reusable measurement
instrument for a security run: exact source-materialization rules, selection
and hunt policies, model/harness roles, budgets, and evidence requirements. A
Pre-Registration (`32451`) freezes the target, commit, profile revision,
hypothesis, and scoring-rubric digest before execution. A later run can cite
that immutable setup; a changed rubric or profile is a different experiment,
and neither a signature nor relay acceptance proves that a scan ran.

### NIP-SC — Source Completeness and Coverage (hardening application)

Makes the readable source boundary and its gaps signed data. A Materialized
Source Set (`32460`) records the exact repository revision, populated and
missing submodules, vendored/dependency inputs, analyzed and skipped counts,
and loss reasons. A Coverage Attestation (`32461`) classifies one run as
complete, partial, or degraded against that source set and profile; a result
without this reference is an anecdote. Divergence Notes (`32462`) compare runs
over the same target, commit, and profile without turning disagreement into a
verdict.

### NIP-FD — Findings, Verdicts, and Disclosure (hardening application)

Separates discovery, independent judgment, and publication. A Finding
Commitment (`32470`) establishes priority without disclosure; an encrypted
Candidate Finding (`32471`) carries the mechanism, proposed severity, CWE,
assumptions, and evidence boundary to the authorized audience; a distinct
verifier signs a Finding Verdict (`32472`); and authority-signed Disclosure
State (`32473`) records the maintainer and publication lifecycle. Embargo
expiry never publishes content by itself, and none of these records implies
owner acceptance, release, settlement, or public claim authority.

### NIP-SI — Security Invariants and Regression Watch (hardening application)

Turns one-time findings into durable properties. A Security Invariant
(`32480`) names the source/configuration/build/artifact/runtime property,
required witnesses, and falsifiers. Artifact Provenance Witnesses (`32481`)
bind exact build inputs and artifacts to observed checks. Regression Watches
(`32482`) re-evaluate a pinned invariant against later revisions with explicit
freshness, stopping, and loss rules. A failing watch is evidence for triage;
it is not a verdict or disclosure decision.

### NIP-BT — Bounties and Contribution Credit (hardening application)

Defines campaign Funding Pools (`32490`), receipt-backed Contribution Credits
(`32491`) for coverage, reproduction, independent verification, refutation,
invariant, and confirmed-finding work, and optional Payout References
(`32492`) to an external admitted settlement rail. Credit works without money,
negative work can earn it, and duplicate evidence cannot mint it twice. The
NIP moves no funds and does not make publication a condition of credit;
pledged, measured, paid, and settled values remain distinct.

## How the specs fit together

SKL is the shared identity and trust substrate: SA skill licenses and AC
credit scopes both pin exact SKL manifest versions. Existing SA tick results
may cite AC settlement receipts and legacy NIP-90 job results so historical
cost remains auditable. Future envelopes and results cite the owning
use-case-specific microstandard. NIP-MKT supplies the negotiated-market spine
without absorbing DS, LBR, TRN, SKL, SA, or AC authority. AC can fund an exact
MKT Order, L402 resource, or skill invocation without making its settlement
authority part of their wire. TRN closeouts can link AC receipts for training
rewards. DS and LBR retain NIP-90 read compatibility while their next
versions own separate data and labor state machines.
