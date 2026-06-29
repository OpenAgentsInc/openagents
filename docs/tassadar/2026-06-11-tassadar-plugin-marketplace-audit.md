# The Store We Built Twice: Tassadar and the Agent Marketplace

> Status: history audit plus speculative essay, 2026-06-11. The audit
> sections cite committed transcripts, deprecated-repo docs, and live
> psionic code; the essay sections are labeled speculative and claim
> nothing. Nothing in this document is a product promise, a served
> capability, or public claim copy. The Tassadar disclosure flow and the
> promises registry govern anything that ever becomes one. The starter
> plugin catalog's own boundary — "the catalog does not imply public
> plugin publication, arbitrary external plugin admission, or a public
> plugin marketplace" (psionic `docs/TASSADAR_STARTER_PLUGIN_CATALOG.md`)
> — remains in force throughout.

## I. Three Generations of One Idea

OpenAgents has now built the same idea twice and is, without quite
saying so, building it a third time. The idea: **a marketplace where
independently authored units of machine capability are listed,
discovered, composed into agents, metered per use, and paid for in
Bitcoin, with revenue flowing automatically to everyone whose component
did work.**

- **Generation one (2024)** was the agent store and paid plugins —
  episodes [`048`](../transcripts/048.md) through
  [`102`](../transcripts/102.md) of the video record. WASM plugins,
  a registry, an agent store modeled on the GPT Store "except we
  actually have payments," per-minute Lightning payouts, and a
  60/20/20 revenue split that paid plugin authors every time their
  code ran inside anyone's agent. It shipped. It also stalled, for
  reasons worth being precise about.
- **Generation two (2025–2026)** was Blueprint — the typed business
  operating layer descended from DSPy, with Program Types, Signatures,
  Module Versions, Program Runs, Optimizer Runs, Source Authority, and
  release gates. Episode [`211`](../transcripts/211.md) states the
  marketplace form of it directly: "DSPy in Effect... independently
  discoverable signatures monetized with Bitcoin connected to an open
  protocol marketplace." Blueprint was sunset as a separate service on
  2026-05-24 and its primitives absorbed into the product surface as
  source material.
- **Generation three** is what the Tassadar program makes possible:
  capability as a **digest-pinned compiled artifact whose execution is
  its own receipt**, verifiable by replay before a purchase clears.

The thesis of this audit is that each generation supplied exactly what
the previous one was missing, and that the missing piece was never the
same piece twice. Generation one had payments without proofs.
Generation two had contracts without a floor. Generation three has the
floor — and should therefore inherit the other two layers rather than
reinvent them.

## II. Generation One: The Agent Store (2024)

### What shipped

The record is in the transcripts, and it is more complete than memory
suggests.

**The plugin substrate (episodes 048–075).** Extism-based WASM plugins
compiled from any of eight languages, loaded as agent extension units,
with capability-based host functions (a plugin got database or
environment access only by explicit host binding). The registry was
designed to be "a registry of plugins, which will itself be a plugin"
(048). By episode 066 a contributor had built a Nostr-based plugin
registry — plugins published as Nostr events (a kind-3514 NIP was
drafted), discoverable on any relay, deliberately not captive to
openagents.com: "there's no reason that you should have to get that
from my company... this more open kind of substrate of communication,
enabling interoperable code and business logic... could be written to
and read by... tens or hundreds of thousands of different
applications" (066). L402 payment-gated plugin deployment landed in
070.

