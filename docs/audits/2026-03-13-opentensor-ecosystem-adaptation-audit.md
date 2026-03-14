# 2026-03-13 Opentensor Ecosystem Adaptation Audit

## Intent

This audit answers a very specific question:

> after reading the local `~/code/opentensor` mirror, which parts of the
> Bittensor / Opentensor ecosystem should OpenAgents learn from, adapt, or
> explicitly avoid?

The answer is not "become Bittensor."

The useful answer is:

- Bittensor is a strong reference for how to organize off-chain utility markets
  around a common economic authority.
- It is a strong reference for validator economics, typed network protocol SDKs,
  operator tooling, indexer/explorer infrastructure, and staged launch
  discipline.
- It is a weak reference for OpenAgents' direct paid compute MVP, because
  OpenAgents should not replace job-linked sats settlement with chain emissions
  or validator weight-setting.

In short:

> Prime is the best local reference for widening the execution substrate.
> Cocoon is the best local reference for trust packaging and artifact truth.
> Opentensor is the best local reference for modular off-chain incentive
> programs, validator-mediated quality markets, and market observability.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/economy-kernel.md`
- `docs/plans/compute-market-full-implementation-plan.md`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-validator-service/src/lib.rs`

Opentensor sources reviewed:

- `~/code/opentensor/README.md`
- `~/code/opentensor/bittensor/README.md`
- `~/code/opentensor/btcli/README.md`
- `~/code/opentensor/btwallet/README.md`
- `~/code/opentensor/subtensor/README.md`
- `~/code/opentensor/subtensor/docs/{consensus.md,running-subtensor-locally.md}`
- `~/code/opentensor/subnet-template/README.md`
- `~/code/opentensor/bittensor-subnet-template/README.md`
- `~/code/opentensor/bittensor-subnet-template/docs/{running_on_testnet.md,running_on_mainnet.md,stream_tutorial/README.md}`
- `~/code/opentensor/bittensor-subnet-template/template/{protocol.py,validator/forward.py,validator/reward.py,base/validator.py}`
- `~/code/opentensor/bittensor/bittensor/{utils/subnets.py,core/axon.py,core/synapse.py}`
- `~/code/opentensor/subtensor-indexer/README.md`
- `~/code/opentensor/explorer/README.md`
- `~/code/opentensor/opentensor-api/README.md`
- `~/code/opentensor/bits/README.md`
- `~/code/opentensor/bits/bits/BIT-0002-Start-Call.md`
- `/Users/christopherdavid/Desktop/2603.08163v2.pdf`
  - `Covenant-72B: Pre-Training a 72B LLM with Trustless Peers Over-the-Internet`

## Executive Summary

The most important thing to understand about Bittensor is that it is not
primarily a model-serving stack.

It is a market architecture:

- one common economic authority layer (`subtensor`)
- many off-chain utility programs (`subnets`)
- producers (`miners`)
- scorers / adjudicators (`validators`)
- a common wallet, CLI, and indexing surface
- a common reward engine that turns off-chain evaluation into on-chain
  economics

That is why Bittensor matters to OpenAgents.

The OpenAgents repo already has the beginnings of a similar split:

- off-chain execution and proof work in `crates/psionic/*`
- provider execution and inventory truth in desktop and provider crates
- validator challenge substrate in `crates/openagents-validator-service`
- economic truth in `openagents-kernel-*` and `apps/nexus-control`

What OpenAgents should take from Opentensor is not the chain.

What it should take is the architectural discipline behind:

- modular off-chain market programs
- a clear producer / validator split
- manipulation-resistant aggregation for subjective quality
- typed protocol SDKs for application-level network interaction
- strong operator, wallet, indexer, and explorer tooling
- explicit local -> test -> production launch ladders

The `Covenant-72B` paper makes this more concrete than the repo READMEs alone.
It shows that on Bittensor Subnet 3, permissionless over-the-internet training
was made practical by:

- keeping many peers active while selecting only a bounded subset to actually
  contribute each round
