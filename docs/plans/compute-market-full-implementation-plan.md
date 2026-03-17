# Compute Market Full Implementation Plan

Status: proposed  
Date: 2026-03-07

Companion docs:

- `docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md`
- `docs/plans/prime-ecosystem-compute-integration-spec.md`
- `docs/plans/compute-market-launch-truth-checklist.md`

## Goal

Implement the OpenAgents Compute Market as a complete market surface rather than only a narrow compute-provider earn loop plus starter authority objects. "Complete" in this context means the system does not merely let a provider go online and get paid for a compute-shaped job. It means the repo exposes a coherent compute commodities stack with explicit product definitions, inventory, tradeable obligations, delivery evidence, price references, policy controls, and user-facing workflows that make those abstractions legible in Autopilot.

This plan deliberately starts from the current repo reality rather than an abstract greenfield ideal. Today the desktop MVP already does the most important emotional thing: a user can go online, take NIP-90 work, receive Lightning payment, and withdraw real sats. That loop is valuable and should remain the primary product wedge. At the same time, the kernel docs now define a much larger compute-market surface consisting of `ComputeProduct`, `CapacityLot`, `CapacityInstrument`, `DeliveryProof`, and `ComputeIndex`, with later extensions for forward contracts, cash-settled futures, reservation rights, swaps, and structured strips. The gap is no longer "does the repo know what compute is?" The gap is "can the product, authority layer, protocol layer, and observability layer all speak the same compute-market truth end to end?"

The implementation program therefore has two obligations that must stay in balance. First, it must preserve and strengthen the existing MVP earn loop, because that is still the product authority in `docs/MVP.md`. Second, it must progressively replace the current implicit compute assumptions with explicit compute-market objects and flows, so the compute lane can grow into a real market without rewriting the stack a second time.

## Why This Needs To Be A Program, Not A Patch

The current compute lane is real, but it is not yet canonical compute-market truth. The desktop currently produces the economically meaningful user experience through provider runtime behavior, labor-side kernel mutations, and wallet-confirmed settlement. The backend already has starter authority routes and passing tests for compute products, lots, instruments, delivery proofs, and indices. What does not yet exist is the connective tissue that makes those objects the default source of truth for live compute trading and live desktop behavior.

That distinction matters. A "compute market" is not fully implemented when the data structures exist in a crate and the test server can round-trip them. It is fully implemented when an ordinary user, a provider, a buyer, an operator, and a verifier can all reason about the same underlying inventory, pricing, delivery, settlement, and failure state without reading source code or inferring hidden behavior. Until then, we still have a compute-provider product, not a full compute market.

This is why the work must be treated as a staged program. There is no safe version of "ship futures and options next week" if spot inventory is not yet the canonical underlying truth, if delivery proofs are not yet automatically produced from live provider execution, if indices are not yet published from real market observations, and if the authority layer still runs in memory. The correct path is not to rush financial surface area. The correct path is to make the existing compute earn loop the first honest substrate of a broader market, then widen from there.

## What "Fully Implemented" Must Mean

A fully implemented compute market in OpenAgents should satisfy eight separate
tests at the same time.

First, the market must be real at the product layer. Providers need to be able to define what compute they are offering, under what constraints, in what region or performance band, for what window, and with what delivery posture. Buyers need to be able to discover that supply, request quotes, accept offers, or later participate in standardized market structures. The desktop must expose enough of that truth that the market does not feel like invisible backend state.

Second, the market must be real at the authority layer. The canonical lifecycle for inventory, obligations, delivery, settlement, corrections, and failures must live in authenticated authority mutations with deterministic receipts. It cannot remain an in-memory simulation if the product claim is that the compute market itself exists.

Third, the market must be real at the protocol layer. The current thin compute proto surface is enough to support the narrow MVP earn flow, but it is not enough to describe the full compute-market semantics already defined in `docs/kernel/economy-kernel.md` and `docs/kernel/economy-kernel-proto.md`. Full implementation requires explicit package coverage for compute products, capacity lots, instruments, indices, and delivery proofs, plus the read and mutation contracts around them.

Fourth, the market must be real at the observability layer. `/stats`, economy snapshots, and public operator-facing surfaces must expose market health in a way that is not limited to generic revenue counts. A mature compute market needs visibility into open inventory, accepted delivery, default behavior, price references, index quality, and eventually open interest and paper-to-physical posture.

Fifth, the market must be real at the policy layer. A compute market that can create obligations without clear controls over deliverability, attestation, index quality, or failure handling is not a market. It is a liability engine waiting to detonate. Full implementation therefore includes the bounded-risk side of the design, not only the happy-path trading side.

Sixth, the market must be real at the execution-substrate layer. It cannot stop
at single-node local inference. The product taxonomy, authority objects, and
delivery semantics must be able to express at least local, clustered, and
remote-sandbox compute supply with explicit provisioning kind, topology, proof
posture, and environment compatibility. That is the part of the plan most
strongly reinforced by the Prime ecosystem audit.

Seventh, the market must be real at the proof and evaluation layer. Delivery
cannot be defended only by optimistic completion state. The market needs
environment-backed evaluation objects, proof-bearing delivery bundles,
validator-compatible challenge hooks, and explicit settlement consequences for
proof failure or missing evidence where the product requires them.

Eighth, the market must be operable through first-class control surfaces. A
real compute market needs operator-grade CLI, inspectability, tunnel or attach
semantics where appropriate, and machine-consumable control APIs such as MCP
wrappers that reuse the same market truth rather than bypassing it.

## Prime Audit Incorporation Requirements

This plan now treats the compute-scoped conclusions of
`docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md` and
`docs/plans/prime-ecosystem-compute-integration-spec.md` as part of the
compute-market program rather than as adjacent architecture notes.

That means the compute market is not considered complete when it only has:

- spot lots,
- forward obligations,
- delivery proofs,
- and later hedging instruments.

It also must absorb the Prime-derived compute architecture in the areas that
are economically relevant to market truth:

- wider-network transport, relay fallback, and cluster membership
- collectives, shard placement, and public-network distributed serving
- bounded sandbox execution as a first-class compute product family
- proof, validator, and challenge-linked settlement infrastructure
- environment, eval, synthetic-data, and benchmark substrate
- operator control surfaces including CLI, MCP, and remote attach or tunnel
  semantics
- later training-class and adapterized compute families once the cluster,
  proof, and environment layers are already credible

The consequence is simple:

> this plan must own both the market objects and the reusable execution,
> proof, and evaluation substrate required to make those market objects
> economically meaningful.

## Guardrails

This plan should be executed under the following guardrails.

- Preserve the MVP core loop in `docs/MVP.md`: `Go Online -> paid job -> wallet tick up -> withdrawal`.
- Treat the current compute-provider lane as phase zero substrate, not as legacy behavior to discard.
- Scope the initial compute-market launch to the first two live compute product families, `inference` and `embeddings`, with `Ollama` and `Apple Foundation Models` as the two primary provider backends where support is truthful and policy-allowed.
- Keep product behavior in `apps/autopilot-desktop`.
- Keep reusable compute domain types, authority clients, and protocol shapes in crates rather than in app code.
- Do not move product workflows into `crates/wgpui`.
- Do not let Nostr or Spacetime become authoritative for money, settlement, or canonical compute obligations.
- Do not introduce unbounded leverage, opaque book state, or hidden insolvency.
- Do not ship derivative products before the underlying spot and forward physical layers are credible.
- Do not treat the Prime-derived cluster, sandbox, proof, environment, or
  operator layers as optional side quests if the goal is a full compute market;
  they are part of the required substrate.
- Keep wallet and payout truth explicit and authoritative in the UI.
- Maintain replay-safe, deterministic receipt and snapshot behavior on every phase.

The ownership boundary is straightforward. `apps/autopilot-desktop` owns compute-market UX, pane orchestration, and user-facing flows. `crates/openagents-kernel-core` owns reusable compute domain types, receipt helpers, validation helpers, and authority client contracts. `apps/nexus-control`, or the durable successor service if that responsibility migrates, owns authoritative state mutation and projection publication. `crates/openagents-kernel-proto` owns the generated wire layer. `crates/spark` continues to own wallet primitives and must not absorb compute-market product logic. `crates/nostr/*` continue to own Nostr transport and protocol primitives, not compute-market business logic.

