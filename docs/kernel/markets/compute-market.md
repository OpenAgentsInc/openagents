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

Compute indices are used for reference pricing, structured settlement, and
later risk-market linkage.

At the broadest level, this market is intended to cover:

- local inference products
- embeddings products
- clustered and public-network serving products
- bounded remote sandbox execution products
- environment-linked eval and benchmark execution
- later training-class and adapter-hosting products

The active MVP only productizes a subset of that. The market definition is
broader than the current user-facing release.

## Core Market Objects At A Glance

| Object | Role | Economic meaning |
| --- | --- | --- |
| `ComputeProduct` | Product definition | Defines admissible compute family, settlement mode, and market-level rules |
| `CapacityLot` | Inventory | Provider supply exposed to the market |
| `CapacityInstrument` | Economic claim | The claim buyers actually hold, track, and settle |
| `StructuredCapacityInstrument` | Aggregated exposure | Grouped claims such as reservations, swaps, or strips |
| `DeliveryProof` | Execution evidence | Evidence supporting delivery and conformance |
| `ComputeIndex` | Market reference | Reference observation series used for price discovery, structured settlement, and risk integration |

One distinction matters immediately:

- `ComputeProduct` defines admissibility and settlement rules
- `ComputeCapabilityEnvelope` defines the measurable execution and performance
  envelope under which that product is being offered

The product is not the same thing as the capability description. The product is
the market category. The capability envelope is the measurable machine promise.

## What The Compute Market Is Not

The Compute Market is not:

- generic job orchestration without economic state
- only provider runtime health
- only wallet payout bookkeeping
- the Labor Market under another name
- raw dataset or context access
- risk pricing itself, even though it feeds the Risk Market

The clean boundary is:

- if the thing being sold is bounded machine execution capacity, it belongs here
- if the thing being sold is open-ended agent work, it belongs in Labor
- if the thing being sold is permissioned context or data access, it belongs in
  Data
- if the thing being priced is uncertainty, liability, or coverage, it belongs
  in Risk

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

## Canonical Market Lifecycle

The canonical compute-market lifecycle should be read as the end-to-end state
machine for a compute promise.

Lifecycle at a glance:

1. Provider declares product and inventory.
2. Buyer discovers supply and procures against defined constraints.
3. An allocation or instrument is created.
4. Execution starts in the runtime substrate.
5. Delivery proof is recorded.
6. Validator or challenge window applies if required.
7. Settlement finalizes or a dispute/remedy path opens.
8. Final state is projected into canonical read models, snapshots, and stats.

The compact rule is:

> products describe what may be sold, lots expose current supply, instruments
> represent the economic claim, delivery proofs defend execution, and authority
> decides closure.

1. Product definition: a `ComputeProduct` defines what kind of machine
   execution is admissible, how it settles, whether it is index-eligible, and
   what capability envelope, environment binding, or proof posture applies.
2. Supply declaration: a provider publishes or materializes a `CapacityLot`
   with quantity, delivery window, optional floor price, region hint,
   environment binding, and offer expiry.
3. Discoverability: that lot becomes discoverable through authority read models
   and market views rather than existing only as hidden runtime state.
4. Procurement or allocation: a buyer or market flow creates a
   `CapacityInstrument` against a product or specific lot. That instrument is
   the actual economic claim: spot, forward physical, future cash, or
   reservation.
5. Runtime execution: provider, Psionic, sandbox, or cluster runtime truth
   records what actually happened during execution.
6. Delivery recording: a `DeliveryProof` is recorded with metered quantity,
   accepted quantity, promised versus observed capability envelope, and any
   topology, sandbox, or verification evidence.
7. Verification or challenge: if the proof posture requires it, validator runs
   and challenge windows apply before the market can treat delivery as
   economically final.
8. Economic closure: the instrument moves toward settlement, default,
   cancellation, expiry, or correction depending on the proof, payment, and
   policy outcome.
9. Index and downstream reference: accepted delivery observations may feed
   compute indices, later corrections, risk triggers, or structured instrument
   settlement.
10. Projection: the final market state appears in authority read models,
    operator views, public stats, and economy snapshots rather than staying
    trapped in runtime-local state.

The code already contains much of this lifecycle as explicit statuses.

Current canonical status surfaces:

- `ComputeProductStatus`: `active`, `retired`
- `CapacityLotStatus`: `open`, `reserved`, `delivering`, `delivered`,
  `cancelled`, `expired`