- combining cheap universal checks with sampled higher-cost validation
- maintaining persistent participant ranking over time
- penalizing copycat or duplicate behavior
- using object storage as the data plane while the chain remained the economic
  coordination plane

That is a real design pattern OpenAgents should learn from for future training,
eval, and subjective-quality lanes.

What OpenAgents should not take is equally important:

- no new `subtensor`-style chain
- no TAO-like emissions as the primary product loop
- no weight-setting consensus as the settlement path for direct compute jobs
- no speculative lane launches before the product is truthful

The best reading is:

> Bittensor is more useful to OpenAgents' future labor / eval / validator /
> risk architecture than it is to the current compute-provider MVP.

## What Bittensor Actually Gets Right

### 1. One authority layer can coordinate many off-chain programs

The core Bittensor split is simple and powerful:

- `subtensor` is the shared economic and identity layer
- subnets are off-chain competitions
- miners and validators run outside the chain
- the chain stores enough information to coordinate rewards, identity, and
  registration

That is stated repeatedly across:

- `~/code/opentensor/README.md`
- `~/code/opentensor/bittensor/README.md`
- `~/code/opentensor/subtensor/README.md`

This maps surprisingly well onto OpenAgents' existing direction.

OpenAgents already wants:

- one shared economy kernel
- many market lanes
- off-chain execution systems
- explicit validator and proof services

The important lesson is that a machine economy does not need one runtime per
market. It needs one shared economic truth layer with many specialized
off-authority execution programs.

OpenAgents should adapt this as:

- one canonical kernel / Nexus authority
- multiple off-authority market programs or "lanes"
- explicit lane-specific protocols, evaluation logic, and validator rules
- common receipts, identity, settlement, and observability across all lanes

This is conceptually close to Bittensor's "one blockchain, many subnets," but
it should land in OpenAgents as "one kernel, many lanes" rather than as a new
chain.

### 2. The producer / validator split is a real economic primitive

Bittensor's most useful market idea is the explicit split between:

- miners that produce utility
- validators that score utility

The chain does not generate the utility. It aggregates and prices the scoring
results.

Relevant sources:

- `~/code/opentensor/bittensor/README.md`
- `~/code/opentensor/subnet-template/README.md`
- `~/code/opentensor/bittensor-subnet-template/README.md`
- `~/code/opentensor/subtensor/docs/consensus.md`

This is directly relevant to OpenAgents because the repo already has validator
concepts:

- `ComputeValidatorRequirements` in
  `crates/openagents-kernel-core/src/compute.rs`
- validator challenge infrastructure in
  `crates/openagents-validator-service/src/lib.rs`

The adaptation should be:

- keep providers responsible for executing work
- keep validators responsible for scoring, challenging, or adjudicating claims
- keep authority responsible for accepting the result into economic truth

That split is already the right owner boundary in `docs/OWNERSHIP.md`.
Bittensor reinforces that this should remain explicit rather than collapsing
execution and verification into one role.

### 3. Subjective utility needs manipulation-resistant aggregation

`subtensor/docs/consensus.md` is the most important Opentensor document for
OpenAgents.

Its core claim is not "weighted medians are cool."

Its core claim is:

> if a market depends on subjective scoring, naive averages and naive voting
> invite collusion, self-dealing, and reward manipulation.

Yuma Consensus addresses that with:

- stake-weighted clipping of outlier scores
- penalties for out-of-consensus scoring
- separate treatment of producer reward and validator reward
- explicit analysis of adversarial coalitions and subjectivity variance

OpenAgents should not copy the exact Yuma formulas.

It should copy the discipline:

- subjective markets need explicit anti-manipulation aggregation rules
- validators should not be able to print money by scoring themselves or their
  coalition freely
- validator reward and producer reward should be modeled separately
- subjective lanes need bond / withhold / challenge rules, not just reputation

This is much more relevant to OpenAgents':

- labor market
- eval market
- future risk and underwriting surfaces
- any future quality-scored "agent work" lane

It is less relevant to the first compute-provider MVP, where the primary truth
should still be:

- job delivery
- delivery proof
- wallet-confirmed settlement

So the Bittensor lesson is:

- use Yuma-like thinking for subjective utility lanes
- do not use Yuma-like weight voting as the primary settlement engine for
  objective compute jobs

### 4. Typed application protocols matter

Bittensor's SDK and templates are opinionated about developer surfaces:

- `Synapse` defines typed request/response messages
- `Axon` defines the server-side interface
- `Dendrite` defines the querying client
- `StreamingSynapse` handles streamed responses
- `SubnetsAPI` gives a reusable client wrapper for subnet-specific protocols

Relevant sources:

- `~/code/opentensor/bittensor/bittensor/core/axon.py`
- `~/code/opentensor/bittensor/bittensor/core/synapse.py`
- `~/code/opentensor/bittensor/bittensor/utils/subnets.py`
- `~/code/opentensor/bittensor-subnet-template/README.md`
- `~/code/opentensor/bittensor-subnet-template/docs/stream_tutorial/README.md`

This is one of the most transferable ideas in the whole ecosystem.

OpenAgents should adapt it as:

- typed protocol contracts for compute families, skill families, and future
  eval or labor request families
- a stable client/server SDK for those contracts
- explicit streaming protocol support
- machine-readable metadata about request origin, timeout, process time, and
  identity
- per-route verification / prioritization / blacklisting hooks

OpenAgents already has the beginnings of this in kernel contracts, desktop
control, and Psionic execution types, but the Bittensor SDK shows a cleaner
application-facing shape than raw transport messages alone.

### 5. Wallet, CLI, and config are part of the protocol, not side quests

The Opentensor ecosystem treats operator surfaces as first-class:

- `btwallet` wraps a Rust wallet core
- `btcli` exposes registration, staking, governance, and network views
- config files and debug logs are explicit
- wallet overview surfaces key economic metrics

Relevant sources:

- `~/code/opentensor/btwallet/README.md`
- `~/code/opentensor/btcli/README.md`

This aligns with OpenAgents' current direction:

- `spark` owns wallet primitives
- `autopilotctl` is the thin operator CLI
- the desktop owns user-facing product truth

The lesson is not to copy Bittensor's exact wallet or its terminology.

The lesson is:

- keep wallet core separate from app UX
- keep CLI and config stable enough for operators
- make diagnostic artifacts easy to export
- expose network and market truth in machine-friendly form, not only in UI

### 6. Indexers and explorers are not optional once a market exists

Bittensor has a broad observability stack:

- `subtensor-indexer`
- `explorer`
- `explorer-api`
- `explorer-ui`
- dashboards and metagraph views

Relevant sources:

- `~/code/opentensor/subtensor-indexer/README.md`
- `~/code/opentensor/explorer/README.md`
- examples in `btcli` and the subnet docs that show metagraph-like views of
  rank, trust, consensus, incentive, dividends, emissions, permits, and axon
  state

This is very relevant to OpenAgents because the compute-market plan already says
the market is not real unless the observability layer is real.

OpenAgents should adapt this as:

- a public or operator-facing explorer over canonical kernel receipts and read
  models
- provider roster / capability graph views analogous to a metagraph
- explicit read models for trust, challenge history, settlement health, and
  inventory posture
- a split between fast indexed views and direct authority-backed truth

Mission Control can remain simple, but the market needs an explorer-grade view
once the product claims a real compute market.

### 7. Launch discipline matters more than ideology

The Bittensor docs are explicit about:

- local development first
- then testnet
- then mainnet

And the `BIT-0002-Start-Call.md` proposal is even more instructive.

It exists because fully open subnet launches led to fake miners, fake
validators, speculation, and bad launches before subnet owners had real code and
real operator readiness in place.

Relevant sources:

- `~/code/opentensor/bittensor-subnet-template/docs/running_on_testnet.md`
- `~/code/opentensor/bittensor-subnet-template/docs/running_on_mainnet.md`
- `~/code/opentensor/subtensor/docs/running-subtensor-locally.md`
- `~/code/opentensor/bits/bits/BIT-0002-Start-Call.md`