## Initial Release Backend Scope

For the initial release, `compute` should be defined as machine-verifiable execution capacity offered under explicit capability and settlement constraints. The market is still the OpenAgents Compute Market. At launch, the first live compute product families inside that umbrella are `inference` and `embeddings`. The launchable provider product is therefore not "arbitrary CPU and GPU capacity," and it is not raw accelerator trading. It is backend-mediated compute capacity exposed through one of two concrete desktop-owned execution backends: `Ollama` and `Apple Foundation Models`. A user should only be able to go online as a compute provider if the local machine is serving one of those backends truthfully and the UI can identify which backend is active, which compute family is being offered, which model family or policy is available, and what constraints apply to the resulting supply.

That clarification should shape the entire plan. The initial compute taxonomy should not start with generic buckets like "GPU" or "CPU" as if the product can already clear those markets honestly. Instead, the first canonical product families at launch should be backend-specific compute products for `inference` and `embeddings`, for example `ollama.text_generation`, `ollama.embeddings`, and `apple_foundation_models.text_generation`. The market can still evolve into broader compute commodities over time, but the first release should model what the desktop can actually deliver today: machine-verifiable compute supply expressed through a small number of well-understood execution paths and standardized launch product families.

The repo already has meaningful Ollama runtime work in `apps/autopilot-desktop`, which makes Ollama the natural first implemented backend for the launch compute families. `Apple Foundation Models` also makes strategic sense for the initial release because it provides a first-party Apple path for on-device compute and maps well to the desktop-app framing of Autopilot as a local personal agent. Together, those two backends give the product a coherent initial story: cross-platform local model serving through Ollama, plus native Apple on-device execution where the platform supports it.

A useful constraint is that the Apple FM lane is not purely hypothetical. The retained tree now contains a live `swift/foundation-bridge/` package plus app-owned Rust supervision in `apps/autopilot-desktop`, and the archive still preserves the broader older bridge shape with `/health`, `/v1/models`, `/v1/chat/completions`, streaming SSE support, and additional session, tool, and adapter endpoints. That combined picture should inform the current plan. The compute market does not need every one of those endpoints on day one, but it does mean the Apple FM path can be framed as a desktop-owned local bridge or sidecar with known health checks, model discovery, and completion semantics rather than as an undefined future integration.

The most important implication of that history is scope discipline. For the initial compute-market release, the Apple FM lane should use the minimal subset of the historical bridge that matters to provider truth: explicit health and availability checks, explicit model or capability discovery, standard completion, optional streaming parity, and clear platform gating. Session continuity, FRLM tool routing, adapter management, and guided-generation schemas are evidence that the bridge once supported a larger surface, but they should remain non-goals for compute-provider launch unless a concrete provider workflow truly requires them.

The archive also contains three specific lessons worth adapting rather than re-inventing. First, the older bridge build/install path switched from copying the Swift binary into `bin/` to emitting a wrapper script that executed the binary from its `.build` directory because copying could break code-signature and entitlement behavior on macOS. Second, the older `ChatHandler` explicitly created a fresh `LanguageModelSession` per request to avoid context accumulation and context-window failures, which is directly relevant to the retained bridge's current shared-session behavior. Third, the older Rust-side bridge manager and `fm-bridge` client code had a cleaner discovery/supervision contract with user-installed, bundled, and development-path lookup plus a typed HTTP client surface. Those are the archive pieces most worth adapting for MVP. The old session, tool, and adapter endpoint surface is still useful as design context, but it should not be pulled forward wholesale.

If a third backend is ever added close to initial release, `MLX` is the only one that makes enough product sense to merit consideration. It is still compute in the same launch shape, still Apple-Silicon-oriented, and still conceptually adjacent to the first two lanes. Even so, the recommendation for the initial release is to defer it. Adding `MLX` immediately would widen test and policy scope without improving the clarity of the first market claim. The safer move is to make `Ollama` and `Apple Foundation Models` rock-solid first, then decide whether `MLX` deserves to become the third supported provider backend.

## Product Identity Versus Capability Envelope

Supporting accelerators like `H100` does make sense, but they should not displace the initial product identity. The clean model is to represent the launch market in two layers. The first layer is the tradable product identity: what the buyer is buying and what the provider is promising in market terms. For the initial release that should remain backend-specific compute products in the first live families, such as `ollama.text_generation`, `ollama.embeddings`, and `apple_foundation_models.text_generation`. The second layer is the capability envelope: the machine, runtime, and policy attributes that qualify or refine that product. That is where accelerator-level truth belongs today.

This separation gives the market a way to support hardware-sensitive procurement without lying about what is actually implemented. A provider might publish an `ollama.text_generation` or `ollama.embeddings` product with a capability envelope indicating `accelerator_vendor=nvidia`, `accelerator_family=h100`, `memory_gb=80`, a specific model family, a quantization posture, and a bounded context window. A different provider might publish an Apple-backed inference product with `accelerator_vendor=apple`, an Apple Silicon family, Apple Intelligence availability, and a different memory envelope. Those offers are not identical, but they still clear within an OpenAgents Compute Market whose first live product families are inference and embeddings rather than pretending the launch product is already a pure accelerator futures market.

The capability envelope should therefore become a first-class part of the compute taxonomy and matching logic. At minimum, the plan should carry fields for `backend_family`, `execution_kind`, `model_policy` or `model_family`, and a nested `host_capability` structure with fields such as `accelerator_vendor`, `accelerator_family`, `memory_gb`, and other runtime-relevant facts the product can honestly observe. For Apple-specific supply, the envelope should also be able to record platform-gating facts such as Apple Silicon requirement and Apple Intelligence availability. For Ollama-based supply, the envelope should be able to capture model-serving constraints, quantization, concurrency posture, and local runtime readiness.

This structure also gives the forward market a cleaner substitution model. In spot and forward procurement, buyers can request a compute product identity plus required or preferred capability-envelope constraints. In settlement and delivery disputes, policy can compare promised versus delivered capability envelopes rather than using vague arguments about whether a machine was "basically equivalent." That is the right place to express H100-like commitments in the initial design. If the market later earns enough depth to support direct raw-accelerator products, those can be introduced as new top-level product families rather than overloading the initial launch taxonomy.

## Current State Summary

The repo already has meaningful compute-market ingredients. The desktop has a working provider runtime, NIP-90 ingress, starter-demand flow, wallet-confirmed settlement gates, and a truthful mission-control loop. It also already contains substantial Ollama-specific provider runtime and execution logic, which is important because Ollama is one of the two intended initial-release backends. The kernel layer already contains reusable compute object types. The control service already exposes authenticated routes and passing tests for creating compute products, capacity lots, capacity instruments, delivery proofs, and compute indices. The docs already describe a much larger compute market that includes spot, forwards, futures-style instruments, options-style reservation rights, indices, and policy-bounded settlement.

The missing pieces are not trivial, but they are clear. The desktop does not yet surface explicit compute-market objects. The live earn loop still lands primarily in labor-side kernel records rather than creating the explicit compute-market lifecycle as its first-class underlying truth. The control service does not yet provide durable storage for canonical market state. The compute proto is still thin. The desktop does not yet expose provider inventory management, buyer-side spot procurement, price discovery, or market operations. There is now an active but partial `Apple Foundation Models` provider lane in the retained MVP code, backed by a retained `swift/foundation-bridge/` sidecar and app-owned supervision, but it is still not unified with the app's default local inference seam or fully productized for packaging and user-facing local-model UX. The retained productized path is still inference-led, and the first-class embedding family is not yet surfaced as a distinct live compute product family in the desktop. Market-wide controls such as index governance, deliverability controls, manipulation defenses, and phased instrument rollout still live mostly in docs rather than in active product and authority behavior.

The correct interpretation is that OpenAgents already has a compute-provider product and a starter compute authority slice. The work below is what turns those pieces into a full compute market.

## Program Thesis

The compute market should not be built as a second product that competes with the current earn loop. It should be built as the deeper economic substrate underneath that loop. The user who simply wants to "sell spare compute for sats" should still be able to do so through a simple `Go Online` experience. The difference is that after this program lands, the simple experience will map onto explicit compute inventory, explicit trade or allocation records, explicit delivery evidence, and explicit market observability.

