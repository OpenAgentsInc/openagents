# Compute Market

This is the canonical status and explanation doc for the `Compute Market`.

Use this file for the definitive answer to:

- what the compute market is,
- why it matters,
- what is actually implemented in this repo now,
- what is only partially landed or still scaffold-quality,
- what remains planned,
- and how the Prime/Psionic compute audit changes the definition of "complete."

Historical note:

- product authority for the retained MVP still lives in `docs/MVP.md`
- active crate ownership still lives in `docs/OWNERSHIP.md`
- the broader kernel law still lives in `docs/kernel/economy-kernel.md` and
  `docs/kernel/economy-kernel-proto.md`

This file is the single market-specific place where those realities are pulled
together for the compute lane.

## In Plain English

Most people understand "selling compute" only as a vague idea: you have a
machine, someone sends it work, and maybe you get paid. That is good enough for
a demo, but it is not enough for a real market.

A real compute market means the machine work becomes legible. The provider is
not just saying "trust me, I ran something." They are offering a defined kind
of capacity, under stated conditions, over a stated time window, with a clear
delivery story and a clear payment story. The buyer is not just spraying jobs
into the void. They are buying a specific promise about machine execution. The
system is not just hoping everything works out. It is recording what was
offered, what was accepted, what was delivered, what proof exists, and what the
economic result was.

That matters because compute becomes much more valuable once it is explicit.
When supply is explicit, providers can compare offers, buyers can ask for the
kind of machine execution they actually need, and the system can reason about
failure honestly instead of hiding it behind generic "job failed" messages.
This is how you go from "an app that can run a task" to "a market where machine
capacity can be discovered, priced, verified, and eventually hedged."

For OpenAgents specifically, the compute market matters because the current MVP
promise is already compute-first: a user opens Autopilot, clicks `Go Online`,
accepts paid machine work, sees sats arrive in their wallet, and withdraws.
That loop is real, and it is the strongest live market loop in the repo. But it
is still only the first layer. Today the user experiences "I got paid for a
compute job." The long-horizon goal is that the same loop is backed by a
complete market structure with explicit products, inventory, proofs, settlement,
and later more advanced supply types such as clustered serving and sandboxed
execution.

The easiest way to say it is this:

> the Compute Market is the part of OpenAgents that turns machine execution into
> a truthful economic object.

If the Labor Market sells bounded work, the Compute Market sells bounded machine
capacity. If the Data Market sells access to useful context, the Compute Market
sells the right to consume a machine execution lane. If the Risk Market prices
uncertainty and liability, the Compute Market is one of the main things whose
failure and deliverability risk need to be priced.

That is why this market is strategically important. It is not just another
feature lane. It is the foundation under the current earn loop and a major
foundation under the wider OpenAgents economy.

## Definitive Repo Verdict

As of `2026-03-13`, the Compute Market is:

- the deepest and most real of the five markets
- already productized at the seller-side MVP loop
- already implemented as a meaningful kernel authority slice with durable state,
  receipts, read routes, indices, and delivery proofs
- already wider than "just local inference" at the type and substrate level
- not yet fully productized end to end as a complete market for ordinary users
- not yet complete in the Prime-audit sense of cluster, sandbox, proof,
  validator, environment, and operator-grade compute-market truth

The short, honest summary is:

> OpenAgents already has a real compute-provider product and a substantial
> starter compute-market authority layer. It does not yet have the full
> end-state compute market.

## What The Compute Market Actually Is Here

In OpenAgents, the Compute Market is the market that prices declared machine
execution capacity as receipted economic state.

The kernel-facing compute objects are:

- `ComputeProduct`
- `CapacityLot`
- `CapacityInstrument`
- `StructuredCapacityInstrument`
- `DeliveryProof`
- `ComputeIndex`

Those objects answer six different questions:

- `ComputeProduct`: what kind of machine execution is being sold
- `CapacityLot`: what inventory is currently on offer
- `CapacityInstrument`: what specific claim or allocation was created against
  that inventory
- `StructuredCapacityInstrument`: what grouped or higher-order exposure exists
  across instruments
- `DeliveryProof`: what evidence says delivery actually happened
- `ComputeIndex`: what market reference price or observation series the system
  has published

At the broadest level, this market is intended to cover:

- local inference products
- embeddings products
- clustered and public-network serving products
- bounded remote sandbox execution products
- environment-linked eval and benchmark execution
- later training-class and adapter-hosting products

The active MVP only productizes a subset of that. The market definition is
broader than the current user-facing release.

## What "Complete" Means

The current canonical implementation plan now treats a complete compute market
as satisfying all of these tests at once:

- product truth: providers can expose legible compute supply and buyers can
  reason about what they are buying
- authority truth: canonical lifecycle, receipts, and settlement live in the
  authority layer