This is a direct warning for OpenAgents.

OpenAgents should adapt this as:

- explicit warm-up or "start call" gates for new market lanes
- private or authority-gated soak periods before a lane is open to the whole
  market
- no new market lane should begin full economic activity merely because code was
  merged
- new compute or eval products should be able to exist in a dormant state until
  policy, monitoring, validator coverage, and starter-demand quality are ready

This is especially important for OpenAgents because the MVP is seller-first and
cold-start-sensitive. A bad lane launch can poison provider trust quickly.

### 8. Formal protocol-governance artifacts help

The `bits` repo is simple, but the model is useful:

- protocol changes are proposed in structured documents
- lifecycle states are explicit
- rationale and security implications are written down

Relevant sources:

- `~/code/opentensor/bits/README.md`
- `~/code/opentensor/bits/bits/BIT-0002-Start-Call.md`

OpenAgents already has:

- ADRs
- plans
- audits

That is better than nothing, but Bittensor reinforces that economic-rule changes
need a more explicit paper trail than ordinary product tweaks. This is
especially true for:

- validator policy changes
- settlement changes
- delivery-proof changes
- lane-launch rules
- price-index and reward methodology

### 9. One real subnet proves a better pattern for permissionless training

The `Covenant-72B` paper is important because it is not just another generic
"decentralized training" claim. It describes one concrete Bittensor subnet
deployment that actually trained a 72B model over commodity internet with
permissionless participation.

Relevant source:

- `/Users/christopherdavid/Desktop/2603.08163v2.pdf`

The specific patterns that matter are:

- active peers and contributing peers are not the same set
- the system caps final contributors per round even when more peers are active
- validators score candidate contributions before aggregation
- validator scoring combines:
  - a main expensive quality signal (`LossScore`)
  - fast universal checks
  - persistent ranking over time (`OpenSkill`)
- peers can be penalized for improving random data more than assigned data,
  which is used as a duplicate/copying signal
- individual pseudo-gradients are normalized before aggregation so a single
  abnormal submission cannot dominate the update
- object storage (Cloudflare R2 in the paper) is used as the communication
  backbone, avoiding direct full-mesh P2P requirements for all participants

This is extremely relevant to OpenAgents' future training and eval lanes.

It suggests that the correct pattern for permissionless high-cost work is often:

- open admission
- bounded contributor selection
- validator-mediated filtering
- durable ranking over time
- artifact/object-store exchange
- authority acceptance after scoring

That is a much stronger and more operationally realistic model than "everyone
who shows up contributes equally."

## What OpenAgents Should Adapt

### 1. Define lane modules more explicitly

OpenAgents should have a stronger concept of:

- one shared kernel
- multiple off-authority lanes
- lane-specific protocols
- lane-specific validator policies
- lane-specific launch controls

That is the OpenAgents equivalent of Bittensor's subnet model.

It should not be implemented as a new chain or a new token. It should be
implemented as:

- kernel objects
- authority policy
- lane registries
- typed contracts
- operator-visible read models

### 2. Formalize validator aggregation for subjective lanes

OpenAgents should introduce an explicit policy for how multiple validators'
judgments combine when work is subjective.

That policy should include:

- clipping or bounded influence
- outlier or collusion resistance
- explicit validator reward / penalty treatment
- bond, withhold, or challenge escalation rules
- transparency about how final scores are derived

This belongs more in:

- labor
- evaluation
- risk

than in first-wave compute.

The `Covenant-72B` paper sharpens this further.

OpenAgents should explicitly support a two-tier validator pattern for expensive
subjective or semi-subjective lanes:

- cheap checks that run on every submission
- expensive scoring that runs on a bounded subset

It should also support persistent validator-side participant ranking over time,
not just per-round scores, and it should let policy distinguish:

- active participants
- eligible participants
- selected contributors
- accepted contributors

That is a much better fit for future training and eval lanes than naive
"everyone submits, everyone counts" aggregation.