That architecture matters for extensibility. Once the underlying spot compute truth is explicit, the system can support buyer-side procurement, scheduled capacity, forward physical commitments, compute price references, and later hedging instruments without inventing a separate state machine for each feature. The plan therefore starts by making the current loop canonical instead of ornamental. If we get that step wrong, every later market extension will either fork the truth model or inherit a weak substrate.

## End-State Shape

At the end of this program, the compute lane should look like this.

Autopilot providers can publish one or more standardized compute offerings that describe what they can deliver. Those offerings may be immediate spot capacity or future capacity windows. Buyers can discover supply, ask for quotes, or accept published terms. The authority layer binds the trade or allocation into explicit compute-market objects. When compute is delivered, the system emits delivery evidence that is linked to the underlying market obligation and any downstream WorkUnits that consumed the capacity. Market-wide price references are published from receipted observations rather than from ad hoc dashboard math. Policy can evaluate deliverability, concentration, attestation, and index quality based on explicit state instead of heuristics layered over job history.

The end state does not require the average earner to stare at a derivatives terminal. The mission-control loop should remain simple. But under the hood, the system should already be using the same underlying compute-market primitives that later power forward sales, inventory planning, index publication, and bounded hedging instruments. In other words, the simple product should remain simple because the architecture is disciplined, not because the complexity was ignored.

## Recommended Rollout Order

The rollout order should mirror the kernel's own staged recommendation. First make the current earn loop canonical compute truth. Then productize the spot market. Then add forward physical capacity. Then add indices and index governance. Only after that foundation exists should the system add cash-settled futures and eventually reservation options or swap-like products.

This ordering is not merely conservative. It matches the economic dependency graph. Spot fills and accepted delivery create the best eligible observations for indices. Indices and delivery credibility are prerequisites for safe cash settlement. Forward physical instruments need credible inventory, substitution rules, and failure remedies before they are worth anything. Options and swaps only make sense once the underlying spot and forward markets are legible enough to price.

The sections below therefore describe the implementation program as a sequence of phases with explicit deliverables and acceptance conditions.

## Phase 0: Make The Existing Earn Loop Canonical Compute Truth

Phase zero is the most important phase even though users may not experience it as a major UI expansion. The goal is to stop treating the current earn loop as a compute-adjacent labor path and instead make it the first honest spot-compute path in the system.

Today the desktop accepts a compute-shaped NIP-90 job, creates labor-side kernel records, executes the task locally, and finalizes settlement when the wallet confirms payment. That is good enough for the MVP, but it leaves the explicit compute-market layer optional. Full implementation begins by making every accepted compute job map onto explicit compute-market objects. A provider going online should be able to advertise or materialize a canonical default `ComputeProduct` for the supported runtime slice, a `CapacityLot` or equivalent spot inventory unit for the availability window, a `CapacityInstrument` or spot allocation when the job is accepted, and a `DeliveryProof` when execution is complete. `WorkUnit` and `Contract` should remain, but they should reference the underlying compute allocation rather than stand in for it.

This phase should also define the smallest standard compute taxonomy that the repo can defend today. For launch, `compute` remains the umbrella market category, while `inference` and `embeddings` are the first live compute product families inside it. The taxonomy should not pretend that the product is already brokering a general CPU market, a general GPU market, or open-ended heterogeneous accelerators. At minimum, the system needs a versioned way to describe the two launch backends as distinct backend families and the two launch compute families as distinct standardized products, with fields for backend family, execution kind, compute family, model identity or policy envelope, modality, quality or latency posture, context-window or token-window constraints where relevant, and any platform restrictions such as Apple-only availability for `Apple Foundation Models`. The taxonomy should also include a first-class capability-envelope structure so accelerator details such as vendor, family, and memory can be published, filtered, and compared without replacing the primary product identity. That taxonomy must be used consistently across provider advertising, lot creation, instrument binding, delivery evidence, and snapshot aggregation.

This is also where the desktop should settle how it wants to talk about "compute" to ordinary users. A provider should not see an abstract claim that they are selling "GPU capacity" if what the app can actually offer is launch compute products such as inference or embeddings through specific backends. The launch message should be stronger and more accurate: OpenAgents is launching the Compute Market with inference and embeddings as the first live compute product families. The market language should become broader only when the underlying execution backends and authority objects become broader.

The output of phase zero should be that a completed paid compute job is no longer just "a job that happened to use compute." It is also a receipted spot compute allocation with linked delivery evidence and settlement.

Phase zero deliverables:

- Define a forward-compatible compute taxonomy that can later express local,
  clustered, and `remote_sandbox` provisioning without a second namespace
  rewrite.
- Define the default standardized compute products that correspond to the currently supported provider capabilities.
- Define the two initial backend families as first-class compute products: `Ollama` and `Apple Foundation Models`.
- Define the first live compute product families at launch as `inference` and `embeddings`, with backend-specific product IDs such as `ollama.text_generation`, `ollama.embeddings`, and `apple_foundation_models.text_generation`.
- Define adjacent fields for provisioning kind, topology posture, proof
  posture, and optional environment binding so later clustered, sandboxed,
  proof-sensitive, and evaluation-linked products do not have to hide in opaque
  metadata.
- Define a capability-envelope schema that can express accelerator-aware supply, including fields such as `backend_family`, `execution_kind`, `model_policy` or `model_family`, `host_capability.accelerator_vendor`, `host_capability.accelerator_family`, and `host_capability.memory_gb`.
- Generate or publish canonical provider inventory records when a provider goes online, either as explicit lots or as controlled dynamic inventory projections.
- Update acceptance flow so matched compute jobs bind to explicit spot compute instruments or allocations.
- Emit compute delivery proofs from live execution completion, linked to the underlying lot or instrument.
- Link compute receipts to labor receipts and wallet settlement receipts so a single execution path can be traced from inventory to payout.
- Extend snapshots and `/stats` so live compute-provider jobs affect compute-market counters directly, not only generic earn counters.
- Add a backend selection and health model in the desktop so provider mode is truthful about whether the machine is serving through `Ollama` or `Apple Foundation Models`.
- Port or replace the historical Apple FM bridge shape as an app-owned local bridge or sidecar with explicit `/health`, model-discovery, completion, and optional streaming support.
- Make `Apple Foundation Models` a first-class implementation of the app-owned local inference seam on macOS rather than keeping it as a provider-only sidecar path.
- Restore the archived Apple FM operational lessons that still matter to launch quality: wrapper-style bridge packaging when entitlements require it, explicit per-request session isolation or reset semantics, and richer bridge discovery/supervision paths for bundled versus developer installs.
- Surface hard Apple FM provider blockers in the desktop, including macOS version, Apple Silicon requirement, and Apple Intelligence availability when that backend is selected.

Phase zero acceptance:

- A wallet-confirmed compute earn job produces linked `WorkUnit`, `Contract`, `CapacityInstrument`, `DeliveryProof`, and settlement references.
- A provider cannot advertise launch compute inventory for inference or embeddings unless one of the two supported backends is healthy and policy-allowed.
- A provider advertising accelerator-aware supply can expose a capability envelope without the market mislabeling that offer as a generic raw-GPU product.
- Mission Control remains simple, but the authority layer and receipt stream clearly show explicit compute-market objects behind the loop.
- Existing desktop earn tests remain green and new compute-linkage tests prove the object chain.

## Phase 1: Replace The Starter Authority Slice With Durable Compute Market Authority

The current control service demonstrates the compute market, but it is not yet a durable authority substrate. Compute objects, receipts, and snapshots are currently held in memory. That is acceptable for a test-backed starter slice. It is not acceptable for a market that claims to support inventory, forward obligations, delivery, indices, or later financial settlement.

The immediate requirement of phase one is to make compute-market state durable, restart-safe, and projection-friendly. This includes canonical receipt persistence, durable storage for compute products, lots, instruments, delivery proofs, and index publications, and consistent replay/rebuild behavior for compute-market read models. Whether this lands inside `apps/nexus-control` or moves into a longer-term owned Nexus authority service is less important than the properties of the resulting system. The compute market needs durable authority first; deployment topology can remain an implementation detail as long as the boundary stays explicit.