- protocol truth: wire contracts cover the real market objects instead of only
  compute-shaped side fields
- observability truth: stats and snapshots expose compute-market health instead
  of only generic job counts
- policy truth: deliverability, proof posture, and bounded-risk controls are
  explicit
- execution-substrate truth: local, clustered, and sandboxed compute can all be
  represented honestly
- proof/eval truth: delivery can carry proof, validator, and evaluation linkage
- operator truth: CLI and control APIs can inspect and operate the compute
  substrate without bypassing market authority

That last three-part widening comes directly from the Prime/Psionic compute
audit. A compute market is no longer considered complete here if it only has
spot lots, forward obligations, and a few proof records. It must also own the
execution, proof, and evaluation substrate that makes those promises
economically meaningful.

## Status Legend

- `implemented`: shipped in current repo behavior or authoritative services
- `partial`: real code exists, but it is not yet the default end-to-end market
  path or not yet fully productized
- `planned`: target architecture or market lane, not yet credibly landed

## Comprehensive Status Matrix

| Dimension | Status | Definitive answer |
| --- | --- | --- |
| Seller-side earn loop | `implemented` | Autopilot already lets a provider go online, accept compute work, deliver, get wallet-confirmed sats, and withdraw. |
| Compute as a user-visible market lane | `implemented`, narrow | Compute is the only visibly productized market lane today, but the live UX is still seller-first rather than a full buyer/seller commodities surface. |
| Kernel compute object model | `implemented` | `ComputeProduct`, `CapacityLot`, `CapacityInstrument`, `StructuredCapacityInstrument`, `DeliveryProof`, and `ComputeIndex` all exist in `openagents-kernel-core`. |
| Durable compute authority | `implemented` | `apps/nexus-control` persists compute objects, receipts, snapshots, environment packages, evals, synthetic jobs, indices, and validator challenges. |
| Compute read routes and mutations | `implemented` | Authenticated HTTP routes exist for the main compute objects and validator-challenge lifecycle. |
| Compute wire/proto surface | `implemented`, still growing | Checked-in `openagents.compute.v1` packages cover products, capacity, instruments, delivery, indices, evals, synthetic data, and benchmark import, but the total market still extends beyond current productized use. |
| Public stats and snapshots | `implemented` | Compute counters, delivery-proof counters, index counters, and validator-challenge counters are projected into stats/snapshots. |
| Local inference supply | `implemented` | The retained productized lane is local inference, currently centered on Apple FM for the historical macOS release cut and GPT-OSS for retained/operator paths. |
| Embeddings family | `partial` | There are product IDs, provider-substrate descriptors, proto/domain support, and Psionic embeddings tests, but embeddings are not yet the primary visible market lane. |
| Remote sandbox execution | `partial` | Psionic sandbox supply, execution receipts, and desktop-control flows exist, but sandbox execution is not yet a first-class kernel-market product for ordinary buyers/providers. |
| Clustered/public-network compute | `partial` | Psionic has meaningful cluster and sharded-serving substrate, but clustered compute is not yet fully productized as canonical market inventory and settlement truth. |
| Delivery proofs | `implemented` | Delivery proof recording, lookup, linkage, and metric projection exist in authority. |
| Validator challenge lifecycle | `implemented`, early | Challenge schedule, lease, finalize, stats, and proof linkage exist, but this is still infrastructure scaffolding rather than a fully matured market-wide challenge regime. |
| Environment registry | `implemented` | Environment package lifecycle exists in the compute authority. |
| Evaluation-run lifecycle | `implemented` | Compute eval runs and benchmark-adapter imports exist as canonical compute-adjacent records. |
| Synthetic-data lifecycle | `implemented` | Synthetic compute jobs and samples exist in the compute authority slice. |
| Buyer procurement UX | `partial` | The app has buyer/bootstrap behavior and market preview, but not yet a broad, explicit buyer-side compute procurement experience. |
| Spot market productization | `partial` | Kernel objects exist, but the current end-user loop is not yet broadly surfaced as explicit spot inventory browsing and allocation UX. |
| Forward physical capacity | `partial` | Capacity instruments and structured instruments exist in authority, but a fully legible forward-capacity product surface is not yet the default market UX. |
| Cash-settled hedges and derivatives | `partial`, mostly authority-first | The kernel already models more than the MVP UI shows, but these are not yet a live user-facing exchange product. |
| Compute indices and corrections | `implemented`, early | Index publication and correction routes exist, but full governance, external feeds, and deeper market-depth operations are still ahead. |
| Operator control surfaces | `partial` | `autopilotctl`, desktop control, and headless compute are real, but a full compute-market operator plane for cluster, proof, and sandbox operations is still incomplete. |
| Training-class products | `planned` | The market definition reserves space for them, but they are not yet live compute-market products. |
| Adapter-hosting products | `planned` | Not productized yet. |