### 3. Build a better typed network SDK

OpenAgents should define typed request/response families in the same spirit as:

- `Synapse`
- `StreamingSynapse`
- `Axon`
- `Dendrite`
- `SubnetsAPI`

Likely OpenAgents landing zones:

- `crates/openagents-kernel-proto`
- typed client crates
- `autopilotctl` request/response schemas
- future MCP or service wrappers over compute / eval / validator operations

The goal is not to recreate Bittensor's Python APIs. The goal is to give users
and lane authors a stable application protocol layer.

### 4. Build a real explorer / indexer surface

OpenAgents should not stop at receipts and internal read models. It should build
market-facing observability:

- provider roster views
- product / lot / instrument views
- validator challenge views
- challenge-result and proof explorer views
- market-health dashboards

The right model is:

- authority remains canonical
- indexed views make the network legible

That is exactly the balance Bittensor's indexer and explorer stack is trying to
achieve.

### 5. Add explicit lane start-call rules

Any new OpenAgents market lane or product class should have:

- dormant registration
- readiness checks
- staged activation
- explicit enablement by authority

This is one of the clearest actionable lessons from Bittensor's start-call
proposal and launch-history problems.

### 6. Keep operator UX simple while preserving deep operator tools

OpenAgents should keep:

- the normal user flow simple
- the operator flow deep

That means:

- simple desktop UX for ordinary providers
- richer CLI / explorer / read-model tools for operators, testers, and
  protocol developers

This is exactly how Bittensor separates the basic participant story from the
deeper chain, subnet, wallet, and explorer surfaces.

### 7. For future training lanes, separate the control plane from the data plane

The `Covenant-72B` design is useful here:

- Bittensor handled coordination and incentives
- object storage handled pseudo-gradient exchange

OpenAgents should adapt the same principle for internet-scale training or
evaluation lanes:

- authority and validator policy should remain in kernel / Nexus
- heavy artifacts, checkpoints, gradient shards, and eval bundles should move
  through an artifact plane owned by Psionic or a related data-plane service
- policy should not require direct peer-to-peer collectives when an asynchronous
  object or staging plane is more robust

This lines up with the existing direction toward `psionic-datastream`,
artifact-residency fields, and explicit proof bundles.

## What OpenAgents Should Not Copy

### 1. Do not build a new `subtensor`

Bittensor needs a blockchain because its core economic primitive is chain-based
emissions and stake-weighted subnet governance.

OpenAgents already has a different architecture:

- direct payments
- kernel receipts
- Nexus authority
- Nostr transport
- desktop-first UX

Creating a new chain or a chain-shaped consensus layer would be architecture
drift, not leverage.

### 2. Do not replace direct paid jobs with emissions

The OpenAgents MVP promise is:

- do work
- get paid
- see sats land
- withdraw

That is a much cleaner initial product loop than speculative emissions.

Bittensor's emission machinery is useful as a reference for subjective utility
markets, but it would be a mistake to make it the primary user experience for
OpenAgents' compute-provider lane.

### 3. Do not let validator weights settle objective compute delivery

For OpenAgents compute, the primary truth should be:

- did the promised work execute
- what proof exists
- what challenge result exists
- what settlement receipt exists

Not:

- what a validator coalition weighted it at

Yuma-like aggregation belongs where value is inherently subjective. It should
not override direct delivery and settlement truth for objective compute jobs.

### 4. Do not allow lane speculation to outrun truthful product launch

Bittensor's start-call BIT is effectively a postmortem on premature open-market
launches.

OpenAgents should learn from that and avoid:

- announcing a lane before validator and monitoring posture exist
- exposing a lane to the whole market before the operator path is solid
- letting new providers or validators exploit unclear launch rules

### 5. Do not import Bittensor's user vocabulary wholesale

Concepts like:

- subnets
- netuids
- hotkeys
- coldkeys
- delegates
- senate

exist for Bittensor's architecture.

OpenAgents should translate only the underlying lessons, not the vocabulary.
The desktop product should stay legible to ordinary users.