This phase should also convert the current write-only flavor of the starter authority routes into a fuller authority-plus-read-model system. The desktop cannot productize compute inventory or buyer workflows if the only available behavior is "create object and hope the receipt stream is enough." The service needs list/get/read surfaces or well-defined projection feeds for products, lots, instruments, indices, and delivery state. It also needs stable pagination and projection semantics so the desktop can render market views without scanning arbitrary receipt history on every refresh.

Phase one deliverables:

- Add durable persistence for compute-market objects and canonical receipts.
- Add durable snapshot persistence or durable snapshot rebuild on demand with explicit replay guarantees.
- Add query or projection surfaces for compute products, active lots, active instruments, delivery records, and recent index publications.
- Preserve idempotency and replay-safe mutation semantics across restarts and multi-request retry behavior.
- Add migration or compatibility handling so pre-existing starter receipts do not break the new compute authority layer.

Phase one acceptance:

- Restarting the service preserves compute inventory, instruments, delivery proofs, and receipt history.
- Snapshot and `/stats` values rebuild deterministically from durable truth.
- The desktop can consume a stable read model for compute inventory and compute trade state.

## Phase 2: Expand The Compute Proto And Client Contracts To Match The Real Market

The compute proto surface is currently too thin to describe the compute market defined in the kernel docs. `ComputeRequirement` is useful as a dependency of labor-side work descriptions, but it is not the same thing as a full compute-market protocol. Full implementation requires explicit package coverage for compute products, capacity lots, instruments, delivery proofs, and indices, along with the relevant request and response messages for mutation and retrieval.

This phase should not wait until every UI surface exists. The protocol contract is the skeleton that keeps the implementation aligned once more than one product surface starts consuming the market. It should therefore be expanded before the desktop grows deep spot or forward market UX. The kernel proto plan already sketches the desired package boundaries with `compute_products.proto`, `compute_capacity.proto`, `compute_instruments.proto`, `compute_indices.proto`, and `compute_delivery.proto`. This phase should make those boundaries real, generate the corresponding Rust types in `crates/openagents-kernel-proto`, and align the authority service and reusable authority client with them.

Just as importantly, this phase should settle the naming and lifecycle semantics before productization proliferates. The team should decide what counts as a product registration, what counts as a lot offer versus a dynamic presence projection, what the canonical instrument state machine is for spot and forward physical instruments, how delivery proof acceptance is represented, and how index publication and correction are expressed at the wire level. It is better to make these contracts explicit now than to let multiple app flows grow around ad hoc JSON payloads.

Phase two deliverables:

- Add explicit compute proto packages for products, capacity, instruments, delivery, and indices.
- Generate updated Rust types and wire them into `crates/openagents-kernel-proto`.
- Expand the reusable authority client in `crates/openagents-kernel-core` to match the new request and read-model contracts.
- Normalize compute receipt families and reason codes to match the canonical wire layer.
- Define stable wire contracts for index corrections, lot cancellation, delivery variance, and instrument closure.

Phase two acceptance:

- The compute proto package tree is broad enough to express the real compute market, not only labor-side compute requirements.
- The authority service, reusable client, and generated proto types agree on the same object model and lifecycle names.
- Future desktop compute-market work can build on generated contracts instead of hand-maintained JSON shapes.

## Phase 3: Productize The Spot Compute Market In Autopilot

Once the underlying truth model and authority layer are durable, the next step is to make spot compute an actual user-facing market rather than a hidden implementation detail behind `Go Online`. This phase is where providers should first gain explicit inventory management and buyers should first gain explicit spot procurement workflows.

For providers, the desktop needs a way to show what supply they are exposing. That does not mean every earner must learn commodity-market jargon on day one. It does mean the system needs a truthful screen where the provider can see which compute products they are currently advertising, how much inventory is open or reserved, which lots are delivering, what prices or price floors are attached, and which delivery or performance conditions apply. In the initial release that view should be backend-explicit and family-explicit: the provider should be able to see whether they are serving through `Ollama` or `Apple Foundation Models`, which launch compute families that backend can truthfully advertise, which local model or policy envelope is active, which accelerator or host capability envelope is being published, and which requests are eligible for that backend. In practice, that means `Ollama` can be positioned for inference and embeddings, while `Apple Foundation Models` is positioned as an inference backend at launch. The simple one-button online flow can remain the default onramp, but behind that default the desktop must expose a more explicit "what am I selling?" view.

For buyers, the desktop or another OpenAgents product surface needs a spot procurement flow that is more explicit than generic NIP-90 demand. The initial implementation should favor bilateral RFQ or controlled quote selection rather than a complex order book. The important part is that buyers can target a compute product family such as inference or embeddings, request a quantity and window, attach required or preferred capability-envelope constraints, review quoted terms, and bind the result into a canonical compute instrument. That procurement flow can later be automated by agents, but it should first exist in a human-legible way.

This phase is also where the desktop mission-control model must widen carefully. The compute market should not force the average earner to operate a trading terminal, but the desktop should still surface the right truth. Mission Control should remain the simple scoreboard. Separate compute-market panes can expose inventory, spot offers, accepted trades, delivery state, and pricing references for advanced users and operators.

Phase three deliverables:

- Add provider-facing compute inventory views in `apps/autopilot-desktop`.
- Add backend-specific provider controls and visibility for `Ollama` and `Apple Foundation Models` readiness, selected model or capability envelope, and local policy restrictions.
- Add provider controls to advertise inference capacity, embedding capacity, or both, with truthful backend-specific eligibility.
- Keep the inventory and RFQ model generic enough that clustered and
  `remote_sandbox` products can later use the same lot and instrument flow
  instead of forcing separate market state machines.
- Add accelerator-aware provider inventory fields and buyer-side filtering so offers can carry host capability constraints without turning the launch UX into a raw hardware exchange.
- Add buyer-facing spot compute request and quote review flows.
- Surface active products, lots, and spot instruments in the desktop with truthful source badges.
- Surface proof posture, topology posture, and environment compatibility in the
  product details when those fields are present, even if the initial launch path
  only productizes local supply.
- Add provider controls for price floor, delivery window, region constraints, and supported product classes where the runtime can honestly enforce them.
- Add buyer controls for quantity, delivery window, quality floor, and acceptable substitution rules where supported.
- Keep the one-button `Go Online` path as the default simple path while exposing deeper compute-market panes behind it.

Phase three acceptance:

- A provider can see and reason about the compute supply they are currently offering.
- A buyer can execute a spot compute procurement flow that creates canonical compute-market objects.
- The desktop can show spot-trade lifecycle truth without hiding behind generic job language.

## Phase 4: Automate Delivery Proofs, Metering, And Cost Integrity

A compute market is only as good as its delivery proof. Product and authority surfaces can be elegant and still fail if the system cannot reliably prove what was delivered, for how long, with what quality, and under what attestation posture. This phase turns compute delivery from a loosely inferred side effect of job completion into a first-class evidence pipeline.

The current repo already contains the beginnings of this design. Compute objects carry fields for attestation posture, cost proof requirements, and delivery evidence references. The kernel docs clearly say that delivery proof is the physical-settlement authority surface. What is still missing is the concrete runtime integration that produces those proofs from live provider execution in a reusable and defensible way. That runtime integration should be backend-aware from the start, because `Ollama` and `Apple Foundation Models` will expose different observability and evidence surfaces.

This phase should define the metering and attestation adapters that the current runtime can honestly support. For the first cut, the scope can remain narrow. If the current provider runtime only has a defensible path for a subset of the launch compute families or deterministic job execution, then the initial delivery-proof automation should only cover those slices. The key is to stop pretending that every provider execution is equally measurable. For each supported compute product, the system should specify what meter is used, which runtime signals are recorded, what attestation digest or cost proof reference is required, what variance is allowed, and what evidence bundle is attached to the resulting delivery proof. In practice that means separate delivery-proof profiles for `Ollama` and `Apple Foundation Models` rather than one flattened compute proof format that erases meaningful backend differences. It also means the delivery proof should capture whether the settled execution class was inference or embeddings and should record the promised and observed capability envelopes when accelerator-sensitive supply is part of the offer.

Phase four deliverables:

- Define per-product metering strategy for the current supported compute slices.
- Produce delivery proofs automatically from provider runtime completion for those slices.
- Link delivery proofs to execution evidence, runtime identity, attestation posture, and cost integrity inputs.
- Add delivery-proof acceptance or adjudication rules for variance and rejection cases.
- Make delivery proofs visible in read models, receipts, and desktop history where appropriate.

Phase four acceptance:

- Physical compute delivery is backed by explicit evidence, not only by optimistic job completion.
- Delivery-proof data is sufficient for future index eligibility and forward-contract dispute resolution.
- Rejected or partial delivery has explicit reason codes and remedy paths.

## Phase 5: Add Forward Physical Capacity Sales

Once spot truth, inventory, and delivery are real, the next major expansion is forward physical capacity. This is the first point where the compute market stops being only "jobs that happen now" and becomes an actual supply planning and procurement system. Providers can pre-sell future windows. Buyers can lock future access. The authority layer can bind those obligations into explicit contracts that mature into physical delivery.

Forward physical capacity should be the first non-spot instrument class because it is the most operationally grounded. It still terminates in physical delivery rather than in purely financial settlement, which keeps the market tied to actual capacity and actual performance. It also directly serves the product story in the Episode 213 framing: users are not only selling spare compute right now, they are selling future capacity with explicit terms.

This phase requires more than new rows in a table. Providers need inventory planning workflows for future windows. Buyers need procurement and acceptance flows for future periods. The authority layer needs collateral and non-delivery semantics. The desktop needs to show reserved, committed, and available future capacity clearly enough that providers do not accidentally overbook themselves. The market must also define the substitution posture for forward delivery. If a provider commits a launch compute product in the inference or embeddings family with a specific model policy or capability envelope and later delivers a materially different envelope, the remedy rules need to be explicit before the contract is ever accepted.

Phase five deliverables:

- Add forward physical instrument support to the authority layer and reusable types.
- Add provider tools to publish and manage future-window capacity commitments.
- Add buyer RFQ and acceptance flows for future delivery windows.
- Implement collateral or bond posture appropriate for forward physical obligations.
- Implement delivery assignment, curtailment handling, non-delivery remedies, and explicit closure receipts.
- Add desktop views for future committed versus available capacity.

Phase five acceptance:

- A provider can pre-sell future compute capacity with explicit delivery terms.
- A buyer can hold a future compute entitlement that later settles into actual delivery.
- The authority layer can distinguish spot fulfillment from forward physical fulfillment and handle failure explicitly.

## Phase 6: Build Compute Indices And Index Governance

Price references are not a cosmetic nice-to-have. They are required before the compute market can safely claim to support cash-settled hedges, market-level observability, or consistent policy inputs. The compute index therefore needs to be treated as a governed market object, not as a dashboard convenience.

This phase should first decide which observations count as eligible inputs to an index. The kernel docs already make the right directional argument: the system should prefer observations backed by real fills and real accepted delivery over thin indicative quotes. That principle should become code. The index pipeline should define which spot trades, accepted forward commitments, delivery proofs, or other market observations are admissible; how observations are normalized by product slice and window; how outliers are trimmed; how provider diversity is considered; and when an index window is too thin to publish high-confidence price references.

The system also needs correction and fallback rules before any financial instrument binds to an index. If an index is corrected, that correction must be a new receipted publication with supersession linkage. If a market window is too thin or too manipulated to publish a trustworthy index, the system needs deterministic fallback behavior rather than operator improvisation. Index quality must itself become visible in `/stats` so users and operators can see whether a given compute price reference is a serious market signal or a weak one.

Phase six deliverables:

- Implement index publication from eligible compute-market observations.
- Define and enforce methodology rules for observation eligibility, outlier handling, thin-market fallback, and correction.
- Publish index quality metrics and methodology versions.
- Add compute index read surfaces and desktop/operator visibility.
- Link index publications into snapshots, policy inputs, and later instrument settlement references.

Phase six acceptance:

- The system publishes receipted compute indices from explicit market observations.
- Index quality and correction behavior are visible and deterministic.
- Future cash-settled instruments can bind to an actual governed index rather than to an implied price feed.

## Phase 7: Add Cash-Settled Futures And Bounded Hedging Instruments

Cash-settled futures should only arrive after spot, forward physical, and indices are credible. This is where the compute market becomes a true commodities stack rather than a procurement system. It is also where the market can become dangerous if the team skips controls. The purpose of this phase is not to maximize financial surface area. It is to implement the bounded, receipted, policy-governed version of compute hedging already defined in the kernel docs.

This phase should begin with the smallest plausible set of standardized futures-style cash-settled contracts tied to the most liquid and best-measured compute slices. The system needs explicit contract units, settlement windows, reference indices, collateral posture, and correction rules. It also needs the authority and policy machinery to prevent paper exposure from outrunning credible underlying market depth. The kernel docs already define relevant control variables such as deliverable coverage ratio, paper-to-physical ratio, and index quality score. Those variables must become active operational controls before futures are considered launchable.

The desktop should be careful here. Most users should not need a derivatives-heavy interface. The product should likely expose these instruments first through advanced or operator surfaces, while the core mission-control experience continues to emphasize real earnings, inventory, and simple market activity. Financial hedges should remain legible and bounded, never disguised as harmless toggles.

Phase seven deliverables:

- Add cash-settled compute futures as a supported instrument class in the authority layer.
- Bind futures settlement to governed compute indices with explicit correction and fallback rules.
- Add collateral reservation and bounded variation settlement semantics where policy enables them.
- Add breakers and policy caps for open interest, paper-to-physical ratio, and index quality.
- Add advanced buyer and provider views for hedged positions, settlement windows, and realized cash settlement.

Phase seven acceptance:

- A cash-settled compute hedge can be created, monitored, and settled deterministically.
- The system can cap or halt issuance when index quality or deliverable posture deteriorates.
- Settlement receipts and `/stats` reflect hedging exposure and settlement outcomes honestly.

## Phase 8: Reservation Rights, Options, Swaps, And Strips

Reservation rights, options-style products, swaps, and structured strips should be treated as late-stage extensions, not as blockers for a complete first compute market. The compute market can already be considered broadly complete for most product purposes once spot, forward physical, indices, and bounded futures are in place. These later instruments exist because the kernel docs explicitly reserve room for them and because real compute markets may eventually need them for budget smoothing and long-horizon planning.

The reason to defer them is simple. These products multiply the importance of every earlier design choice. If product taxonomy, delivery proofs, substitution rules, index quality, or collateral boundaries remain ambiguous, structured products amplify that ambiguity into harder-to-debug failures. By the time this phase begins, the system should already have stable underlying slices, trustworthy indices, and proven delivery and settlement paths.

Phase eight deliverables:

- Add reservation-right or call-style option instruments where the product can honestly explain them.
- Add swap and strip support only as deterministic bundles of underlying legs, not as opaque synthetic positions.
- Preserve bounded leverage and explicit closure rules.
- Add clear UX boundaries so advanced structured products do not pollute the core earn experience.

Phase eight acceptance:

- Every advanced compute instrument decomposes into explicit underlying obligations and receipts.
- No structured product bypasses the bounded-risk posture of the underlying compute market.

## Cross-Cutting Workstream: Product And UX

The product workstream spans every phase because the compute market has to stay legible as it grows. Mission Control should remain the entry point and scoreboard. It should continue to answer the questions ordinary earners care about: am I online, what am I earning, what just happened, and can I withdraw? That simplicity is an asset and should not be sacrificed.

At the same time, a full compute market needs additional surfaces. Providers need inventory management, delivery history, price floor controls, future capacity views, and later hedge or commitment views. Buyers need product selection, quote review, trade state, and delivery or settlement history. Operators need market-health dashboards and breaker visibility. These should be additive panes, not replacements for the core loop.

The major product task is therefore not only "add more screens." It is "introduce more explicit market truth without making the first successful user journey worse." The beginner path should stay simple because the system chooses sane defaults and progressive disclosure, not because the compute market remains hidden.

Product deliverables:

- Keep Mission Control as the simple primary wedge.
- Add compute inventory panes for providers.
- Add buyer procurement panes for spot and forward flows.
- Add advanced panes for market activity, indices, and later hedges.
- Add clear state labels for inventory, commitment, delivery, settlement, and market risk.
- Ensure UI copy is explicit and truthful about what is guaranteed, reserved, delivering, or merely quoted.