## Current Product Truth

The most important truth to preserve is the one in `docs/MVP.md` and
`docs/v01.md`: OpenAgents already ships a real compute-provider earn loop.

Today that means:

- a provider can go online from `apps/autopilot-desktop`
- the app can receive compute work through the current network flow
- the provider executes the work locally
- the provider publishes a result
- payment settles into the built-in Spark wallet
- the user can see wallet-authoritative balance movement and withdraw

This is why compute is the deepest market. It is not theoretical. It is already
tied to product behavior people can actually experience.

It is important to separate current repo truth from older planning language:

- the retained MVP and `v0.1` product truth are Apple-FM-first on the historical
  macOS release path
- the retained operator and provider-substrate truth also includes GPT-OSS
  inference, GPT-OSS embeddings descriptors, and sandbox execution descriptors
- older long-horizon compute planning documents may still reference `Ollama`
  from pre-prune work; treat that as historical planning context rather than
  the current retained MVP code truth

## What Is Implemented Right Now

### 1. Productized desktop behavior

Implemented in `apps/autopilot-desktop` and supporting docs:

- provider online/offline lifecycle
- local runtime health and provider readiness
- NIP-90-style paid compute request flow
- starter-demand/bootstrap behavior
- active-job lifecycle visibility
- wallet-confirmed payout visibility
- inline withdrawal path
- app-owned control plane and `autopilotctl`
- headless packaged and roundtrip verification flows

This is the live market wedge: "sell compute, get paid."

### 2. Canonical compute authority

Implemented in `apps/nexus-control/src/kernel.rs` and
`apps/nexus-control/src/lib.rs`:

- create/list/get/cancel for compute products and capacity lots
- create/list/get/close/cash-settle for capacity instruments
- create/list/get/close for structured capacity instruments
- record/list/get for delivery proofs
- publish/list/get/correct for compute indices
- schedule/lease/finalize/list/get for validator challenges
- durable persistence and reload for compute authority state
- compute-specific metrics in stats and snapshots

This matters because the market is no longer only "app runtime state." It has
real authority objects and real durable records.

### 3. Wire/proto contracts

Implemented under `proto/openagents/compute/v1/*` and generated into
`crates/openagents-kernel-proto`:

- compute products
- compute capacity
- compute instruments
- compute delivery
- compute indices
- compute evals
- compute synthetic data
- benchmark-adapter import surfaces

This is broader than the old "thin compute proto" description. It is still not
the final word on the whole market, but it is already a substantial starter
package tree.

### 4. Compute-adjacent market extensions

Implemented in the kernel authority and documented elsewhere:

- environment package registry
- evaluation-run lifecycle
- synthetic-data lifecycle
- benchmark-adapter import into canonical eval runs

Supporting docs:

- [../compute-environment-packages.md](../compute-environment-packages.md)
- [../compute-evaluation-runs.md](../compute-evaluation-runs.md)
- [../compute-synthetic-data.md](../compute-synthetic-data.md)
- [../compute-benchmark-adapters.md](../compute-benchmark-adapters.md)

These are not side trivia. They are part of the broadened compute-market shape
because compute value increasingly depends on environments, evaluation, and
benchmark truth.

### 5. Psionic execution substrate that already exists

Real code already exists in `crates/psionic/*` for more than single-node
inference:

- local inference runtime and serving substrate
- embeddings tests and model-backed execution paths
- cluster state, ordered state, scheduling, replicated serving, layer sharding,
  and tensor sharding seed material
- sandbox runtime detection, bounded execution profiles, sandbox execution
  receipts, and sandbox evidence
- execution proof bundle types that can carry sandbox context and runtime proof
  evidence

That means the Prime audit is not asking the repo to imagine these categories
from zero. It is asking the repo to finish productizing and market-linking them.

## What Is Only Partial Today

These are the areas where real code or real specs exist, but the market is not
yet fully there.

### Explicit market productization

- The end-user loop is still mostly "paid compute jobs" rather than explicit
  browsing and trading of compute inventory.
- The kernel objects exist, but the desktop does not yet make them the ordinary
  user-facing mental model.

### Embeddings as a first-class family

- Embeddings exist in product IDs, domain types, provider-substrate descriptors,
  and Psionic tests.
- Embeddings are not yet as complete or as visibly productized as the
  inference-led lane.

### Sandbox as a first-class compute product

- Sandbox profiles, runtime health, execution flows, receipts, file transfer,
  and desktop-control actions exist.
- The missing piece is full market productization: sandbox inventory,
  procurement, delivery proof, settlement, and operator views as a normal
  compute-market family.

### Clustered compute as a first-class compute product