### 6. Do not accept Python-first ergonomics as the final form

Bittensor is heavily Python-shaped. OpenAgents is not.

The lessons to import are:

- typed protocols
- client/server SDKs
- clear role boundaries
- stable operator tools

The implementation should remain OpenAgents-owned and Rust-first where that is
already the repo direction.

## Recommended Changes To The OpenAgents Backlog

### Highest priority

1. Define an explicit lane/module registry in kernel authority.
   It should represent off-authority market programs, their protocol family,
   validator policy family, and launch posture.

2. Define a validator-aggregation policy surface for subjective lanes.
   This should cover clipping, quorum, dispute windows, and penalty posture.

3. Build typed lane protocol SDKs with streaming support.
   This is the OpenAgents analogue of `Synapse` / `StreamingSynapse` /
   `SubnetsAPI`.

4. Build explorer/indexer surfaces over kernel receipts and read models.
   The market needs public or operator-grade legibility once multiple lanes are
   active.

### Next priority

5. Add dormant -> active lane activation rules.
   New lanes should be registerable before they are open for real economic
   traffic.

6. Add metagraph-like provider and validator roster views.
   These should show capability, health, validator coverage, challenge posture,
   and recent settlement quality.

7. Define a formal change-document path for market-rule changes.
   ADRs and plans already exist; the missing piece is a narrower protocol-change
   process for economic rules.

### Lower priority

8. Consider lane-specific external APIs or connector registries later.
   `opentensor-api` is a reminder that once lane truth exists, wrappers and
   monetized APIs will appear naturally. That is a later product layer, not a
   first architectural dependency.

9. For future training or distributed optimization lanes, add explicit
   participant-state categories and contribution caps.
   `Covenant-72B` shows the value of tracking active peers separately from
   contributing peers and keeping spare active capacity so dropouts can be
   replaced quickly.

## Ownership Map For The Adaptation

- `apps/autopilot-desktop`
  - own the simple provider UX, Mission Control, and buyer/provider product
    surfaces
- `crates/openagents-provider-substrate`
  - remain the narrow provider descriptor and inventory-control layer
- `crates/psionic/*`
  - own execution runtime, transport, and proof-bearing execution substrate
- `crates/openagents-validator-service`
  - own validator challenge execution and validator-worker mechanics
- `crates/openagents-kernel-core` and `crates/openagents-kernel-proto`
  - own lane registries, validator requirement shapes, aggregation-policy
    contracts, and canonical economic object types
- `apps/nexus-control`
  - own lane activation, receipt mutation, read models, and explorer-facing
    market projections
- `crates/spark`
  - continue to own wallet primitives rather than absorbing lane-market logic

## Bottom Line

The best way to read the Opentensor ecosystem against OpenAgents is:

- Prime shows how to widen the execution and compute substrate.
- Cocoon shows how to package runtime trust, artifact truth, and settlement
  discipline.
- Bittensor shows how to organize many off-chain utility programs around a
  common economic authority with explicit validators, typed protocols, and
  market observability.

OpenAgents should take from Bittensor:

- the off-authority lane model
- the producer / validator split
- the insistence on manipulation-resistant aggregation for subjective markets
- the typed application protocol surface
- the indexer / explorer mindset
- the launch-discipline mindset

OpenAgents should not take from Bittensor:

- the chain
- the emissions-first product loop
- the user vocabulary
- the tendency to let economic permissionlessness outrun product truth

So the actionable conclusion is:

> OpenAgents should become more Bittensor-like in how it structures lane
> economics, validators, typed protocols, and market observability, while
> remaining entirely unlike Bittensor in its settlement UX, chain posture, and
> desktop-first MVP loop.
>
> And after reading `Covenant-72B`, one additional conclusion is clear:
> Bittensor is not only useful as an abstract subnet architecture reference. It
> is also a concrete reference for how a future OpenAgents training lane could
> admit permissionless participants, filter them through validator scoring,
> move heavy artifacts asynchronously, and keep the economic authority separate
> from the actual training data plane.
