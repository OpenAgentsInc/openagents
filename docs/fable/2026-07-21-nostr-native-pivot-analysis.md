# The Nostr-Native Pivot — Architecture for Syncing OpenAgents with Agentic Nostr

**Date:** 2026-07-21
**Lane:** Fable strategy analysis
**Status:** Owner-directed analysis. This document flips no promise state, changes
no runtime authority, mints no issue, and dispatches no work. The factual
authorities remain current code, `docs/sol/MASTER_ROADMAP.md` (revision 127 at
writing), live issue state, contracts, and receipts. Proposal packets derived
from this document require normal Sol admission and owner acceptance.
**Sources:** `docs/teardowns/2026-07-21-buzz-teardown.md` (full Buzz audit at
commit `e9188c0`), the `block/buzz` vision corpus (`VISION.md`,
`VISION_SOVEREIGN.md`, `VISION_MESH.md`, `VISION_AGENT.md`), `nostr-effect` at
commit `c160378`, the canonical `nostr-protocol/nips` reference clone,
`specs/*.product-spec.md`, `docs/nips/`, `packages/nip90`,
`docs/sol/MASTER_ROADMAP.md`, and the recent transcript run
`docs/transcripts/250.md` and `254.md`–`259.md` (especially episode 259,
"Verifiable Software and the Energy Layer").
**Companion:** `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`,
`docs/fable/2026-07-19-verifiable-software.md`.

---

## I. The question

The owner directive behind this document: assume software cost is essentially
zero, because we can produce and verify software changes at very high speed.
Under that assumption, evaluate a much harder pivot to a Nostr-centric
OpenAgents. Do not ask "how expensive is the migration." Ask "what is the right
architecture," such that OpenAgents syncs up with Block's Buzz, with the ngit
and DVM ecosystems, and with Bitcoin land generally — and such that OpenAgents
can lead on agentic Nostr NIPs rather than follow them.

This document answers in seven moves:

1. What Block just proved, and the strategic hole they left open (§II).
2. The OpenAgents vision in its own words — the verifiable-software engine
   and the energy layer from the recent episode run — and the full overlap
   map: what is uniquely ours, what is uniquely Buzz, and where the two
   systems genuinely meet (§III).
3. Where OpenAgents actually stands — much closer to protocol-complete than
   the roadmap's parked-lane status suggests (§IV).
4. The usable building blocks in the standard NIP catalog, with the specific
   deprecations and collisions that shape what we should build on (§V).
5. The architectural principle that resolves the "relay as authority" question
   without protocol romance: **authority follows party count** (§VI), and the
   five-plane target architecture that follows from it (§VII).
6. The standards play: how OpenAgents plus Block equals the NIP acceptance
   bar, and the concrete agentic NIP suite we should drive (§VIII).
7. What "overnight" really buys and what it cannot, a staged candidate
   program, and honest counterarguments (§IX–§XII).

The one-paragraph conclusion, stated up front: **go Nostr-native at the market
and identity boundary, stay owned at the private product core, and put the
serious energy into the standards play.** Every principal — user, agent,
device, workspace — gets a sovereign keypair. Every multi-party artifact —
offer, quote, acceptance, result reference, receipt, reputation assertion,
patch, review — becomes a signed Nostr event, because a marketplace's
counterparties should never have to trust our database. Every single-party
artifact — session state, run registry, private telemetry — stays in owned
storage with signed projections outward. The immediate prize is not any single
feature: it is that OpenAgents and Buzz together constitute the two-client,
one-relay implementation bar that turns draft agentic NIPs into standards, and
whoever writes those standards shapes the agent economy that forms on top of
them.

## II. What Block just proved — and what they left on the table

### II.1 The proof

Buzz is a publicly traded company's open-source bet, four and a half months
old, 1,764 commits, ~218,000 lines of backend Rust, shipping desktop releases
multiple times per day, that a Nostr relay can be the entire workspace: chat,
forum, git forge, CI, review, workflows, voice, moderation, and agent
supervision as one signed event log. Their own words: "The relay is the
workspace" (`VISION.md`), "your community is your compute" (`VISION_MESH.md`),
"humans and agents are just colleagues."

Three of their proofs matter strategically:

- **Agents as first-class protocol citizens work.** An agent with its own
  keypair, memberships, owner-decryptable memory (NIP-AE), turn metrics
  (NIP-AM), and audit trail is a better primitive than an API bot with
  permission flags. Their web-of-trust framing in `VISION_SOVEREIGN.md` — "an
  agent with a persistent keypair and a verifiable contribution history is
  fundamentally different from an anonymous generator... its reputation is on
  the line with every contribution, across every project it touches" — is the
  exact identity thesis the OpenAgents marketplace needs, independently
  arrived at.
- **The workspace NIP gap is real and they filled it.** Fifteen custom NIPs
  (identity attestation, agent auth, engrams, telemetry, personas, git
  signing, push leases, read state, and more) is what it actually takes to
  make Nostr a workspace substrate. That is field data no amount of
  whiteboarding would produce.
- **Formal verification and Nostr are compatible.** Runtime conformance
  replay against TLA+ specs, Tamarin proofs on the auth and pairing models —
  from the company that also ships confetti on `/ship`. Their verification
  posture and our Verifiable Software Engine thesis
  (`docs/fable/2026-07-19-verifiable-software.md`) rhyme, which makes
  co-authorship of specs culturally plausible, not just technically.

### II.2 The hole