**The store (episodes 085–102).** Episode 085 unified the API and the
marketplace — agents built from composable nodes, "each node may have
an associated fee payable to its creator upon use," paid in "the
native currency of the internet, which can be sent in extremely small
denominations anywhere in the world instantly for essentially no fee."
Episode 092 launched the open beta: user-created agents, public
discovery, Lightning-address payouts ("like an email address, but for
your Bitcoin"), and the explicit competitive frame — OpenAI had
announced GPT Store monetization six months earlier and "from what we
can tell zero people have been paid... we have shipped essentially the
same stuff... except we actually have payments." Episode 093 recorded
the first payout: 100,000 sats across four agent builders. Episode 098
moved payouts to **once per minute** and fixed the split: 80% to the
agent creator and 20% to the platform, or — when plugins were used —
60% to the creator, 20% to the platform, and **20% split among the
authors of every plugin that executed in that message**. Episode 102
shipped the plugin marketplace UI: creator-set pricing in sats per
invocation, secrets and environment injection, input/output templates,
and a review queue, with roughly twenty developer-submitted plugins.

That is, concretely: an open plugin format, a decentralization-capable
registry design, per-component metering, author revenue share, and
sub-minute permissionless settlement — in production, in 2024.

### Why it stalled

The transcripts are candid, and the candor is the valuable part.

1. **The quality problem.** Episode 100, verbatim: "we don't have
   agents that are like really compelling that you want to pay for...
   we kind of have to solve that for this to be worth people paying."
   The store was a beautiful settlement layer over goods that were
   prompt templates with RAG files. The unit of trade could not carry
   value commensurate with its rails.
2. **The trust problem.** Nothing in the system could *verify* what a
   plugin did. A WASM blob was opaque; an agent was instructions. The
   review queue (episode 102) was manual, which meant admission was a
   bottleneck and trust was a curator's opinion. Pricing had nothing
   to ground against: a sat-per-invocation number was a guess, not a
   function of any measurable property of the good.
3. **The decentralization deferral.** The Nostr registry was
   deprioritized "while we get our own kind of UX nailed" (066) and
   the event structure never became a NIP. The open-protocol layer
   stayed an intention.
4. **The split outran the substrate.** Per-minute multi-party revenue
   splits (creator, plugin authors, platform, referrers) required the
   system to know *whose component did what work in which message* —
   and the runtime's account of that was bookkeeping, not evidence.

Compress those four and you get the diagnosis this audit turns on:
**the 2024 store had payments without proofs.** Settlement was
real-time and permissionless; the thing being settled was unverifiable.
Every failure above is a projection of that one gap. The platform
later articulated the general law — the bottleneck of a machine-work
economy is not producing work but verifiably producing it
([`work-that-proves-itself.md`](work-that-proves-itself.md)) — but the
store had run the experiment first, from the unverified side.

## III. Generation Two: Blueprint (2025–2026)

Blueprint attacked a different gap. Where the store's goods were
untyped and unverifiable, Blueprint made the unit of machine decision
a **typed, versioned, governed contract**:

- A **Program Type** was a versioned behavior contract (not a prompt),
  owning purpose, risk class, evidence requirements, and release gate.
- A **Program Signature** was the stable typed input/output schema —
  the DSPy inheritance, signatures as the durable interface.
- A **Module Version** was the swappable implementation artifact
  behind a signature: a deterministic reducer, a prompt, a fine-tune,
  a tool plan, an escalation to a coding agent — promotion-gated,
  never self-promoting.
- A **Program Run** was immutable decision evidence that could
  recommend but never write: "Program Runs are decision evidence. They
  do not authorize writes" (`blueprint/docs/programs-optimization-and-rlm.md`).
  Writes went through Action Submission, Source Authority, approval
  policy, and receipts.
- **Optimizer Runs** (GEPA/MIPRO-class) improved Module Versions
  offline, behind release gates.
- **App Manifests** and **MCP Agent Profiles** — deferred but designed
  — were the capability-listing layer: typed declarations of which
  programs, actions, and scopes a surface or external agent could see.

Episode 211 framed the marketplace consequence: the first chat surface
using DSPy-style primitives as first class, "independently
discoverable signatures monetized with Bitcoin connected to an open
protocol marketplace." The signature — not the agent, not the blob —
was now the discoverable, sellable unit.

Blueprint was deprecated as a standalone service on 2026-05-24 and its
kernel (Program Types, Signatures, Module Versions, Program Runs,
Action Submissions, Source Authority, Context Packs, release gates) is
being rebuilt natively in the product surface
(`apps/openagents.com/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`).
The retrospective on the DSPy era is exact about what mattered: "The
most valuable OpenAgents DSPy work was not 'LLM call wrappers.' It
was: manifests, promotion state, shadow mode, training example
capture, trace mining, receipts, compiled policy history"
(`products/2026-04-14-dspy-dsrs-gepa-rlm-forge-and-probe-audit.md`).

What generation two still lacked was a **floor**. Every guarantee in
Blueprint is governance-shaped: evals, scorecards, shadow mode,
promotion gates, human approval. Those are the right machinery for
statistical components — and they are *all the machinery there is*
when no component can prove its own behavior. A Module Version's claim
of correctness was graded, never proven. The release gate could demand
evidence; it could not demand a proof, because no implementation kind
in the catalog could produce one.

## IV. Generation Three: What Tassadar Changes

The Tassadar program ([`README.md`](README.md),
[`RESEARCH_PLAN.md`](RESEARCH_PLAN.md)) supplies precisely the missing
floor: a class of capability whose execution is deterministic,
digest-pinned, and **verifiable by replay** — where a validator's
verdict is a hash comparison, the cheapest verification grade that can
exist. A compiled weight module is, in marketplace terms, *a plugin
whose every invocation ships its own receipt.*

Put the three generations in one table and the convergence is hard to
miss:

| Marketplace concern | 2024 agent store | Blueprint | Tassadar lane (committed code) |
|---|---|---|---|
| Unit of trade | prompt-template agent; opaque WASM blob | typed Module Version behind a Program Signature | digest-pinned compiled bundle (Futamura-specialized weight module, portable numeric artifact — psionic #1100/#1113) |
| Interface | input/output templates (102) | Program Signature | typed packet schemas + typed refusals + negative claims (starter-plugin registry) |
| Execution substrate | Extism WASM, outside the model | runtime-of-choice under contract | the Tassadar Wasm window, *as transformer weights*, every step in the trace |
| Admission | manual review queue (102) | release gates, evals, shadow mode | conformance replay: five executor legs agreeing digest-for-digest; differential harness; exact trace-replay verdicts (#1106/#1107) |
| Discovery | central registry; Nostr kind-3514 draft (066) | "independently discoverable signatures" (211); App Manifest / MCP Agent Profile (deferred) | module manifest / catalog / linker family; `TassadarCapabilityEnvelope` (W4.1, openagents#4750) |
| Trust | curator opinion | promotion state + receipts | replay before purchase clears; capability declared only with self-test receipts |
| Pricing | creator-set sats/invocation, ungrounded | risk-classed governance | the verification ladder as pricing tiers (H6) |
| Revenue split | 60/20/20 declared by bookkeeping (098) | receipts link evidence | the trace itself decomposes who computed what |
| Settlement | per-minute Lightning sweeps (093/098) | (inherited product rails) | paid Lightning closeouts on the assignment route — live with receipts since 2026-06-10 |

Two observations about that table, one historical and one structural.

**Historical:** the 2024 store and the Tassadar lane chose the *same
instruction substrate* — WebAssembly — for unrelated reasons, a decade
of trust apart. The store ran Wasm blobs beside the model and could
not see inside them. Tassadar interprets a Wasm window *inside the
model*, and the execution is the output. The plugin format survived;
its opacity did not.

**Structural:** none of the right-hand column was built *for* a
marketplace. The starter-plugin runtime
(psionic `docs/TASSADAR_STARTER_PLUGIN_AUTHORING.md`) exists for
bounded internal computation: a central
`StarterPluginRegistration` carrying plugin id, versioned packet
schemas, typed refusal sets, replay class, capability class and
namespaces, origin class (`operator_builtin` vs `user_added`), mount
envelope, manifest and artifact ids, and explicit negative claims —
with a scaffold helper for the capability-free class and a deliberately
harder manual path for the networked class. Six plugins are cataloged,
each with descriptor, fixture bundle, and mount-envelope sidecar. The
module manifest / catalog / linker / package-manager family and the
plugin packet ABI + Rust PDK live beside it in `psionic-compiler`.
That is a store's entire back office — registration, typing, admission
classes, refusal contracts, artifact identity, catalog projection —
built under a no-marketplace boundary, for evidence reasons. Which is
the correct order: generation one built the storefront first and had
nothing provable to put on the shelves. Generation three built the
shelving, the labeling system, and the inspection bench first, and
keeps the storefront explicitly closed.

The settlement rail, meanwhile, stopped being hypothetical on
2026-06-10: a digest-pinned executor workload dispatched through the
production assignment route, replay-verified by a separate device
(tampered digest correctly Rejected), one paid closeout settled over
real Lightning (`compute.tassadar_executor_poc.v1`, green). The 2024
store's payment loop and the 2026 lane's proof loop have now each run
end-to-end — they have simply never run *together*. That conjunction
is the whole product.

## V. The Ecosystem (Speculative)

Everything in this section is labeled speculation, per the folder's
discipline. It describes what a plugin ecosystem *built around*
Tassadar would look like if the program's open hypotheses hold, and it
deliberately reuses the three generations' parts rather than inventing
new ones.

### The shelf structure is the verification ladder

The 2024 store had one shelf: "plugins," priced by author guess. The
organizing move for a Tassadar-era store is that **the marketplace's
shelf structure is the verification ladder itself** — a module's proof
class is as load-bearing as its function, and pricing follows the
ladder (hypothesis H6):

- **Tier E — exact.** Compiled weight modules: integer and fixed-point
  arithmetic, ledger state transitions, finite-state protocol
  validators, bounded parsers, checksum and digest kernels,
  scheduling/assignment kernels (the canonical Percepta demo is the
  Hungarian algorithm — literally the computation a work dispatcher
  runs), and interpreter slices. Every invocation emits a replayable
  trace; conformance is checked by replay before admission *and*
  before purchase clears; near-misses refuse rather than interpolate.
  The weakest devices in the contributor funnel validate this entire
  tier. Authoring, conformance-testing, and auditing these modules is
  itself CPU-bound, deterministic paid homework — the store's supply
  side generates the network's work.
- **Tier D — deterministic.** The existing
  `capability_free_local_deterministic` starter-plugin class:
  host-native deterministic code with typed packets and
  `deterministic_replayable` posture. Not weights, but the same
  receipt shape and the same mechanical admission. The six cataloged
  plugins are this tier's seed inventory.
- **Tier S — statistical.** Psion-class learned modules with bounded
  claims, sold with first-divergence histograms and eval-suite
  receipts instead of proofs, graded by the W3 harness. The naming
  rule is a *labeling law* here: Tier S goods carry Psion's claim
  vocabulary and may never borrow Tassadar's. A store that lets a
  learned module advertise exactness has recreated generation one's
  trust problem with better typography.
- **Tier N — effectful.** The `networked_read_only` class and beyond:
  mount-envelope-gated capabilities, snapshot-backed replay, and — for
  anything that writes — the full Blueprint discipline inherited
  intact: Source Authority, Action Submission, approval policy,
  Trust/Failure Receipts. Generation two's governance machinery is
  exactly right here, because this is the tier where proofs cannot
  reach and contracts must.

### The lifecycle, mechanized

The 2024 lifecycle was: upload → human review → listed → priced by
guess → invoked → split by bookkeeping. The Tassadar-era lifecycle
replaces every soft link with a mechanical one:

1. **Submit:** a module arrives as a digest-pinned artifact with its
   manifest — profile id, schemas, refusal set, negative claims,
   declared tier.
2. **Admit:** conformance replay against the reference suite. The
   review queue from episode 102 becomes a harness run; admission is a
   verdict, not an opinion. (New authors inherit W2's
   quarantine-before-admission posture wholesale.)
3. **List:** the catalog projection publishes the capability envelope
   — and, per the platform's hardest-won recent lesson, **rebuilds on
   validation transitions**, never on registration events. A store
   whose shelves disagree with its inventory is the
   projection-staleness defect class (#4744–#4746) wearing a price
   tag.
4. **Discover:** signatures published on open-protocol rails — the
   NIP-89/90-class discovery and NIP-DS sale flows the 2024 Nostr
   registry anticipated, plus the draft skills-registry shape — with
   selection done semantically (typed selectors and embeddings, per
   the workspace's no-keyword-routing rule), so an agent finds "exact
   fixed-point ledger transition, profile core_i32.v0.3" by meaning,
   not string match.
5. **Buy:** for Tier E, the purchase protocol is *replay before
   settlement* — the buyer (or any validator they hire for dust) replays
   the conformance trace against the candidate artifact and the digest
   either matches or it does not. Caveat emptor becomes caveat
   replicator.
6. **Split:** the revenue share returns, but grounded. Episode 098's
   60/20/20 was declared by the platform's bookkeeping; in a
   trace-native store, a job's receipt *decomposes* — the trace shows
   which module's steps ran in which spans — and the split is computed
   from the evidence rather than asserted over it. Multi-party
   settlement per message stops being an accounting promise and
   becomes an arithmetic consequence.

### Composition, and the organ market

The module linker is where the ecosystem stops being a parts catalog
and becomes a system. psionic has been preparing for module linking
(`tassadar_module_linker.rs`, cross-profile link compatibility) ahead
of any published research on composing specialized weight banks —
which means "agent assembled from purchased modules" has a concrete
technical referent: a planner (the H2 hybrid — trained control around
frozen exact cores) routing across a set of conformance-tested,
digest-pinned modules behind explicit ABI tokens.

If H2 and H4 hold, the store's deepest product appears: **modules as
organs**. A sold module is not only callable — it is *installable into
a model*, because a compiled module is a weights file and the
specializer's output format is designed so embedding-into-host is a
layout decision. Percepta's line — "future AI systems will not just
use software; they will contain it" — is, read commercially, a
description of a package manager for model organs. Generation one sold
agents; generation three's endgame sells the anatomy. And because
every organ is replayable, the validator economy scales with the
catalog: every module sold mints standing verification work for the
long tail of weak devices, which is the supply thesis
([`work-that-proves-itself.md`](work-that-proves-itself.md) §III)
closing its loop.

### What the buyer constitutionally cannot do

The demand floor is the same one identified in the business essay §IV:
the buyers are agents, and frontier-model agents fail at exact
computation *constitutionally*. Every agent in the economy currently
performs the write-code/pause/trust-the-sandbox ritual for arithmetic
it cannot check. A store shelf of exact modules — ledger transitions,
protocol validators, schedule solvers — is aimed at the one customer
class that demonstrably cannot make these goods itself and whose
purchase decision ("does the digest match?") is the cheapest diligence
in commerce. That demand-shape is hypothesis H6 and the §IV
composition argument; it is unproven at volume, and the kill
conditions below apply.

## VI. What Would Kill It

Stated plainly, because boundaries are the product.

- **Generation one's failure modes survive wherever proofs don't
  reach.** Tier S and Tier N goods are exactly as hard to trust, price,
  and review as 2024 plugins were; the ladder helps only if the
  high-trust tiers carry enough of the catalog's value. If the market
  concentrates in unprovable goods, this is the old store with better
  paperwork.
- **The shelf is nearly empty today.** The interpreted window is
  twelve opcodes and essentially one committed program-family shape.
  The catalog of *expressible* exact modules is bounded by W1's window
  ladder — which is precisely why the research plan made W1 the gate
  on everything. No window, no inventory, no store.
- **"Install into a model" is not yet real.** Dense materialization
  (W1.2) does not exist; modules today are scalar-lane numeric
  artifacts, not loadable checkpoint blocks. Module linking is open
  research. The organ market is two unproven hypotheses (H2, H4) deep.
- **Demand is one data point.** Exactly one paid closeout has settled
  for executor work. H6 needs volume, and the program's first kill
  condition — no buyer ever values trace-as-receipt over a raw CPU —
  would kill the store while leaving the factory standing.
- **The publication gates are closed, correctly.** The starter catalog
  says no public marketplace; the disclosure flow gates any widening;
  no registry promise covers any of this and none is warranted until a
  workstream produces evidence that needs one. This document changes
  none of that.
- **The sequencing trap.** Generation one's deepest error was building
  the storefront before the goods. The corrective is already encoded
  in the workstreams: W1 (window) → W2 (factory) → W4.2 (module
  library, conformance-tested, behind ABI tokens) → and only then
  listing/discovery/settlement surfaces. **The store is the last thing
  built this time, not the first.** Any future issue that proposes
  marketplace UI ahead of a conformance-tested module library should
  be read as the 2024 mistake attempting a comeback.

## VII. Where This Lands in the Filed Program

No new issues are filed by this audit; the focus directive
(`directive.owner.20260611.focus_tassadar_psion_cs336`) governs. But
the mapping is concrete:

- **W4.1 (openagents#4750)** — the `TassadarCapabilityEnvelope`
  consumer in Pylon — is the store's *supply-side honesty layer*:
  capacity advertised only with self-test receipts. It is already
  filed and is the natural first brick.
- **W4.2 (the module library)** is where the catalog's Tier E
  inventory comes from: small exact modules behind explicit ABI
  tokens with fixed schemas and replayable module traces. The
  marketplace adds nothing to W4.2 except eventual *listing* — the
  conformance machinery is identical.
- **W2's contract freeze (openagents#4748)** already specifies the
  admission discipline (quarantine, never-train-from-unverified,
  projection-rebuild rules) that a store would inherit verbatim for
  goods instead of traces.
- **The Blueprint kernel rebuild** in the product surface owns the
  contract layer (Signatures, Module Versions, release gates, Source
  Authority) that Tiers S and N require; the deferred App Manifest /
  MCP Agent Profile concepts are the listing format's closest
  existing ancestors.
- **The optimizer lane stays parked.** GEPA-class optimization of
  modules and routing is planned-by-directive; a future store's
  "self-improving inventory" story waits on owner re-prioritization,
  exactly as the registry records.
- **Candidate future issues** (named, not filed): a module
  conformance-before-admission flow generalizing the starter-plugin
  admission rule; a catalog projection with rebuild-on-transition; a
  per-trace revenue-decomposition spec reviving episode 098's split
  on evidence rails; an SKL/NIP-DS listing adapter reviving episode
  066's registry as an actual NIP. All four are post-W4, owner-gated,
  and cheap to specify when their time comes.

The 2024 store proved the sats can flow. Blueprint proved the
contracts can hold. The Tassadar lane has now proven — once, at
smallest viable scale, with a receipt — that the goods can carry their
own evidence. The third store is the first one where all three are
true at the same time, and the discipline that gets it built is the
same one that closed the first two generations honestly: receipts or
it did not happen.

## Pointers

- [`README.md`](README.md) — the lane essay and current state
- [`RESEARCH_PLAN.md`](RESEARCH_PLAN.md) — the unified directive (W1–W4,
  hypotheses, kill conditions)
- [`work-that-proves-itself.md`](work-that-proves-itself.md) — the
  business thesis this audit applies to the marketplace surface
- `docs/transcripts/README.md` and episodes
  [`048`](../transcripts/048.md), [`066`](../transcripts/066.md),
  [`085`](../transcripts/085.md), [`092`](../transcripts/092.md),
  [`093`](../transcripts/093.md), [`098`](../transcripts/098.md),
  [`100`](../transcripts/100.md), [`102`](../transcripts/102.md),
  [`141`](../transcripts/141.md), [`211`](../transcripts/211.md) —
  the generation-one and generation-two record
- `blueprint/docs/master-spec.md`,
  `blueprint/docs/programs-optimization-and-rlm.md` (deprecated repo,
  historical) and
  `apps/openagents.com/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`
  (the live kernel rebuild)
- psionic `docs/TASSADAR_STARTER_PLUGIN_AUTHORING.md`,
  `docs/TASSADAR_STARTER_PLUGIN_CATALOG.md`,
  `docs/TASSADAR_STARTER_PLUGIN_RUNTIME.md` — the existing plugin
  registry, admission classes, and catalog (and their no-marketplace
  boundary)
- `products/2026-04-14-dspy-dsrs-gepa-rlm-forge-and-probe-audit.md`
  (workspace root) — the DSPy-era retrospective