## Cross-Cutting Workstream: Authority, Persistence, And Read Models

The compute market cannot become real without durable authority. This workstream is therefore foundational rather than optional. The authority service must own canonical mutation rules, canonical receipt persistence, and canonical read models or projections. It must also be explicit about which surfaces are authoritative and which are projections. Nostr and Spacetime can coordinate and mirror. They cannot be the authority for compute obligations or settlement.

The read-model question matters almost as much as the write-model question. Compute markets become unusable if every UI has to reconstruct market truth by replaying the entire receipt log independently. The service needs stable projection surfaces for active products, lots, instruments, delivery status, and index publications, with explicit replay and recovery semantics under the hood. This can still be projection-driven internally, but it must be product-consumable externally.

Authority deliverables:

- Durable canonical storage for compute objects and receipts.
- Projection surfaces for current market truth.
- Clear idempotency semantics for all compute mutations.
- Restart-safe snapshot computation or snapshot persistence.
- Explicit distinction between authoritative state, coordination state, and UI projections.

## Cross-Cutting Workstream: Protocol And Type System

The protocol surface is currently narrower than the compute-market plan. This creates a long-term risk: the repo may have strong docs and a growing implementation, but still force product code to depend on unstable internal shapes. The way to avoid that is to make the compute-market protocol explicit before the product surface becomes wide.

This workstream therefore covers more than just adding fields to a proto. It includes settling lifecycle names, state machines, reason codes, and request or response boundaries. It should produce a protocol surface that can survive multiple consumers and multiple rollout phases.

Protocol deliverables:

- Full compute proto package set.
- Generated Rust types.
- Updated reusable authority client methods and error types.
- Stable lifecycle enums for product, lot, instrument, delivery proof, and index state.
- Stable fields for provisioning kind, topology posture, proof posture,
  validator linkage, and environment binding.
- Stable reason codes for cancellation, variance, curtailment, non-delivery, manipulation, correction, and settlement failure.

## Cross-Cutting Workstream: Psionic Network, Cluster, And Collectives

The current plan cannot fully incorporate the Prime compute audit unless it
explicitly carries the cluster and transport substrate. Prime's most valuable
compute lesson is not just "sell more GPU products." It is that market-grade
compute supply eventually includes public-network sessions, elastic cluster
membership, shard placement, collectives, and topology-aware execution that the
market can actually settle against.

This workstream should rebuild the useful parts of:

- `protocol`
- `prime-iroh`
- `pccl`
- `prime-vllm`
- `prime-pipeline`

Market implications:

- clustered supply becomes a first-class compute product family, not a hidden
  runtime mode
- topology and shard placement become part of deliverability truth
- delivery proofs and substitution policy can reference explicit topology
  posture instead of vague cluster metadata

Cluster deliverables:

- Add Psionic-owned peer identity, session establishment, direct-connect plus
  relay-fallback transport, and cluster membership views.
- Add collectives and shared-state sync suitable for cluster-backed compute
  products.
- Add stage placement, shard manifests, and public-network pipeline semantics
  for clustered inference products.
- Make clustered capability envelopes, topology evidence, and delivery linkage
  visible to the kernel and desktop rather than trapped in runtime internals.

## Cross-Cutting Workstream: Sandbox And Remote Execution Products

The Prime audit also makes bounded sandbox execution a required part of the
compute-market shape, not only a convenience feature. The repo already has seed
material here, but the market plan needs to treat sandboxed execution as a
first-class compute family.

This workstream should rebuild the useful parts of:

- `prime-sandboxes`
- `prime-tunnel`
- relevant operator patterns from `prime`

Sandbox deliverables:

- Productize `remote_sandbox` as an explicit provisioning kind and compute
  family rather than hiding it behind app-local execution semantics.
- Move the long-term runtime engine into `OpenAgentsInc/psionic` while keeping
  provider-substrate as the reusable descriptor layer.
- Support background jobs, file transfer, artifact retrieval, attach or expose
  semantics, and bounded profile digests through a canonical compute contract.
- Expose sandbox evidence, environment compatibility, and proof posture through
  delivery objects and provider inventory surfaces.

## Cross-Cutting Workstream: Proof, Validators, And Challenge-Linked Settlement

The current delivery-proof phase is necessary but not sufficient. The Prime
audit requires proof-bearing execution artifacts, validator services, and
challenge-linked settlement paths to become part of the compute-market program
where economically justified.

This workstream should rebuild the useful parts of:

- `toploc`
- `toploc-validator`
- `gpu-challenge`

Proof deliverables:

- Define canonical execution-proof bundles for local, clustered, and sandbox
  compute.
- Add optional activation-fingerprint or similar compact proof adapters where a
  product requires them.
- Add validator-side challenge queues, challenge execution, and result receipts
  for proof-sensitive products.
- Widen `DeliveryProof`, market claims, and settlement logic so proof absence,
  challenge failure, or validator rejection have explicit economic outcomes.

## Cross-Cutting Workstream: Environments, Evals, Synthetic Data, And Data Plane

The Prime audit also makes it clear that compute is not just generic capacity.
Environment compatibility, evaluation output, synthetic-data generation, and
benchmark ingestion are all part of the compute market once products need to be
compared, verified, and sold against real workloads.

This workstream should rebuild the useful parts of:

- `verifiers`
- `prime-evals`
- `community-environments`
- `research-environments`
- `genesys`
- `evalchemy`
- `datasetstream`
- selected patterns from `prime-rl`

Environment and eval deliverables:

- Treat environment packages and environment refs as first-class compute-market
  bindings, not only side registries.
- Bind compute products and delivery proofs to environment compatibility where
  relevant.
- Keep evaluation-run, synthetic-data, and benchmark-adapter lifecycles as
  canonical compute outputs.
- Add a Rust-owned streamed data plane for eval bundles, tokenized corpora, and
  later training or checkpoint flows.

## Cross-Cutting Workstream: Operator Control, CLI, MCP, And Attach Surfaces

The Prime audit includes operator-surface lessons that the market plan should
carry explicitly. A real compute market needs inspectability, not only backend
objects and UI panes.

This workstream should rebuild the useful parts of:

- `prime`
- `prime-mcp-server`
- `prime-evals`
- `prime-tunnel`

Operator-surface deliverables:

- Widen `autopilotctl` and the desktop control plane to inspect inventory,
  cluster state, sandbox jobs, proof posture, validator outcomes, and compute
  receipts.
- Add an MCP wrapper over the same control contracts without bypassing kernel
  authority or policy.
- Add attach or tunnel semantics only where the product can expose them
  truthfully and safely.
- Keep the app-owned control plane primary so operator surfaces reuse the same
  market truth as the desktop instead of forking it.

## Cross-Cutting Workstream: Later Training-Class And Adapter Products

The Prime audit goes beyond inference and sandboxes. It also identifies later
training-class and adapterized compute families as part of the long-horizon
compute market. Those are not phase-zero or launch blockers, but they do belong
inside the complete market plan.

This workstream should rebuild the useful parts of:

- `prime-diloco`
- `prime-rl`
- `datasetstream`
- `cloud-lora`

Later-family deliverables:

- Reserve explicit compute-family space for `evaluation`, `training`, and
  `adapter_hosting`.
- Add checkpoint-bearing delivery semantics and environment-linked training or
  evaluation obligations only after the cluster, proof, and environment layers
  are credible.
- Add adapterized serving products only after artifact, serving, and settlement
  truth are already strong.

## Cross-Cutting Workstream: Market Structure And Matching

The kernel docs already recommend bilateral RFQ as the right first market structure. That recommendation should be followed. The compute market should not start with a complex central order book if simpler bilateral flows can make the underlying truth credible sooner.

This workstream therefore begins with RFQ and quote selection for spot and forward physical trades. Only after those flows are stable should the system consider deterministic auctions or batch clearing. If more advanced structures are later added, they must remain replayable and auditable with explicit clearing receipts and tie-break rules.

Market-structure deliverables:

- Spot RFQ and quote acceptance.
- Forward physical RFQ and binding.
- Deterministic quote comparison and selection logic under policy.
- Later, batch auction or clearing rules where liquidity justifies them.
- Explicit clearing receipts when matching becomes more than one-to-one quote binding.