- Psionic already contains cluster and sharded-serving substrate.
- What is still missing is the market-facing layer: inventory, topology truth,
  placement truth, cluster-backed delivery semantics, and settlement logic that
  make clustered serving a product rather than just runtime internals.

### Validator and challenge systems as default market behavior

- Validator challenge routes and proof linkage exist.
- The repo is not yet at the point where proof-sensitive products routinely
  flow through a mature validator/challenge regime.

### Buyer-side compute procurement

- The app contains buyer/bootstrap behavior and market preview.
- It does not yet expose a full buyer-facing compute procurement surface with
  explicit inventory discovery, quote acceptance, and standardized product
  comparison.

### Advanced instruments

- The kernel authority already knows more than the UI currently exposes.
- Structured instruments and index-linked behavior exist at authority level.
- They are not yet a broad, legible market UX.

## What Is Still Missing For The Full Market

The compute market is not complete until the following are true in practice,
not only in docs.

### Missing market-surface pieces

- explicit buyer-side spot procurement UX
- explicit provider inventory management for multiple compute families
- visibly productized forward physical capacity windows
- mature compute-index governance and external reference integration
- later derivatives only after the physical and index layers are credible

### Missing execution/productization pieces

- clustered compute as a first-class sold family
- public-network transport and relay-fallback cluster truth wired into market
  inventory
- remote sandbox execution as a first-class sold family
- environment-linked compute offerings and environment-compatibility policy in
  the normal procurement path
- clearer compute-family expansion to eval, training, and adapter-hosting lanes

### Missing proof and risk pieces

- proof posture that is routine rather than exceptional for sensitive products
- mature validator operations and challenge-linked remedies
- broader policy and risk controls around deliverability, manipulation, and
  settlement failure

### Missing operator-plane pieces

- fuller CLI/operator visibility for cluster, sandbox, proof, and validator
  state
- market-linked control surfaces that do not require reading code or logs to
  understand machine supply and delivery posture

## What The Prime/Psionic Audit Adds

The Prime adaptation audit changed the definition of a "fully implemented"
compute market in this repo.

Before that widening, it was possible to think of the compute market mostly as:

- products
- lots
- instruments
- delivery proofs
- indices

That is still correct, but it is incomplete.

The Prime/Psionic reading adds the requirement that compute-market truth must
also cover the reusable execution substrate that makes those objects honest.
That means the compute market now has to absorb, at minimum:

- public-network transport, relay fallback, and cluster membership truth
- collectives, shard placement, and clustered serving truth
- bounded remote sandbox execution as a first-class compute family
- proof bundles, validator services, and challenge-linked settlement
- environment, eval, synthetic-data, and benchmark substrate
- operator-grade CLI, control-plane, and attach/tunnel-like surfaces
- later training-class and adapter-hosting families after the substrate is
  credible

In practical terms:

- if clustered serving exists only as runtime internals, the compute market is
  not complete
- if sandbox execution exists only as app-local tooling, the compute market is
  not complete
- if proof-sensitive delivery has no credible validator/challenge loop, the
  compute market is not complete
- if environments and evals are disconnected from the compute products that
  depend on them, the compute market is not complete

That is now the canonical reading.

## Relationship To The Other Four Markets

- `Compute` sells declared machine capacity and machine execution contracts.
- `Labor` sells bounded agent work. If the question is "please do this task" in
  an open-ended worker sense, it belongs there.
- `Data` sells permissioned access to useful context, corpora, or datasets.
- `Liquidity` routes money, reserves, and value movement.
- `Risk` prices uncertainty, insurance, challenge outcomes, underwriting, and
  liability around market promises.

These markets interact heavily, but they are not the same thing.

## Current Repo Truth Lives In

Code:

- `apps/autopilot-desktop`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-core/src/compute_contracts.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `crates/openagents-provider-substrate/src/admin.rs`
- `crates/psionic/*`
- `proto/openagents/compute/v1/*`
- `crates/openagents-kernel-proto`

Primary supporting docs:

- [../README.md](../README.md)
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)
- [../../MVP.md](../../MVP.md)
- [../../v01.md](../../v01.md)
- [../../plans/compute-market-full-implementation-plan.md](../../plans/compute-market-full-implementation-plan.md)
- [../../plans/prime-ecosystem-compute-integration-spec.md](../../plans/prime-ecosystem-compute-integration-spec.md)
- [../../audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md](../../audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md)

## Bottom Line

The compute market is already real enough to matter and already broad enough to
deserve serious architecture discipline.

Today, it is best understood as:

- a real seller-side product
- a real starter authority market
- a partially productized commodity market
- and an incomplete but credible foundation for clustered, sandboxed,
  proof-bearing, environment-linked machine execution

That is why it is both the strongest current market in OpenAgents and still one
of the biggest remaining build programs in the repo.