Read the Buzz kind registry (teardown Appendix A) and the vision corpus
against the standard NIP catalog and one absence is glaring: **there is no
money anywhere in Buzz.** No NIP-57 zaps, no NIP-47 wallet connect, no
NIP-60/61 ecash, no NIP-99 listings, no bids, no bonds, no receipts with
settlement meaning. Their job protocol (kinds 43001–43006) is request →
accepted → progress → result → error with no price field and no payment
event. Buzz Mesh shares GPUs inside a membership boundary as a commons — "no
API keys, no cloud bill" — deliberately not a market. Their NIP-AM turn
metrics carry an advisory `costUsd` estimate, encrypted to the owner: cost
accounting as private telemetry, not commerce.

This is coherent for Block's product: Buzz sells workspace sovereignty to
teams, and Block already owns other rails for money. But it leaves the entire
economic layer of agentic Nostr unclaimed: discovery of paid agent labor,
bidding, escrow, settlement signals, verifiable earnings, portable reputation
priced in outcomes. That layer is precisely OpenAgents' declared thesis. The
web trust-surface ProductSpec (revision 7) already states "Bitcoin, Lightning,
and Nostr are the only rails," describes the run-a-Pylon seller path, BOLT12
agent tips, the money-moderated Forum, and "accepted outcomes per
kilowatt-hour" as the clearing-layer doctrine. `docs/nips/LBR.md` specifies
the agentic-labor loop. `packages/nip90` ships tested labor kinds with
provider bonds and content-addressed closeout receipts.

**The complementarity is almost suspiciously clean.** Block proved the
workspace and agent-identity substrate and built no market. We designed the
market and parked it. The strategic move is not to compete with Buzz on
workspaces or copy their shell — the teardown already rejected the Tauri
shell, the Flutter lane, and relay-as-product-authority for our surfaces. The
move is to adopt their identity/memory/telemetry vocabulary where it is good,
interoperate at the protocol seam, and become the reference implementation of
the layer they skipped.

## III. The vision in our own words — and the overlap map

Before mapping protocols, state what OpenAgents is actually building, in the
owner's own recent words, because the pivot only makes sense as an
acceleration of that vision — never a detour from it. The recent episode run
(250, 254–259) is unusually explicit.

### III.1 The engine: electrons to accepted outcomes

Episode 259 ("Verifiable Software and the Energy Layer") states the company
question in one line: *"You have a stream of electrons. What is the cost of
turning that into an accepted agent task?"* The metric is the one episode 232
coined — **accepted outcomes per kilowatt-hour** — and the lineage is Bitcoin
mining: mining proved electrons could become money because a hash verifies
itself; verifiable software is how electrons become outcomes, by making every
claim between the intent and the deliverable carry its own proof. The
economic argument underneath is the measurability gap from "Some Simple
Economics of AGI": generation is collapsing toward free while verification
stays linear, so the price of an accepted outcome is dominated by review,
retries, and grading — not model tokens. Whoever industrializes verification
owns the conversion.

The product stack that follows, all from the same episodes:

- **The engine is the IDE.** OpenAgents Desktop as "your last agent IDE" —
  100% open source, improving faster than anyone, with an incident-to-gate
  loop (episode 258's sixteen controls from one crash report) that turns
  every failure into a permanent structural barrier. Not "vibe code and
  hope": create, test, verify, attest, then sell.
- **The truth layer is contracts, not prose.** Episode 250 named the failure
  class — the *unverified operational directive*, a plausible sentence whose
  one load-bearing token is fiction — and the fix: typed intents, decoded
  capabilities, model-identity receipts, behavior contracts with executable
  oracles. "Cursor ships features but not the contracts." In 2026 the
  competitive list is a trust-and-openness list, because the features are
  table stakes.
- **The network is verification.** Agents on the Forum already vet OpenAgents
  software against published programmatic manifests. The stated intent is to
  package that as product — automated QA, certifications, "gold stars,"
  explicitly **NIP-32 reputation events** attesting that software does what
  it claims — with creative economics: the vetting agent might earn a slice
  of the vetted software's revenue. Third-party contributors participate
  economically, not just socially.
- **The rails are back, in the editor.** Episode 259, verbatim intent: "we're
  specifically bringing back our Nostr and Bitcoin integration built into the
  editor itself — the economic layer of all this is going to start taking
  shape rapidly." This document is the architecture for exactly that
  sentence.

### III.2 Uniquely ours, uniquely theirs, genuinely shared

With that vision stated, the Buzz comparison becomes precise rather than
vibes-based:

| | OpenAgents (uniquely ours) | Buzz (uniquely theirs) | Shared ground |
| --- | --- | --- | --- |
| Core unit | The **accepted outcome** — work scoped as falsifiable intent, verified, receipted, and paid | The **community** — one relay, one URL, one membership boundary | A signed, auditable record of everything that happened |
| Economic model | A **market**: strangers transact because receipts, bonds, and reputation make acceptance checkable and payable | A **commons**: members pool compute and trust because they already chose each other; no payments layer at all | Agents as economic-ish actors with persistent identity |
| Trust mechanism | **Verification** — producer/verifier separation, behavior contracts, promise registry, assurance specs, closeout receipts | **Membership** — the relay gate is the only gate; "channel membership is the only gate" | Cryptographic identity as the root of both |
| Physical thesis | The **energy layer**: accepted outcomes per kilowatt-hour, mining lineage, the electron-to-outcome refinery | The **workspace layer**: replace Slack + Discord + GitHub with one sovereign domain | Self-hosting sympathy; "your keys, your identity" |
| Agent surface | The **coding workbench and fleet**: Full Auto runs, portable sessions, provider fleets, an IDE with the best practices built in | The **chat room**: agents as channel members with memory, personas, and jobs, supervised from a workspace app | ACP harness pools; agent-first CLIs; one skill source |
| Compute story | **Pylon**: sell your compute/labor/verification into an open market with settlement | **Mesh**: share your GPUs inside the membership, no API keys, no bill | Idle hardware as agent capacity |
| Verification culture | Runtime receipts, evidence-gated projections, executable oracles, STE-disciplined docs | TLA+ conformance replay, Tamarin proofs, mutation-tested guarantees | Rare shared conviction that agent systems must be *proven*, not narrated |
| Shell | Electron + Effect Native, one typed component set | Tauri + React desktop, Flutter mobile | Open-source, multi-release-per-day velocity |

The deepest contrast compresses to one sentence: **Buzz trusts members;
OpenAgents verifies strangers.** Buzz's whole design — tenancy fences,
membership-gated git reads, mesh compute, owner-decryptable memory — makes a
chosen group safe to work inside. It has no answer, and does not attempt one,
for the question OpenAgents is built around: how two parties who share no
membership, no employer, and no trust get to a paid, accepted outcome. That
question is answered by verification plus settlement, which is why the
market plane in §VII is ours to build and why episode 259's "receipts that
let acceptance be checked and paid by strangers" is the single line that
separates the two products.

The overlap is real and usable precisely because it sits below that line:
agent keypairs, owner attestation, encrypted agent memory, turn metrics,
persona definitions, git identity. Those are trust-*neutral* primitives —
a commons and a market can share them — which is exactly why the §VIII
standards play works: we standardize the shared identity vocabulary with
Block, and build the verification-and-settlement layer they structurally do
not need.

One more consequence worth naming: a Buzz community is not a competitor to
the OpenAgents market — it is a natural *cluster of counterparties* on it.
A community that trusts internally still needs verification the moment it
buys from or sells to the outside. If OpenAgents' rails are the standard
ones, every Buzz workspace is a prospective market participant the day it
wants work done by someone it doesn't already know.

## IV. Our actual position: closer to protocol-complete than the roadmap admits

The roadmap treats Nostr as a parked lane. The code says something different.

### IV.1 `nostr-effect` is a near-complete dual implementation

At commit `c160378`, `nostr-effect` (373 source files, 146 test files, ~90
subpath exports) implements:

- **Essentially the full standard catalog** — NIP-01 through NIP-99 plus the
  lettered drafts, as typed Effect services, wire-format wrappers, or both.
- **Both sides of the wire.** A full relay implementation (message handling,
  subscription management, filter matching, NIP-42 auth, a pluggable
  28-module NIP registry, policy pipeline, negentropy sync, NIP-86 admin)
  with Bun and Cloudflare Durable Object backends — plus a multi-relay client
  pool. Almost nobody in the ecosystem holds a maintained client *and* relay
  in one typed codebase. This matters enormously for §VIII: the NIP acceptance
  bar is "two clients and one relay."
- **The complete payments stack.** NIP-57 zaps with the validation appendices
  and zap splits, NIP-47 Nostr Wallet Connect including hold invoices, NIP-60
  Cashu wallets, NIP-61 nutzaps, NIP-87 mint discovery, NIP-75 zap goals.
- **All fifteen Buzz custom NIPs**, tested, on shared NIP-44/Schnorr/OA
  crypto primitives — twelve as full services, two as verifying readers for
  relay-signed projections (CW, DV), one as wire format only (PL).
- **A real signer boundary.** `LocalSignerPort` exposes `getPublicKey`,
  `signEvent`, NIP-44 encrypt/decrypt, and NIP-98 tokens while structurally
  refusing to export mnemonic, nsec, or raw key material; NIP-46 remote
  signing (bunker and nostrconnect flows), NIP-49 encrypted keys, NIP-55
  Android signer, and NIP-07 browser signing are all present.
- **NIP-34 git plus GRASP.** Repository, patch, PR, issue, and status kinds,
  and the kind 10317 GRASP server list — which Buzz itself does not register.
- **Seven OpenAgents-authored draft NIPs already in the tree**: SA (Sovereign
  Agents), AC (Agent Credit), SKL (Skills), TRN (Training Network), LBR
  (Agentic Labor), DS (Datasets), SB (Remote Sandbox).

### IV.2 The product-side assets are drafted, tested, and parked

- `packages/nip90` is a thin, disciplined layer over `nostr-effect/nip90`:
  labor kinds 5934/5935/5936 (code/review/document) with results 6934–6936,
  NIP-LBR decode-time rejection of raw prompts/paths/credentials, provider
  bonds (`provider_bond`/`bond_release`/`bond_forfeit`, ref-only, integer
  msat, XOR terminal), NIP-DS dataset kinds, and the `lbr-closeout`
  content-addressed receipt that composes request + quote + acceptance +
  result into one public-safe object. It deliberately moves no sats.
- `docs/nips/LBR.md` carries the full loop design — and the explicit marker
  `STATUS (2026-07-08): POSTPONED`.
- The web trust-surface spec holds the market vision; the desktop spec defers
  "seed-derived Nostr/Lightning identity" to a later Pylon contract; the
  mobile spec requires an owned E2EE relay for reachability.

### IV.3 The gaps that actually matter

Honest inventory of what does not exist:

1. **No sovereign-signer ProductSpec.** The signer boundary exists as library
   code (`LocalSignerPort`, issue #9092 lineage) but no spec assigns keys to
   principals, defines custody/rotation/recovery, or wires the signer into
   product surfaces. This is the single most load-bearing gap.
2. **No production relay home.** The relay backends are Bun and Cloudflare
   Durable Objects. Cloudflare is retired for OpenAgents production by owner
   mandate; Google Cloud is the sole infrastructure authority. An owned relay
   needs a Cloud Run (or GCE) deployment design. The Bun backend is the
   natural seed; the retired `apps/nostr-relay` must not be resurrected as-is.
3. **No projection outbox.** Nothing today deterministically converts
   canonical Cloud SQL writes into signed events.
4. **Depth is uneven.** Roughly ninety NIPs of coverage necessarily means
   many modules are codecs rather than hardened services; `AUDIT-NIP-EFFECT-
   COVERAGE.md` in that repo already flags Effect-harness test gaps. Parity
   claims need per-NIP depth review before any public interop claim.
5. **Nothing is wired into product.** No supported surface signs, publishes,
   or reads a Nostr event today.

The net position: the protocol layer is ~90% built and 0% deployed. Under the
zero-software-cost assumption, gap 4 and gap 5 are cheap; gaps 1–3 are design
decisions, which is exactly what this document is for.

## V. The building blocks: reading the standard catalog with intent

The full survey is long; what follows is the part that changes decisions.

### V.1 The deprecations that redirect us

- **NIP-90 (Data Vending Machines) is officially unrecommended upstream** —
  the README's words are that it "got totally out of control," with guidance
  to prefer use-case-specific microstandards. NIP-15 (full marketplace) is
  likewise deprecated as too complex, in favor of NIP-99 classified listings.
  This matters directly: our `packages/nip90` labor kinds live in the 5000er
  DVM namespace. Buzz independently avoided DVM kinds for its job protocol
  (its `kind.rs` comment cites bounded auth-chain requirements). The
  ecosystem's endorsed direction — tight, single-purpose kind pairs — is
  exactly what NIP-LBR already is in spirit. The consequence is not "abandon
  the work"; it is **"reframe NIP-LBR as the agentic-labor microstandard,
  registered in its own right, rather than as a NIP-90 extension."** Keeping
  wire compatibility with DVM-era tooling can be a compatibility note, not
  the identity of the spec.
- **NIP-04, NIP-26, NIP-28, NIP-72 are deprecated** — modern replacements
  (NIP-17/44/59 for privacy, NIP-29 for groups) are all in `nostr-effect`
  and all in Buzz. No friction.

### V.2 The economic primitives nobody has assembled

The catalog contains, scattered and unassembled, every piece of an agent
labor market:

| Primitive | NIP | Why it matters for agent labor |
| --- | --- | --- |
| Service listing | NIP-99 (30402) | Offers with structured price tags, including recurring `frequency` — agent retainers, not just spot jobs |
| Capability discovery | NIP-89 (31989/31990) | Agents advertise handled kinds; buyers discover through their follow graph — social filtering of providers for free |
| Competitive bidding | NIP-15's auction machinery (1021/1022) | Deprecated as a whole, but its bid/confirm/trust-gate state machine is the best-specified auction reference to mine |
| Payment signal | NIP-57 (9734/9735) | Zap receipts as settlement signals; **zap splits** (weighted `zap` tags) are a native revenue-share: agent/platform/referrer division at the protocol layer |
| Programmatic wallets | NIP-47 | Scoped, budgeted, revocable wallet connections for autonomous agents — and **hold invoices**: lock on job start, settle on delivery, cancel on failure. The escrow primitive the DVM era never had |
| Bearer balances | NIP-60/61 | Relay-synced encrypted Cashu wallets; nutzaps where **the payment is the receipt**, offline-verifiable via DLEQ — an observer can audit an agent's earnings without trusting any server |
| Reputation | NIP-32 (labels), NIP-58 (badges), NIP-85 (trusted assertions), NIP-39 (external identity proofs) | Skill ontologies, non-transferable credentials, third-party attestations — and cryptographic GitHub-account proofs, which for a *coding*-agent market imports the exact reputation that matters |
| Spam pricing | NIP-13 (PoW) | Delegatable proof-of-work on job requests and registrations |
| Presence/rooms | NIP-53, NIP-38 | Live sessions with role tags and proof-of-agreement participant signatures; expiring agent status ("busy on job X") |
| Sync | NIP-77 (negentropy) | Efficient set reconciliation between our relay, Buzz relays, and public relays |
| Git | NIP-34 + GRASP (10317) + NIP-GS | Issues, patches, PRs, status, decentralized repo hosting, and Nostr-keyed commit signatures — the coding-agent work loop as portable events |

The assembled system — listings + discovery + bidding + hold-invoice escrow +
zap-split settlement + nutzap receipts + labeled reputation + PoW admission —
does not exist anywhere in the ecosystem today. Every piece is a draft or
stable NIP with at least partial `nostr-effect` support. This assembly is the
"immediate functionality" the owner directive points at, and it is also the
thing no one else (including Block) is positioned to ship, because no one
else holds the payments stack, the relay, the client, and a live agent fleet
in one place.

### V.3 The kind-space facts that constrain leadership

- NIP-01 defines semantics only up to 39999. **Everything at kind ≥ 40000 is
  semantically unclassified** — no defined storage/replaceability behavior.
  Most of Buzz's registry (forum 45001–45003, jobs 43001–43006, workflows
  46001–46031, DMs 41001–41012, huddles 48100+, and more) lives in that
  undefined zone and therefore cannot be standardized upstream as-is.
- Buzz has real collisions: kind 9041 (`KIND_MODERATION_UNBAN`) collides with
  NIP-75 zap goals; kinds 39005/39006 (channel-window overlays) sit inside
  NIP-29's reserved 39000–39009 group-metadata block. Their NIP-IA kinds
  (8002/8003/13535) nestle adjacent to NIP-43's registered kinds by design.
- The registration path is the external `registry-of-kinds` repo plus the
  informal acceptance bar: **implemented in at least two clients and one
  relay**, optional, backwards-compatible, and not duplicative.

These constraints are opportunities in disguise: whoever shows up with clean
kind allocations, collision fixes, and dual implementations gets to define
the agentic register (§VIII).

## VI. The architecture: authority follows party count

The Buzz teardown ended at "signed projection bus" as the safe middle
posture: Cloud SQL stays authoritative for everything; Nostr carries signed
projections. That was the right conservative answer to "should we re-platform
our product on a relay." The owner's question here is different — what is the
*right* architecture if software is free — and the projection-only answer is
too timid for one specific domain, while relay-maximalism is wrong for
another. The principle that separates them:

**Let the number of mutually distrusting parties in a record decide where its
authority lives.**

- **Single-party state** — a Full Auto run registry, session checkpoints,
  private telemetry, owner memory, harness config. One owner, no
  counterparty. Putting authority for this on a relay buys nothing (the owner
  already trusts their own store) and costs plenty: ordering ambiguity
  (`created_at` is not a dense version), unverifiable deletion, metadata
  leakage, dual-write hazards. Verdict of the teardown stands: **owned
  authority, signed projections outward** where visibility helps.
- **Multi-party state** — an offer, a quote, an acceptance, a bond, a
  closeout receipt, a review approval, a reputation assertion, a patch and
  its merge outcome. Here the counterparty relationship inverts the
  argument: a provider should not have to trust the OpenAgents database to
  prove it was promised payment; a buyer should not have to trust it to
  prove what was delivered; a third party evaluating an agent should not
  have to ask our API's permission. For these records, **the signed event is
  not a projection of the truth — it is the truth**, and our database
  becomes the cache/index. This is also what makes the market *a market*
  rather than a platform with an API: participants can verify, replicate,
  and exit.

Two corollaries keep this honest:

1. **A signature is never authority.** A valid signed job request proves who
   asked; it does not prove admission, budget, policy compliance, execution,
   acceptance, or payment. Every inbound event crosses an Effect Schema
   boundary and an explicit admission gate, and every state-changing outcome
   gets its own signed result event. (This is the teardown's admitted-input
   rule, unchanged.)
2. **Settlement finality stays where the money is.** Lightning preimages and
   Cashu proofs are the settlement facts; Nostr events carry commitments,
   references, and receipts of them. The platform ledger remains the
   authority for platform credits exactly as `docs/nips/LBR.md` already
   specifies. Nothing in this pivot creates custody.

This principle also resolves the apparent conflict with the landed teardown:
the teardown rejected the relay event log as authority *for our product
surfaces* (chat, sessions, receipts of our own runs) — single-party state.
It did not, and this document does not, put those on a relay. The market
plane was never evaluated there as an authority candidate; under the party-
count principle it is the one place relay-native authority is correct.

## VII. The five-plane target architecture

The Nostr-native OpenAgents, drawn as planes with explicit authority labels.

```text
                      ┌────────────────────────────────────────────┐
   Plane 4 RELAYS     │  relay.openagents.com (GCP, owned policy)  │
   transport/policy   │  + Buzz relays + public relays (negentropy)│
                      └────────────────┬───────────────────────────┘
                                       │ signed events both ways
   Plane 3 WORKSPACE   NIP-29 groups · NIP-34/GS git · forum kinds
   admitted input      NIP-53 rooms · Buzz-profile interop
                                       │
   Plane 2 MARKET      NIP-LBR jobs · NIP-99 listings · NIP-89 discovery
   Nostr-authoritative NIP-57 splits · NIP-47 hold-invoice escrow
                       NIP-60/61 wallets/receipts · NIP-32/58/85/39 reputation
                                       │
   Plane 1 RECORDS     Cloud SQL / Khala Sync canonical (single-party)
   owned + projected   transactional outbox → signed projections
                       NIP-AE memory · NIP-AM metrics · NIP-AO telemetry
                                       │
   Plane 0 IDENTITY    sovereign signer (LocalSignerPort · NIP-46 · NIP-49)
   keys for everyone   NIP-OA owner attestation · NIP-AB pairing · NIP-05
```

### Plane 0 — Identity: a keypair for every principal

Every user, agent, device, and workspace gets a secp256k1 identity behind the
sovereign signer boundary. Concretely:

- **Humans**: key generated or imported into the platform vault; NIP-49 for
  encrypted backup; NIP-05 handles under `openagents.com`
  (`chris@openagents.com`, `agent-name@openagents.com`) for readable identity
  and org attestation; NIP-39 external-identity proofs to bind GitHub.
- **Agents**: keys minted per agent via the signer service and **never
  exported** — agents sign through `LocalSignerPort` or NIP-46 with
  per-kind-scoped permissions (`sign_event:5934`, …), so a compromised
  execution host cannot exfiltrate identity. NIP-OA owner attestations bind
  agent keys to their owner with time- and kind-bounded conditions; NIP-AA
  derives relay access from owner membership; NIP-AB moves keys between
  owner devices without pasting an nsec.
- **The org**: `_@openagents.com` as the root identifier; platform events
  (registry attestations, badge issuance) signed by a platform key with
  published rotation policy.

This plane is the missing ProductSpec named in §IV.3 and is prerequisite to
everything else. It also finally gives the desktop spec's deferred
"seed-derived Nostr/Lightning identity" a home, and it is the natural spine
for the Claim-Your-Agent tweet-first flow (the tweet binds a pubkey, not a
platform row).

### Plane 1 — Records: owned authority, signed projections, encrypted memory

Cloud SQL and Khala Sync remain canonical for single-party product state — no
change to settled architecture, no dual-write. What is added:

- **A transactional outbox**: deterministic post-commit projection of
  selected records into signed events — public-safe ones signed by the
  platform key (promise transitions, tokens-served attestations, release
  announcements), owner-private ones NIP-44-encrypted (session summaries,
  run outcomes). Relay publication is retryable and non-authoritative; a
  relay outage delays visibility, never correctness.
- **The Buzz agent-state vocabulary adopted at the boundary**: NIP-AE
  engrams for durable agent memory (the owner-decryptable-by-construction
  invariant the teardown already flagged as the sharpest idea in Buzz —
  stated as a contract on `packages/agent-experience-memory`), NIP-AM for
  per-turn metrics projections, NIP-AO for live telemetry streams. These are
  *projection formats*, not new authorities: token accounting remains
  exact-only in `token_usage_events`; an AM event must never become billing
  authority.
- **Portable sessions get portable names**: a session's repository is a
  NIP-34 address (`30617:<pubkey>:<repo-id>`) plus GRASP list, its
  checkpoints are content digests referenced from signed events, its
  identity is owner-minted — which is exactly the host-independent identity
  the portable-sessions spec (PORT-00..08) already demands, now with a wire
  format other ecosystems can read.
- **Mobile reachability**: the mobile spec's owned E2EE courier requirement
  maps cleanly onto NIP-17 gift-wrap over the owned relay plus NIP-PL push
  leases (whose non-amplification design is genuinely good); this replaces a
  bespoke courier protocol with one we can standardize.

### Plane 2 — Market: the Nostr-authoritative economy (the pivot's core)

The full lifecycle, every artifact a signed event on the owned relay
(mirrored wherever participants want):

1. **Presence**: provider publishes NIP-99 listing (30402) and NIP-89 handler
   (31990) declaring handled LBR kinds; agent card composes AP persona + OA
   owner proof + NIP-58 badges + NIP-39 GitHub proof + verifiable nutzap
   earnings history.
2. **Request**: buyer publishes an LBR job request (public-safe objective,
   budget, required confidence tier, PoW-stamped). A `request_admitted`
   platform event answers it — signature ≠ admission (§VI).
3. **Bids**: providers respond with quotes (the NIP-15 auction state machine,
   reborn as LBR feedback semantics); buyer accepts one, producing a signed
   acceptance that names the quote by id.
4. **Escrow**: buyer's wallet (NIP-47) opens a **hold invoice** for the
   quote; the payment hash is referenced in a signed bond event. Provider
   bonds post the same way (`provider_bond`, already specified ref-only in
   `packages/nip90`). No custody: the hold sits in the buyer's own wallet
   infrastructure.
5. **Execution**: provider runs with its own credentials (LBR rule); progress
   as feedback events; for coding labor the work product flows through
   Plane 3 as NIP-34 patches with NIP-GS-signed commits.
6. **Delivery and settlement**: output-only result event with artifact
   digests; buyer (or the verification tier) accepts; hold invoice settles;
   the preimage lands in the **closeout receipt** — the `lbr-closeout`
   content-addressed object we already ship, now referenced from a signed
   event. Zap splits route the platform take and referral share in the same
   settlement.
7. **Reputation**: both sides may publish NIP-32 labels scoped to an
   OpenAgents namespace; the platform issues NIP-58 badges for verified
   tiers; NIP-85 trusted assertions let third-party verifiers attest
   outcomes. Nutzap receipts make earnings independently auditable.

Pylon is the provider daemon of this plane — its heartbeat capacity refs
become signed capacity events, its "run a Pylon" seller path becomes the
onboarding, and the Buzz Mesh contrast becomes our pitch: their community
compute is a commons inside one membership; ours is a market across all of
them, and the two interoperate at the protocol layer because both speak
Nostr.

### Plane 3 — Workspace: admitted collaboration input, Buzz interop

Where OpenAgents surfaces meet other people's clients:

- **Git**: adopt NIP-34 + GRASP + NIP-GS per the teardown's §7.9 profile —
  refs/object-store pointer authoritative, kind 30618 as signed projection
  after the ref commit, patches/PRs/issues as admitted proposals, a separate
  signed merge outcome. Test against ngit and gitworkshop, not just Buzz.
- **Forum**: keep the live Forum authoritative in Cloud SQL (single-party
  moderation authority, per §VI), but publish posts as signed projections and
  accept signed posts as admitted input — which makes the Forum readable and
  writable from any Nostr client and gives the money-moderated ranking a
  zap-native implementation. NIP-7D (kind 11) and Buzz's 45001–45003 forum
  kinds diverge here; this is a §VIII harmonization target.
- **Rooms and presence**: NIP-53 for live sessions (its proof-of-agreement
  participant signature solves "don't list my agent without consent"),
  NIP-38 expiring statuses for agent liveness.
- **Buzz communities**: an OpenAgents agent should be able to join a Buzz
  workspace as a member — auth via NIP-42+AA, memory via AE, metrics via AM,
  git via their Smart HTTP with NIP-98 — with its OpenAgents reputation
  attached. That is the concrete, demo-able meaning of "sync up with Block."

### Plane 4 — Relays: owned policy node plus federation

- **`relay.openagents.com`** on Google Cloud (Cloud Run service or GCE,
  seeded from the `nostr-effect` Bun backend with Cloud SQL-backed storage;
  the Cloudflare DO backend stays a library capability for others, never our
  production). Policy: NIP-42 auth for writes, the market/workspace kind
  registry, PoW thresholds, NIP-86 admin, NIP-56 report intake that queues
  and never auto-actions (Buzz's moderation posture, which matches our
  Forum-first report doctrine).
- **Federation posture**: market events mirror to public relays
  (censorship-resistance for the order book is a feature, not a risk);
  owner-encrypted records pin to the owned relay plus the owner's NIP-65
  list; NIP-77 negentropy reconciles with Buzz and public relays. The
  explicit non-goal: requiring any third-party relay for correctness (the
  mobile spec's rule, kept).

## VIII. The standards play: leading on agentic NIPs

This is the highest-leverage part of the pivot and the part with a genuine
time window.

### VIII.1 The arithmetic of the acceptance bar

The NIPs repo's acceptance criterion is running code: **two clients and one
relay**, optionality, and non-duplication. Today, agentic-workspace NIPs have
exactly one implementer (Buzz) and agentic-labor NIPs have zero live
deployments. `nostr-effect` is already the second implementation of all
fifteen Buzz NIPs — and it is also a relay. **OpenAgents plus Block is, by
itself, the entire acceptance bar for an agentic NIP suite.** Whoever
convenes that convergence writes the register that every subsequent agent
platform inherits. Four and a half months into Buzz's life, with their NIPs
still marked draft and their kind registry carrying fixable collisions, the
window is open. It will not stay open: 801 commits a month is a team that
will standardize with or without us.

### VIII.2 The concrete program

1. **Publish our drafts.** Move SA/AC/SKL/TRN/LBR/DS/SB from `docs/nips/`
   into a public `OpenAgentsInc/anips` (or similar) spec repo with test
   vectors, mirroring Buzz's `docs/nips/` convention, CC0 like upstream.
   Register every kind in `registry-of-kinds`.
2. **Bring Buzz fixable gifts, not critiques.** File the 9041/NIP-75 and
   39005-6/NIP-29 collisions upstream with proposed re-allocations; propose
   a 40000+ range convention to NIP-01 (their registry needs it more than
   anyone's); contribute the failing all-zero-pubkey NIP-GS parser fix the
   teardown found. Arriving as the second implementation that makes their
   drafts standardizable is the credibility position.
3. **Harmonize the overlaps.** Their 43001–43006 job kinds vs our LBR; their
   45001–45003 forum kinds vs NIP-7D; their AP personas vs our SA/SKL; their
   AM cost telemetry vs our AC credit semantics. Each pair should end as one
   spec with two implementations — ours brings the price, escrow, bond, and
   receipt semantics theirs lack.
4. **Champion the money layer.** Propose the agentic-labor microstandard
   (LBR reborn per §V.1) with hold-invoice escrow choreography, zap-split
   fee semantics, nutzap earning receipts, and the closeout receipt format
   — the layer where we are the natural editor because we ship the only
   dual-side implementation.
5. **Stand up the interop matrix as CI.** Continuous conformance runs:
   nostr-effect client ↔ Buzz relay, Buzz desktop ↔ our relay, ngit ↔ our
   GRASP list, NWC against real wallets, vector suites per NIP. Interop
   claims only from green matrix runs — promise-registry discipline applied
   to protocol claims.
6. **Dogfood in public.** The OpenAgents repo itself announced via NIP-34 on
   our relay, agents' commits NIP-GS-signed, releases zapped — the
   `VISION_SOVEREIGN` story, performed by us, with money in it.

### VIII.3 Why lead at all

Because the alternative is inheriting someone else's register. The Buzz NIPs
already encode Buzz server policy as implicit protocol (host-derived tenancy,
relay-only kinds, their trust rules). If the agent economy standardizes on a
workspace vocabulary with no market vocabulary, the market layer gets built
later, worse, by whoever shows up — or gets built as a proprietary API and
the decentralization premise dies at the payment boundary. Our whole thesis
(machine-work economy, verifiable outcomes, Bitcoin rails) needs the market
layer to be *protocol*, and nobody else is incented to write it.

## IX. What "overnight" buys — and what it cannot

Under the zero-software-cost assumption, the following are genuinely fast:
wiring the signer service into surfaces, the outbox, deploying the relay,
promoting `packages/nip90` into live (testnet-bounded) flows, the interop
matrix, publishing specs. The teardown's protocol-vs-product distinction
collapses when implementation is free — **but four costs are not software
and do not go to zero:**

1. **Key custody is a human problem.** "Losing your private key means losing
   your identity" (`VISION_SOVEREIGN.md`, honestly). Recovery ceremonies,
   rotation policy, the owner's own custody practices — these are design
   and operational decisions with irreversible failure modes. NIP-46/49/AB
   shrink the problem; nothing deletes it.
2. **Authority discipline is a governance problem.** Every plane boundary in
   §VII is a rule someone can violate in a hurry ("just read it off the
   relay"). Dual-authority drift is the classic failure of exactly this
   pivot; the invariant ledgers and admission gates have to move in the
   same change as the wiring, and that is review capacity, not typing speed.
3. **Spam, moderation, and liability arrive with the first open port.** An
   authenticated-write owned relay with PoW and report queues bounds this,
   but market surfaces attract adversaries at the speed of money, not the
   speed of software.
4. **Standards move at the speed of other people.** The two-client bar we
   can meet overnight; rough consensus we cannot. Budget months of upstream
   conversation regardless of how fast the code lands.

And one cost is strategic: **focus.** The active flagship is Full Auto
(#8979 gating #8967), and the roadmap's non-revival boundaries exist for
reasons. The resolution is not to pretend there is no tension. It is that
the pivot's first deliverables (signer spec, outbox, relay, market pilot)
are *additive infrastructure* that does not touch the Full Auto lane, and
Full Auto itself becomes the market's first supply: a Full Auto run whose
closeout is a signed, verifiable receipt is precisely an "accepted outcome"
the market can price. The verifiable-software thesis and the market thesis
are one thesis at the receipt boundary.

## X. A staged candidate program

Candidates for Sol admission, not dispatch. Ordered so every stage is
independently valuable and no stage creates dual authority.

- **N0 — Sovereign signer ProductSpec + service.** Principals, custody,
  rotation, recovery, per-kind agent scoping, NIP-05 namespace, NIP-OA
  issuance. (Everything depends on this.)
- **N1 — Owned relay on Google Cloud.** `nostr-effect` relay core, Cloud
  SQL-backed store, authenticated writes, kind policy, NIP-86 admin, NIP-77
  sync. Pin a reviewed `nostr-effect` revision with per-NIP depth audit.
- **N2 — Projection outbox.** Platform-signed public projections first
  (promise transitions, tokens-served, releases); owner-encrypted AE/AM/AO
  projections second. Explicitly non-authoritative.
- **N3 — Identity in product.** Agent cards (AP+OA+badges+NIP-39), NIP-AB
  device pairing, Claim-Your-Agent on pubkeys, `@openagents.com` handles.
- **N4 — Market pilot, one lane.** LBR code-labor loop end-to-end on the
  owned relay with hold-invoice escrow on test capacity, closeout receipts
  live, Pylon as provider daemon. Bounded budget, owner gate on real sats
  (the LBR live-run gate stands).
- **N5 — Git profile.** NIP-34/GRASP/GS per teardown §7.9; our repo
  announced; agent commits signed; ngit interop proven.
- **N6 — Standards program.** Spec repo, registry entries, Buzz upstream
  contributions, harmonization proposals, public interop matrix. (Runs in
  parallel with N1–N5; listed last only because its artifacts cite theirs.)
- **N7 — Workspace interop.** OpenAgents agents joining Buzz communities;
  Forum projections/admitted input; NIP-53 rooms. The demo that makes the
  partnership conversation concrete.

## XI. Honest counterarguments

- **"The teardown said don't re-platform on the relay."** It did, for our
  single-party product surfaces, and this document keeps that verdict. The
  party-count principle is the disciplined boundary the teardown's four-
  posture table gestured at; the market plane was never the thing it argued
  against. If a future owner decision wants relay authority for product
  chat/sessions, that is a new ProductSpec per the teardown — unchanged.
- **"NIP-90 is deprecated; you are building on sand."** We are building on
  its lesson: microstandards. LBR as its own registered spec is aligned
  with, not against, upstream direction. The DVM kinds remain a compat note.
- **"Block could swallow the register."** Yes — if we stay parked. The
  counter is being the second implementation and the money layer editor
  before their drafts ossify. Apache-2.0 code, CC0 specs, and public-domain
  NIPs make embrace-and-extend hard to sustain against a live interop
  matrix.
- **"Nostr's culture may reject corporate agentic NIPs."** Possible; the
  registry path (external, YAML, permissionless) doesn't require the NIPs
  repo's blessing to function. De-facto standardization through running code
  is Way (a) in their own README.
- **"Key custody UX will hurt onboarding."** It will (Buzz says so too). The
  mitigation is that platform-vault custody with NIP-46 scoping gives
  normal-user UX while keeping exit rights — sovereignty as an option, not
  an entry toll.
- **"This distracts from Full Auto."** §IX's answer: additive planes, and
  Full Auto receipts are the market's first product. If the owner weighs
  focus above the window, N0+N6 alone (signer + standards) preserve the
  leadership position at minimal surface area.

## XII. Recommendation

Adopt the five-plane architecture as the target picture, with the party-count
principle as the invariant that keeps it honest. Sequence N0 (sovereign
signer spec) and N6 (publish the drafts, engage Buzz upstream) immediately —
they are cheap, reversible, and hold the window open. Then N1–N4 as bounded
Sol packets: relay, outbox, identity, one market lane with real escrow
semantics on test capacity.

The deep opportunity is not that Buzz exists. It is that the agentic Nostr
stack currently has exactly one serious implementer, no economic layer, and
an acceptance bar we can satisfy by showing up. OpenAgents holds the only
dual-side (client + relay) typed implementation, the only drafted labor/
credit/skills/dataset NIP family, the payments stack, the provider fleet
design, and a verification thesis that turns "work happened" into "outcome
proven" — which is the only thing a machine-labor market can actually price.
Sync with Block on identity, memory, and workspace vocabulary; lead on the
market; and let the building blocks compound.

Stated in the vision's own terms: Buzz built the room where agents work.
OpenAgents is building the refinery that turns electrons into accepted,
paid-for outcomes — and the receipts that let strangers trust them. The
rooms and the refinery share a protocol, and that is the entire opportunity:
adopt the room vocabulary, standardize it together, and make sure the
economy that runs through every room settles on rails we lead.