## Cross-Cutting Workstream: Delivery, Metering, And Attestation

The delivery workstream is where the market stops being abstract. For physical settlement to mean anything, the system must measure what was delivered and preserve the evidence needed to defend that measurement later. This includes runtime-level metering, attestation posture, accepted versus promised quantity, performance variance, and explicit linkage into delivery-proof receipts.

This workstream also needs to stay honest about what the current runtime can and cannot attest. If certain provider environments cannot yet emit strong cost proof or runtime attestation, the market should not pretend otherwise. Instead, the product should expose the posture as it is and let policy govern which compute products require stronger evidence.

Delivery deliverables:

- Product-specific metering rules.
- Runtime adapters that emit delivery-proof evidence.
- Delivery acceptance and rejection semantics.
- Variance reason codes and remedy hooks.
- Strong linkage from delivery proof to underlying market obligation.

## Cross-Cutting Workstream: Policy, Risk Controls, And Breakers

A full compute market without market controls is not complete. It is merely exposed. The compute market therefore needs active policy support for permitted product slices, permitted instrument classes, maturity limits, attestation floors, collateral posture, index quality thresholds, deliverability posture, and breaker conditions.

The kernel docs already identify the relevant control surfaces. This workstream must turn them into implemented policy behavior and visible operations. That includes breakers for index quality, deliverability deterioration, paper exposure, manipulation signals, and attestation degradation. It also includes the bounded-response behavior when those breakers trip: halting issuance, forcing collateralization, narrowing allowed maturities, or disabling certain settlement modes.

Policy deliverables:

- Policy bundle support for compute-market controls.
- Breaker implementations and reason codes.
- Operator visibility into active restrictions.
- Receipt and `/stats` publication for breaker activation and clearance.

## Cross-Cutting Workstream: `/stats`, Economy Snapshots, And Public Observability

A full compute market requires public observability that is specific to compute, not only general to earnings. The kernel docs already define the right direction: spot table, forward or futures table, delivery integrity table, index integrity table, and hedging posture table. The current repo only implements part of that shape.

This workstream should progressively add compute-specific metrics as the market matures. In phase zero and phase three, the important visibility is active supply, accepted delivery, and price floor or fill behavior. By the time forward physical and futures exist, the system also needs open interest, deliverable coverage ratio, index quality, paper-to-physical ratio, default rate, and concentration metrics. The major rule is that the public story must match the real market. If the market claims to support a given instrument class, `/stats` should reveal enough truth for operators and users to understand its health.

Observability deliverables:

- Compute inventory and delivery tables.
- Compute pricing and fill tables.
- Forward and futures exposure tables as those instruments land.
- Index quality tables and correction counts.
- Provider breadth and concentration signals.
- Explicit source-of-truth labeling between authoritative and projected surfaces.

## Cross-Cutting Workstream: Testing And Verification

Compute-market implementation should be treated as a first-class correctness program, not only as product expansion. The more market structure we add, the more important deterministic tests become. There should be tests for object lifecycle transitions, idempotency, restart persistence, delivery-proof linkage, quote acceptance, forward-physical settlement, index publication, correction handling, breaker triggers, and policy enforcement.

The existing targeted tests are a good start because they prove the current compute-provider loop and starter authority flows. The next step is to widen that matrix so each new compute-market phase has its own end-to-end proof. The rule should be simple: no new market surface is considered real until there is a deterministic test that proves its lifecycle under success, retry, and failure conditions.

Testing deliverables:

- Unit tests for lifecycle state machines and reason codes.
- Authority tests for durable mutation and replay behavior.
- End-to-end tests for spot procurement and delivery.
- End-to-end tests for forward physical issuance through delivery.
- Index publication and correction tests.
- Futures settlement and breaker tests before any advanced financial rollout.

Validation gates that should remain part of compute work:

- `scripts/lint/workspace-dependency-drift-check.sh`
- `scripts/lint/ownership-boundary-check.sh`
- `scripts/lint/touched-clippy-gate.sh`

## Cross-Cutting Workstream: Documentation And Launch Truth

The compute market docs are already ambitious. Implementation work should keep them honest rather than letting docs drift further ahead of the product. Every major phase should therefore update the authoritative docs and the public launch language together. If a feature is backend-only, the docs should say so. If a market surface is productized, the docs and `/stats` should reflect it. If an instrument class remains planned, the public product should not imply it is live.

This workstream matters because the compute market is already a public-facing narrative in the repo. Honest sequencing protects trust. It is better to say "spot and forward physical are live; futures remain planned" than to create a doc set that sounds fully financialized while the product still operates mainly as a compute earn loop.

Documentation deliverables:

- Keep `docs/kernel/README.md` implementation status accurate.
- Update `README.md` and product messaging when each compute phase becomes real.
- Add explicit compute-market user documentation when provider inventory and buyer procurement land.
- Keep audits and launch materials aligned with actual delivered phases.

## Concrete Repo Work Packages By Ownership Boundary

The plan above is intentionally market-shaped. To execute it in this repo without boundary drift, the work also needs to be expressed in terms of concrete ownership. That translation matters because the easiest way to make a market plan fail is to let each new feature scatter state transitions across the desktop, wallet, transport, and kernel crates. The repo already has the correct broad ownership boundaries in `docs/OWNERSHIP.md`. Full compute implementation should lean into those boundaries rather than eroding them under delivery pressure.

`apps/autopilot-desktop` should own the visible productization of the compute market. That includes provider inventory panes, buyer procurement flows, mission-control truth, progressive disclosure for advanced market surfaces, and all local orchestration that turns user intent into authority mutations. It should also own the runtime-side production of delivery-proof inputs, because that logic is inherently tied to local execution and local user experience. For the initial release, that explicitly includes the app-owned execution adapters and health models for `Ollama` and `Apple Foundation Models`, plus the startup and supervision of any Apple FM localhost bridge or sidecar derived from the old `swift/foundation-bridge` design. What it should not own is reusable domain law. If the desktop starts carrying ad hoc copies of compute instrument state machines or index correction rules, the product will immediately begin forking the market truth.

`crates/openagents-kernel-core` should become the place where the reusable compute-market language is made precise. That includes the domain structs that already exist, the validation helpers that should prevent malformed product or lot definitions, the authority client methods that should hide service transport details from the desktop, and the receipt-linking helpers that should let tests and operators reason about a compute trade from creation through settlement. This crate is where the implementation should centralize lifecycle invariants such as which transitions are legal for a spot instrument, what fields are required for a forward delivery window, and how delivery variance is represented consistently across clients.

`OpenAgentsInc/psionic` should explicitly own the reusable execution substrate the
Prime audit calls out: transport, cluster membership, collectives, clustered
serve paths, bounded sandbox execution, proof bundle assembly, and later
training-class compute. The compute market plan is incomplete if it treats
Psionic as merely a local runtime while all cluster and sandbox semantics stay
implicit.

`crates/openagents-provider-substrate` should remain the narrow reusable
descriptor and provider-health layer. It should publish truthful provider
inventory templates and advertisability checks for local, clustered, and
sandbox products, but it should not become a second execution engine or proof
system.

`crates/openagents-kernel-proto` should own the generated protocol surface for the full compute market. This is more than a codegen concern. Once product, service, and tests all depend on the same generated package tree, it becomes much harder for the implementation to drift from the architecture described in the kernel docs. That is the real value of expanding the proto surface. It gives the repo a common language that is explicit enough to support multiple clients, durable enough to survive feature growth, and narrow enough to keep app code from inventing private payload shapes.

`apps/nexus-control`, or the durable owned successor when that split happens, should own canonical mutation, persistence, projection, and policy enforcement for compute-market authority. This includes durable object storage, durable receipts, deterministic replay, read-model publication, breaker logic, and index or settlement governance. The service should be the place where market validity is enforced, not merely where app requests are recorded. If a spot instrument is malformed, if a forward commitment exceeds policy, if an index correction supersedes a prior publication, or if paper exposure must be capped, the service should be able to say so authoritatively and emit a receipted reason.

Validator services and environment/eval services should be treated as first-
class compute-market infrastructure in the long-horizon owner map. They should
own challenge execution, verifier workloads, environment registry operations,
eval orchestration, and synthetic-data or benchmark pipelines, while the kernel
continues to own canonical economic outcomes and settlement truth.