- `CapacityInstrumentStatus`: `open`, `active`, `delivering`,
  `cash_settling`, `settled`, `defaulted`, `cancelled`, `expired`
- `StructuredCapacityInstrumentStatus`: `open`, `active`,
  `partially_closed`, `settled`, `defaulted`, `cancelled`, `expired`
- `DeliveryProofStatus`: `recorded`, `accepted`, `rejected`
- `ComputeIndexStatus`: `published`, `superseded`

That is the clearest current answer to "what is the compute-market state
machine here?"

## Truth Boundaries

The compute market spans multiple truth domains. The doc should state them
explicitly because they are not interchangeable.

- Authority truth: canonical market records, receipts, settlement state,
  corrections, and policy outcomes owned by the kernel authority layer.
- Runtime truth: what Psionic, provider runtimes, clusters, and sandboxes
  actually observed during execution.
- Wallet and payment truth: what funds movement and payout confirmation say
  about economic completion.
- Validator truth: challenge outcomes, proof verification artifacts, and
  verification verdicts produced by validator infrastructure.

Those truths meet, but they are not the same thing.

Examples:

- a runtime can say execution finished, but authority can still reject or
  default the market claim
- a delivery proof can be recorded, but wallet settlement can still fail
- a payment can land, but later correction or dispute logic can still supersede
  market interpretation
- a validator can reject a challenge-eligible delivery even if the provider
  believes the runtime result was correct

The canonical rule is:

> runtime truth explains what happened, but authority truth determines the
> market outcome.

Runtime identity should also be read as a first-class concept here. Runtime
identity means the verified runtime context responsible for execution, such as a
provider node identity, sandbox instance identity, or cluster member set. Proof
attribution, validator handling, and settlement disputes depend on that link.

## Settlement Boundary And Economic Closure

The compute market needs a sharper settlement boundary than "the job ran."

The relevant milestones are:

- execution completion: the runtime finished or terminated
- delivery recording: a `DeliveryProof` was submitted
- delivery acceptance: the proof was accepted rather than merely recorded
- economic acceptance: the authority recognizes the execution as conforming
  enough to advance the market claim
- final settlement: the instrument reaches `settled`, or the structured
  instrument reaches its own terminal state
- adverse closure: the claim reaches `defaulted`, `cancelled`, `expired`, or a
  settlement-failure path instead
- correction: later index or authority correction supersedes an earlier market
  view where policy allows it

That distinction matters because these are not synonyms:

- "compute ran"
- "delivery was proven"
- "delivery was accepted"
- "the market position is economically closed"

Canonical closure terms:

- delivered: the runtime reached a machine-delivery outcome and the lot or
  instrument advanced into a delivery path
- proved: a `DeliveryProof` exists and was recorded
- accepted: the delivery proof was accepted as conforming market evidence
- disputed: proof, validator, challenge, or remedy handling is still open, so
  the market claim is not economically final yet
- finalized: the market claim reached a terminal economic state
- cash-settled: closure happened through a cash/index path rather than a pure
  physical-delivery path
- corrected or reversed: authority later superseded a previously published
  market interpretation, usually through correction, rejection, or default logic
- defaulted: the market claim terminated adversely because deliverability or
  settlement requirements were not met

These are conceptual market-closure distinctions across objects. They are not
all represented by one enum today, which is precisely why this doc needs to say
them explicitly.

Current settlement modes already in code:

- `physical`: the market centers on physical delivery of compute
- `cash`: the market centers on cash settlement against a reference
- `buyer_election`: the buyer may have a bounded settlement choice under policy

Current instrument kinds already in code:

- `spot`
- `forward_physical`
- `future_cash`
- `reservation`

So the compute market is not only "did the runtime finish?" It is also "what
kind of economic claim existed, and how did that claim close?"

## Status Legend

- `implemented`: shipped in current repo behavior or authoritative services
- `partial`: real code exists, but it is not yet the default end-to-end market
  path or not yet fully productized
- `planned`: target architecture or market lane, not yet credibly landed

Operational meaning of those labels:

- `partial` becomes `implemented` when the dimension has canonical authority
  objects, real delivery or settlement linkage, and product or operator surfaces
  that use that truth instead of bypassing it.
- `implemented, early` becomes truly mature only when adversarial handling,
  correction paths, operator inspection, and clear acceptance criteria exist.

Completion thresholds for the most important current partial lanes:

- sandbox becomes fully implemented when sandbox products can be advertised,
  procured, delivered, proven, settled, and inspected through the same market
  objects as other compute families
- clustered compute becomes fully implemented when topology, placement,
  proof posture, and settlement truth are market-visible rather than only
  runtime-visible
- validator regime becomes mature when proof-sensitive products routinely flow
  through validator and challenge handling with explicit economic outcomes
- buyer procurement becomes fully implemented when buyers can discover,
  compare, procure, and track compute claims through explicit product and
  instrument flows
- advanced instruments become mature when their rights, closure modes, and
  operator views are legible without reading source code

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

## Pricing And Procurement Semantics

The compute market is not defined only by supply objects. It is also defined by
what buyers can procure against and how price can be formed.

Canonical procurement targets:

- a buyer procures first against a `ComputeProduct` and its constraints
- a buyer may then target a specific `CapacityLot` if they want a concrete
  provider inventory unit
- successful procurement or allocation creates a `CapacityInstrument`, which is
  the actual economic claim the market later settles against

That means:

- `ComputeProduct` is the admissibility and comparability layer
- `CapacityLot` is the offer and inventory layer
- `CapacityInstrument` is the claim and settlement layer

At minimum, buyers should be able to procure against:

- a product family
- a concrete `ComputeProduct`
- a specific lot
- a delivery window
- a quantity
- a provider or provider class
- an environment binding
- a proof posture
- a validator posture
- a deliverability envelope such as latency, throughput, or concurrency limits

The repo already contains some of the pricing fields needed for that:

- `CapacityLot.min_unit_price`
- `CapacityInstrument.fixed_price`
- `CapacityInstrument.reference_index_id`
- `ComputeProduct.settlement_mode`

So the intended compute-market pricing family already includes:

- posted ask-like inventory
- direct quote and accept flows
- fixed-price spot procurement
- negotiated forward physical windows
- index-referenced settlement for cash-oriented instruments

What is not yet fully productized is the buyer-facing UX and matching layer that
make those semantics ordinary.

Exchange-like books or richer auction mechanics may belong later, but they are
not prerequisites for the market definition itself.

Procurement therefore is not "buying a job." It is buying a machine-execution
claim under explicit product, inventory, proof, environment, and deliverability
constraints.

## Deliverability Contract Dimensions

The compute market only works if "machine capacity" means something specific.

A real compute offer can vary along these deliverability dimensions:

- `Execution structure`: backend family, execution kind, topology kind,
  provisioning kind
- `Artifact lineage`: model family, model policy, environment binding,
  checkpoint binding
- `Performance envelope`: latency expectation, throughput expectation,
  concurrency limit, performance band
- `Verification posture`: proof posture, validator requirements
- `Market window terms`: region, delivery window, and related timing posture

Several of these already exist explicitly in the code through
`ComputeCapabilityEnvelope`, `ComputeEnvironmentBinding`, product fields, and
lot fields.

That means a buyer is not just procuring "compute." They are procuring a
machine promise under a bounded deliverability contract.

## Time Semantics

Time is a first-class part of the compute-market contract.

The main time boundaries are:

- delivery window: when capacity is supposed to be delivered
- offer expiry: when a lot is no longer procureable on the current terms
- challenge window: how long challenge-eligible delivery remains exposed to
  validator challenge
- settlement window: how long the market has to reach economic closure on the
  relevant claim
- correction window: when later evidence may still supersede a previous market
  interpretation

Some of these are already explicit in the code today through delivery windows,
offer expiry, and validator challenge timing. Others are still more policy and
market-semantics concepts than first-class standalone fields, but they are part
of the contract either way.

## Economic Risk Surfaces

The Compute Market feeds the Risk Market because it carries its own economic
risk surfaces.

Those include:

- delivery failure risk
- capability misrepresentation risk
- proof insufficiency risk
- validator disagreement risk
- settlement failure risk
- index manipulation or bad-reference risk

That is the main reason compute and risk remain separate markets while staying
deeply linked.

## Trust Classes, Proof Postures, And Admissibility

The compute market needs explicit trust classes because not all supply should be
treated as equally trustworthy or equally interchangeable.

The current code already exposes the core axes for this:

- provisioning kind:
  `desktop_local`, `cluster_attached`, `remote_sandbox`,
  `reserved_cluster_window`
- topology kind:
  `single_node`, `remote_whole_request`, `replicated`, `pipeline_sharded`,
  `layer_sharded`, `tensor_sharded`, `sandbox_isolated`,
  `training_elastic`
- proof posture:
  `none`, `delivery_proof_only`, `topology_and_delivery`,
  `toploc_augmented`, `challenge_eligible`

That gives the market a practical trust taxonomy:

- proof-light retail or local execution: simple local lanes with light proof
  posture and straightforward delivery expectations
- proof-bearing delivery: execution is accepted primarily through delivery
  records and bounded conformance checks
- validator-sensitive delivery: market acceptance depends on validator posture
  and challenge policy, not just provider-reported completion
- topology-sensitive clustered compute: the market requires topology truth in
  addition to delivery
- proof-augmented compute: topology plus compact proof material such as
  activation-fingerprint references
- challenge-eligible compute: market claims remain exposed to validator
  challenge and challenge-window outcomes
- environment-bound compute: admissibility depends on matching environment,
  dataset, rubric, or evaluator policy bindings
- cluster-backed execution with topology truth: placement and topology evidence
  are part of the machine promise itself

Admissibility is therefore not just "is a provider online?" It also includes:

- whether the product family is allowed
- whether the proof posture is allowed
- whether the validator policy is sufficient
- whether the environment binding is admissible
- whether the runtime and capability envelope actually match the market claim

## Failure, Dispute, And Settlement Taxonomy

The doc should name the main failure classes plainly.

Canonical failure classes:

- provider no-show or provider offline
- late delivery or expired delivery window
- partial delivery
- invalid, stale, or insufficient proof
- environment mismatch
- runtime identity mismatch
- sandbox or runtime failure
- buyer cancellation
- provider cancellation
- challenge rejection or challenge timeout
- settlement failure
- correction or reversal after later evidence

The current code already carries much of this as explicit reason families.

Current reason-code surfaces include:

- lot cancellation:
  `provider_unavailable`, `policy_disabled`, `market_halt`, `superseded`,
  `offer_expired`
- instrument closure:
  `filled`, `buyer_cancelled`, `provider_cancelled`, `curtailed`, `expired`,
  `defaulted`
- non-delivery:
  `provider_offline`, `capability_mismatch`, `policy_blocked`, `missed_window`
- settlement failure:
  `payment_timeout`, `receipt_rejected`, `non_delivery`,
  `cost_attestation_missing`, `adjudication_required`
- delivery variance:
  `capability_envelope_mismatch`, `partial_quantity`, `latency_breach`,
  `throughput_shortfall`, `model_policy_drift`
- delivery rejection:
  `attestation_missing`, `cost_proof_missing`, `runtime_identity_mismatch`,
  `non_conforming_delivery`
- index correction:
  `data_quality`, `manipulation_filter`, `methodology_bug`,
  `late_observation`

This is important because a compute market is not defined only by happy-path
delivery. It is defined equally by how it handles failure without lying.

Canonical remedy paths:

- cancellation before binding or before credible delivery
- rejection of non-conforming proof
- default when delivery or settlement obligations were not met
- adjudication-required handling when automated closure is insufficient
- correction or supersession when later evidence invalidates an earlier market
  conclusion

## Instrument Semantics

The two central economic object families after products and lots are
`CapacityInstrument` and `StructuredCapacityInstrument`.

`CapacityInstrument` is the primary market claim. In practice it can represent:

- a spot claim on near-term physical execution
- a forward physical claim on future execution capacity
- a future-cash exposure settled against a compute index
- a reservation right with bounded delivery or exercise semantics

The important point is that an instrument is not just metadata about a job. It
is the thing that says what economic right existed.

Plain-language instrument rights:

- spot: claim on near-term physical execution
- forward physical: claim on future physical capacity over a defined window
- future cash: exposure to compute price/reference movement without requiring
  pure physical closure
- reservation: a bounded right to reserve or claim future execution capacity

Terminal instrument behavior already exists conceptually through the lifecycle:

- close because it filled
- settle because the claim was economically completed
- default because delivery or settlement failed
- cancel because policy or counterparties closed it before completion
- expire because the window passed without successful closure

Transferability is not yet a strongly surfaced canonical property in the
current repo. Lifecycle and settlement semantics are the sharper truth today.

`StructuredCapacityInstrument` is the grouped exposure layer. In code it already
supports:

- `reservation`
- `swap`
- `strip`

And the current leg roles already encode meanings such as:

- `reservation_right`
- `swap_pay`
- `swap_receive`
- `strip_segment`

The UI does not yet fully explain these instruments to ordinary users, but the
kernel already treats them as more than theoretical placeholders.

For reservation-style or forward-style claims, exercise semantics also matter:

- exercise conditions define when the holder may claim or bind the capacity
- expiry conditions define what happens if that right is not exercised inside
  the allowed window

Those rules are part of the economic meaning even where the current product
surface does not yet expose them as richly as the underlying model can.

## Comparability And Inventory Normalization

One of the hardest unsolved problems in the compute market is not just creating
offers. It is making different offers comparable.

Two compute offers are not interchangeable merely because both say "inference"
or both say "sandbox."

Comparability has to account for at least:

- compute family
- execution kind
- topology kind
- provisioning kind
- proof posture
- validator requirements
- environment binding
- model policy or model family
- host capability
- latency, throughput, and concurrency limits
- settlement mode
- delivery window
- artifact lineage

This is why the capability envelope matters so much. It is the normalization
surface that lets the market compare claims without collapsing unlike things
into one fake commodity bucket.

Examples:

- local inference and clustered serving are not equivalent supply
- proof-light and challenge-eligible products are not equivalent supply
- sandbox execution and model-serving execution are not equivalent supply
- two environment-bound offers are not equivalent if their environment or rubric
  lineage differs

The market therefore has to own comparability as a first-class problem, not as
an afterthought in UX.

The short rule is:

> two compute offers are only economically comparable when their product
> descriptors, proof posture, environment compatibility, deliverability terms,
> and settlement semantics have been normalized enough to compare honestly.

## Artifact Lineage As Market Truth

For the compute market, artifact lineage is part of the economic claim.

Some compute products are only meaningful if the system can say exactly which
artifacts, versions, and policy surfaces were involved.

That lineage can include:

- model family and version
- model policy
- adapter version
- environment package version
- dataset and rubric bindings
- sandbox profile and runtime image
- checkpoint family
- proof bundle family
- benchmark harness version

This is already visible in the current data model through:

- `ComputeEnvironmentBinding`
- `ComputeCheckpointBinding`
- sandbox evidence refs
- verification evidence refs
- promised versus observed capability envelopes

Artifact lineage affects price, admissibility, and comparability, which makes it
part of the market claim rather than runtime metadata.

That is why the compute market is broader than "CPU/GPU time." It is really a
market in machine execution under a specific artifact lineage.

## Governance And Policy Ownership

The compute market also needs a plain statement about who governs admissibility
and corrections.

The current canonical answer is:

- kernel authority governs canonical market truth, receipts, lifecycle, and
  settlement outcomes
- policy refs and registries govern product admissibility, validator posture,
  environment admissibility, and related operating rules
- environment and eval registries govern package and evaluation lineage
- validator systems govern challenge execution and verification results
- index publication and correction remain authority-owned even when informed by
  external data or validator evidence

Governed surfaces include:

- admissible compute families
- admissible proof postures
- validator policy sets and validator pools
- environment-package admissibility
- index publication and index correction authority
- settlement remedies and remedy eligibility

Not every governance surface is fully productized yet, but this is the owner
split the doc should make explicit.

## Operator Truth Boundary

If operator truth is real, operators must be able to inspect the market without
reading code or reconstructing state from logs.

At minimum, operator surfaces should expose:

- inventory state
- product family, backend family, and compatibility envelope
- active lots and instruments
- delivery-proof posture and current proof status
- validator and challenge status
- cluster or sandbox health as linked market facts rather than isolated runtime
  facts
- settlement, dispute, correction, and default state

That is what "operator truth" means in compute-market terms.

## All-Rust And Psionic-Native Implications

The Prime/Psionic audit implies that execution substrate truth must live inside
the same Rust-native system that owns market authority.

That widening also changes what "implemented" must mean for future compute
families.

For clustered, sandboxed, environment-linked, eval-linked, and later
training-class products, mature implementation increasingly means:

- Psionic-native execution truth instead of opaque foreign runtime state
- receipt-bearing artifact lineage
- proof-aware and validator-aware control paths
- operator-inspectable runtime state
- settlement semantics that consume the same runtime and proof facts rather than
  bypassing them

In other words, later compute families should not merely be "supported by a
runtime." They should be legible market products inside a Rust-native authority,
proof, and execution stack.

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