`crates/spark` and the Nostr crates should stay narrow. `crates/spark` should continue to own wallet and payment primitives, invoice confirmation, and payout truth, but it should not absorb compute-market state machines. Likewise, the Nostr crates should continue to own relay connectivity, NIP-90 transport, and protocol-specific event handling, but they should not become shadow ledgers for compute obligations. Keeping those boundaries clean is not stylistic. It is what lets the market remain auditable when the implementation grows from a simple earn loop into a layered market surface.

Concrete work packages by owner:

- `apps/autopilot-desktop`: inventory panes, provider pricing controls, buyer RFQ flows, compute history, delivery-proof visibility, index visibility, advanced market panes, compatibility-preserving Mission Control updates, the initial-release backend adapters for `Ollama` and `Apple Foundation Models`, and supervision of the local Apple FM bridge lifecycle if that backend is revived as a sidecar.
- `OpenAgentsInc/psionic`: transport and relay-fallback sessions, cluster membership,
  collectives, clustered serving, sandbox execution engines, proof bundle
  assembly, and later training-class execution substrate.
- `crates/openagents-provider-substrate`: reusable provider descriptors, health,
  advertisability checks, and inventory-control helpers for local, clustered,
  and sandbox compute families.
- `crates/openagents-kernel-core`: expanded compute domain types, state-machine validation, authority client methods, receipt-linkage helpers, and reusable filtering or projection adapters.
- `crates/openagents-kernel-proto`: new compute proto packages, generated Rust types, service request or response contracts, and stable lifecycle enums or reason-code surfaces.
- `apps/nexus-control`: durable persistence, queryable read models, deterministic replay, market policy enforcement, index publication, correction handling, and breaker operations.
- validator services: proof verification, challenge execution, and challenge
  result publication for proof-sensitive compute products.
- environment and eval services: environment registry operations, eval-run
  orchestration, synthetic-data pipelines, benchmark ingestion, and streamed
  data-plane support.
- `crates/spark`: wallet-confirmation surfaces and settlement-linkage hooks only where needed to keep payout truth connected to compute receipts.
- `crates/nostr/*`: transport and event-ingress changes only where needed to carry richer compute references without moving market law into relay code.

## Operational Readiness, Rollout, And Migration

A compute market can be correct in code and still fail in production if rollout and migration are treated as an afterthought. The repo already has a live MVP flow that people can understand: go online, do work, get paid. That flow should not be broken by a sudden move to explicit compute objects, especially if the first release of those objects is only partially surfaced in the UI. The implementation therefore needs a migration posture that keeps the current earn loop alive while progressively shifting canonical truth underneath it.

The safest approach is phased activation behind explicit capability gates. Phase zero should land the new compute-object creation and receipt-linking path while keeping the existing earn-loop affordances unchanged. During that window, operators should be able to compare the old labor-centered reporting path with the new compute-centered reporting path and prove they reconcile. Only after that reconciliation is stable should the desktop begin treating explicit compute inventory and explicit compute trade state as the default visible truth. The same principle applies later for forward capacity, index publication, and bounded futures: each new surface should be dark-launched behind policy and operator gates before it is marketed as live.

Migration also needs to be honest about historical data. The repo already contains labor receipts, mission-control history, wallet-confirmed earnings, and starter compute objects from the transitional authority slice. The implementation should decide explicitly which of those histories will be backfilled into the new compute-market projections and which will remain legacy history. A partial backfill is acceptable if it is clearly labeled. A silent hybrid where some screens treat older jobs as compute trades and others do not is not acceptable. Historical truth and current truth need consistent labeling so users do not infer a continuity that the system cannot actually prove.

Operational readiness also means failure drills, not only happy-path tests. Before each market phase is called live, operators should know what happens when persistence restarts, when a delivery proof arrives late, when a provider goes offline mid-obligation, when an index window is too thin, when a correction supersedes a published price, and when policy halts issuance. Those drills do not all need full UI polish on day one, but the authority layer and internal runbooks need to exist before the market claims become broader.

Rollout deliverables:

- Feature gating for explicit compute-object canonicalization before it becomes user-visible default truth.
- Side-by-side reconciliation views or internal diagnostics that compare legacy earn accounting with compute-market receipts during migration.
- Explicit policy gates for forward physical, index publication, futures issuance, and advanced structured products.
- Historical-data labeling rules for legacy jobs, transitional compute objects, and post-migration canonical compute trades.
- Operator runbooks for restart recovery, delivery variance, non-delivery, index correction, and breaker activation.
- Release criteria that require observability and failure drills, not only functional demos.

Rollout acceptance:

- The current earn loop keeps working throughout migration with no ambiguity about payout truth.
- Operators can explain whether a given historical record is legacy labor truth, transitional compute truth, or canonical compute-market truth.
- New market phases are activated only when their authority, observability, and failure handling are already live.

## What Should Not Ship Early

Several tempting shortcuts should be avoided.

The team should not build a complex central order book before the spot RFQ path is solid. The team should not expose cash-settled futures before index quality, persistence, and deliverability controls are real. The team should not treat derivative volume as a substitute for real supply. The team should not hide weak attestation posture behind optimistic labels. The team should not replace the simple mission-control flow with a market terminal that only power users can understand.

The compute market will be stronger if it grows from real earned compute, real delivered compute, and real procurement flows. It will be weaker if it starts by emulating the shape of commodity finance without the discipline of delivery and evidence.

## Final Acceptance Criteria For A Fully Implemented Compute Market

The compute market should be considered fully implemented only when all of the following are true.

- The existing `Go Online` earn loop creates explicit compute-market objects as canonical truth.
- Providers can publish and manage real compute inventory in a user-facing way.
- Buyers can procure spot compute through a productized flow.
- Physical delivery emits explicit delivery proofs with linked evidence.
- The compute taxonomy and proto layer can express local, clustered,
  `remote_sandbox`, proof-sensitive, and environment-bound products without
  falling back to opaque metadata.
- Clustered and sandbox compute can be advertised and settled through the same
  canonical market object model as local supply.
- Proof bundles, validator outcomes, and challenge-linked remedies are explicit
  where the product requires them.
- Environment and eval objects are linked into compute supply and delivery where
  those bindings matter to the product being sold.
- Operator surfaces can inspect cluster, sandbox, proof, and settlement state
  without bypassing canonical market authority.
- Forward physical capacity can be sold and later delivered with explicit remedies for failure.
- Governed compute indices are published from explicit market observations with correction rules and quality signals.
- Cash-settled compute hedges can settle deterministically against those indices under bounded policy.
- Canonical compute-market authority state is durable and replay-safe.
- The compute proto and generated client layers cover the real compute market, not only a thin MVP slice.
- `/stats` and economy snapshots expose compute-market health beyond generic earnings metrics.
- Breakers, collateral posture, deliverability controls, and manipulation defenses are implemented and visible.
- Mission Control remains simple and truthful for the ordinary earner.

## Recommended Immediate Backlog Order

If execution starts now, the first sequence should be:

1. Canonicalize the current earn loop into explicit compute-market truth and widen the compute taxonomy for provisioning, topology, proof posture, and environment binding.
2. Add durable compute authority persistence and read models.
3. Expand the compute proto surface and generated types.
4. Widen `OpenAgentsInc/psionic` for transport, cluster membership, collectives, and clustered-serving truth.
5. Productize bounded sandbox execution as a first-class compute family alongside the local lane.
6. Add proof bundles, validator or challenge services, and challenge-linked settlement hooks for proof-sensitive products.
7. Bind environments, evals, synthetic-data, benchmark adapters, and streamed data-plane flows into canonical compute-market objects.
8. Extend `autopilotctl`, desktop control, and MCP-facing operator surfaces to inspect and operate the widened compute substrate.
9. Add provider inventory UX and buyer-side spot procurement UX.
10. Automate delivery proofs and explicit compute observability.
11. Add forward physical capacity sales.
12. Add compute indices and index governance.
13. Only then add cash-settled futures, later training-class products, adapterized serving products, and structured derivatives.

This order preserves the product wedge that already works while turning it into the first defensible layer of a broader compute commodities stack.
