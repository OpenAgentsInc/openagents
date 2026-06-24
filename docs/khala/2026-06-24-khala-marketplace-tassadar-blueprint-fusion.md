# The Other Half of the Store: Khala as the Marketplace's Demand Side

> Status: architecture essay + direction note, 2026-06-24. Companion to
> [`../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)
> ("The Store We Built Twice"). The audit sections cite live code; the
> ecosystem sections are labeled speculative and claim nothing. **Nothing here
> is a product promise, a served capability, or public‑claim copy.** There is no
> public plugin/skills marketplace today; the starter‑plugin catalog boundary
> ("does not imply public plugin publication, arbitrary external plugin
> admission, or a public plugin marketplace", psionic
> `docs/TASSADAR_STARTER_PLUGIN_CATALOG.md`), the Tassadar disclosure flow, the
> product‑promise registry, the evidence‑only Blueprint boundary, the Khala
> identity guard, and the INERT accepted‑outcome settlement machine all hold
> throughout. **FUTURE** marks speculation; **OWNER‑GATED** marks owner‑gated steps.

## I. The thesis: the audit built the shelf; Khala is the register and the shopper

["The Store We Built Twice"](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)
tells the marketplace's history from the **supply side**. It catalogs the goods
(digest‑pinned compiled modules), the shelving and labeling system (the
verification ladder, Tiers E/D/S/N), the inspection bench (conformance replay),
and the factory (the Tassadar workstreams) — and it is emphatic that the
storefront stays *closed* until the goods exist: "the store is the last thing
built this time, not the first."

This doc is the other half. A store is not only a shelf; it is a **register and a
shopper**. Someone has to walk in, decide what they need, pick the right item off
the right shelf, ring it up, and pay — and in a machine‑work economy that
someone is an agent, calling an API. **Khala is that surface.** Khala is the one
OpenAI‑compatible endpoint (`openagents/khala`, base `https://openagents.com/api/v1`)
that agents already call; it is therefore the natural place where demand for
machine capability lands, where capabilities are *selected and composed* into an
answer, where execution produces the receipts the audit's floor requires, and
where the per‑message revenue split actually fires. The fusion in one line:

> **Khala = Blueprint program execution × the Tassadar verification floor ×
> capability‑marketplace consumption.** Blueprint gives the typed program and the
> governance; Tassadar gives the proof that a purchased capability did what it
> claimed; Khala is the surface where a request becomes a program that *buys,
> composes, runs, and meters* those capabilities — and pays their authors in
> Bitcoin from the trace.

The audit's own §IV says the conjunction is the product: "The 2024 store's
payment loop and the 2026 lane's proof loop have now each run end‑to‑end — they
have simply never run *together*." Khala is where they run together, because Khala
is the thing that runs the message.

## II. Why the demand side has to be Khala (the buyer is constitutionally a Khala caller)

The audit's demand floor (§V, "What the buyer constitutionally cannot do") is the
load‑bearing economic claim: the buyers are agents, and frontier‑model agents fail
at exact computation *constitutionally* — they write code, pause, and trust a
sandbox for arithmetic they cannot check. A shelf of exact modules (ledger
transitions, protocol validators, schedule solvers) is aimed at exactly the
customer that cannot make those goods itself and whose purchase decision — "does
the digest match?" — is the cheapest diligence in commerce.

Now notice *where* that customer already is. An agent that needs inference calls an
OpenAI‑compatible endpoint. Khala **is** that endpoint, and it is positioned as
"one endpoint over a network of agents." So the demand for marketplace capability
does not need to be created — it is the inbound traffic Khala already receives.
Every Khala turn that needs an exact computation it should not hallucinate is a
**purchase waiting to happen**: instead of emitting a plausible‑but‑unverified
number, Khala routes the sub‑task to a Tier‑E module whose answer ships its own
replay receipt. The marketplace's demand aggregator is not a new storefront to
build; it is the inference surface we already run, taught to *shop* instead of
*guess*.

This is also why Khala must stop being a model‑alias router (the argument in the
[Khala brain audit](2026-06-24-khala-brain-and-blueprint-hookup-audit.md) and the
[extensibility note](2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)):
a model‑alias router can only answer from the base model's weights. A program that
*selects capabilities* can answer from the whole catalog — and pay for the parts
it used.

## III. A Khala turn as a marketplace transaction (fused into Blueprint)

The audit's §V lifecycle is written for the **author**: Submit → Admit → List →
Discover → Buy → Split. Read from Khala's side, the live half of that list is
Discover → Buy → Compose → Settle — and each step has a real Blueprint surface.

1. **Request → typed program.** A Khala turn becomes a typed Blueprint program
   call, not a raw prompt. *(The contract layer is LIVE —
   `apps/openagents.com/workers/api/src/blueprint/` — and the turn runtime
   `blueprint/services/chat-program-runtime.ts` exists; routing the Khala request
   path through it is the first integration milestone, still FUTURE.)*
2. **Discover by meaning, never by string.** The program's plan selects
   capabilities via the **typed semantic selector**
   `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`
   (`lookupBlueprintSignatures`) — by program‑signature ids, family, risk ceiling,
   surface, backend kind — so it finds "exact fixed‑point ledger transition,
   profile `core_i32.v0.3`" by meaning, satisfying the workspace no‑keyword‑routing
   rule. This is the same selector the audit's §V step 4 ("selection done
   semantically … not string match") describes. *(Selector LIVE; capability
   discovery riding it is FUTURE.)*
3. **Buy = replay before settlement (for Tier E).** The audit's §V step 5 is the
   purchase protocol: the buyer (or a validator they hire for dust) replays the
   conformance trace; the digest matches or it does not. Khala is the buyer here.
   The kernel already carries the binding to do this: `schemas/program.ts` exposes
   `BlueprintTassadarModuleStepBinding` and `BlueprintReplayModuleBinding` on a
   program tool scope, with executors `blueprint/services/tassadar-module-step.ts`
   and `blueprint/services/replay-module.ts` and a
   `blueprint/repositories/tassadar-module-registry.ts`. *(Bindings + executors
   LIVE in the kernel; wired into a Khala turn — FUTURE.)*
4. **Compose = the organ market.** A program plan that routes across several
   conformance‑tested, digest‑pinned modules is the audit's §V "Composition, and
   the organ market": a planner around frozen exact cores behind explicit ABI
   tokens (psionic `tassadar_module_linker.rs`). Khala is the planner's runtime
   home. *(Composition is open research on the supply side; Khala as the composer
   is FUTURE.)*
5. **Execute → decomposable receipt.** Tier‑E steps emit replayable traces; the
   whole turn produces a `BlueprintProgramRunRecord` (`authorityBoundary:
   'evidence_only'`) — decision evidence that never writes. Effectful (Tier‑N)
   steps do not act; they propose through approval‑gated Action Submissions. *(Run
   record + Action Submission LIVE‑in‑code.)*
6. **Settle = the split, computed from the trace.** The audit's §V step 6 is the
   move that fixes generation one: episode 098's 60/20/20 was declared by
   bookkeeping; in a trace‑native store the receipt *decomposes* — which module's
   steps ran in which spans — and the split is computed from the evidence. Khala is
   where that receipt is produced (it ran the message), so Khala is where the split
   is grounded. The money path rides the `omni-accepted-outcome-*` surfaces and the
   **8‑state INERT settlement machine**, behind NIP‑AC outcome‑scoped credit over
   Lightning. **OWNER‑GATED.**

The point of the list: steps 1–5 are evidence‑only and already have live
substrate; only step 6 moves money, and it stays behind the inert machine and the
promise registry. The marketplace can be *built and demonstrated* end‑to‑end
without a single sat moving — which is exactly the discipline the audit insists on.

## IV. The shelf Khala shops on is its routing policy

The audit's deepest design move (§V) is that **the marketplace's shelf structure
is the verification ladder itself**. For Khala that ladder is not a catalog; it is a
**routing‑and‑honesty policy**. When a program plan needs a sub‑task done, the tier
it picks *is* the claim it is allowed to make to the user:

- **Tier E — exact.** Compiled weight modules, replay‑verified. When Khala routes
  here, the answer *carries its own receipt*; Khala may say "this is exact" because
  a digest comparison backs it. This is the tier that fixes the constitutional
  buyer problem.
- **Tier D — deterministic.** The live `capability_free_local_deterministic`
  starter‑plugin class — same receipt shape, mechanical admission.
- **Tier S — statistical.** Psion‑class learned modules with bounded claims, sold
  with first‑divergence/eval receipts, never proofs. **The labeling law is a Khala
  invariant:** Khala must never let a Tier‑S result borrow Tassadar's exactness
  vocabulary in what it tells the user. This is the same discipline as the Khala
  identity guard, extended from "never name the provider" to "never overclaim the
  proof grade." A Khala that advertises a guess as exact has recreated generation
  one's trust problem at the demand surface.
- **Tier N — effectful.** Networked/writing capabilities under full Blueprint
  governance — Source Authority, Action Submission, approval policy, receipts.
  Khala proposes; it does not act.

So the refusal‑posture work in the brain audit and the marketplace meet here:
Khala's honesty about *what it can do* and Khala's honesty about *the proof grade
of what it just did* are the same property, and both are what make a paid answer
trustworthy enough to charge for.

## V. Khala is simultaneously the demand engine and the supply on‑ramp

The elegant part of fusing the marketplace into Khala is that **the same surface
that consumes capabilities is where new capabilities are born.** The brain audit's
loop — refusal → offer → guide → capture → distill → skill → list → earn — runs
through Khala:

- A Khala turn hits a gap (a capability the catalog does not yet hold). Instead of
  refusing, Khala offers to be guided.
- The guided session is captured as a deterministic trace (the executor‑trace loop,
  `apps/openagents.com/workers/api/src/artanis-scheduled-runner.ts`, already records
  replay‑verified traces; one paid Lightning closeout settled 2026‑06‑10,
  `compute.tassadar_executor_poc.v1`).
- The trace is distilled into a candidate typed signature + Module Version, enters
  Blueprint as an `optimizer_candidate`, is refined by GEPA, and is promoted only
  through a Release Gate (no self‑promotion).
- The promoted capability is listed (NIP‑SKL) on the right ladder tier and now
  serves *future* Khala turns — and its author (which may be the user who guided
  the original session) earns a share of every future use, decomposed from the
  trace.

So Khala is both ends of the marketplace: the **buyer** that creates demand by
shopping instead of guessing, and the **on‑ramp** that turns its own failures into
new supply that pays the people who taught it. A gap becomes a guided session
becomes a listed capability becomes recurring sats. That flywheel — consumption
reveals gaps, gaps become guided supply, supply serves consumption — is the
marketplace Khala is for.

## VI. The fusion in code (what is live, what is the wiring)

The substrate is unusually far along for something with no storefront:

- **Blueprint kernel (LIVE‑in‑code):** schemas, D1 repositories, mounted
  `/api/blueprint/*` routes, migrations — `apps/openagents.com/workers/api/src/blueprint/`.
- **The typed selector (LIVE):** `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`.
- **Tassadar ↔ Blueprint bindings (LIVE in the kernel):**
  `BlueprintTassadarModuleStepBinding` / `BlueprintReplayModuleBinding` on
  `schemas/program.ts`; executors `services/tassadar-module-step.ts`,
  `services/replay-module.ts`; registry `repositories/tassadar-module-registry.ts`.
- **The Khala turn runtime (LIVE but unwired):** `services/chat-program-runtime.ts`
  — exists, not yet called from the Khala request path.
- **Supply‑side honesty layer (FILED):** the `TassadarCapabilityEnvelope` consumer
  in Pylon (W4.1, openagents#4750) — capability advertised only with self‑test
  receipts.
- **Settlement (LIVE but INERT):** `omni-accepted-outcome-*` contracts/economics +
  the 8‑state settlement state machine; NIP‑SKL/NIP‑AC rails specced
  (`docs/nips/SKL.md`, `docs/nips/AC.md`).

The honest gap is the connective tissue: **no Khala turn is yet expressed as a
Blueprint program that selects a Tassadar capability and settles a decomposed
split.** Every brick exists; the building is not assembled. That assembly — not
new primitives — is the marketplace work on the Khala side.

## VII. What would kill it (from the demand side)

Mirroring the audit's §VI, the Khala‑specific failure modes:

- **Khala stays a model‑alias router.** If a turn never becomes a capability‑selecting
  program, there is no demand surface and no split to compute — just a model talking
  to itself. This is the default failure and the one to fight first.
- **The shelf is empty.** Khala can only shop a catalog that exists; the audit's "the
  shelf is nearly empty today" is the binding constraint. No Tier‑E inventory, no
  exact routing, no constitutional‑buyer product. Demand cannot precede supply here.
- **Keyword routing creeps in.** The instant capability selection degrades into a
  string switch on user text, the no‑keyword‑routing invariant breaks and the system
  becomes unauditable. Selection rides `signature-lookup.ts` or an explicit typed
  parser, always.
- **Proof‑grade inflation.** If Khala lets a Tier‑S result wear Tier‑E vocabulary in
  what it tells the user, it has rebuilt generation one's trust problem at the front
  door — with worse blast radius, because now a price is attached.
- **The split reverts to bookkeeping.** If the revenue decomposition is asserted by
  accounting rather than computed from the trace, generation one's "payments without
  proofs" returns. The receipt must decompose; the split must be arithmetic.
- **The sequencing trap, demand‑side edition.** The audit's deepest warning —
  building the storefront before the goods — has a Khala twin: building a Khala
  "marketplace UI" or charging for capability routing before a conformance‑tested
  module library and a real decomposed receipt exist. Any proposal to monetize
  Khala capability selection ahead of the proof floor is the 2024 mistake wearing an
  inference‑API costume.

## VIII. Where this lands / sequencing

This filings nothing and widens nothing. The build order, demand‑side:

1. **Route one Khala turn through `chat-program-runtime.ts`** and emit an
   evidence‑only `BlueprintProgramRunRecord`. The first Khala‑on‑Blueprint call.
   *(Evidence‑only; no money.)*
2. **Turn discovery onto `signature-lookup.ts`** so a Khala program selects a
   capability by typed selector — start with one Tier‑E/Tier‑D capability already in
   the starter catalog. *(No keyword routing.)*
3. **Wire one Tassadar replay step** (`tassadar-module-step.ts` /
   `replay-module.ts`) into that turn so the sub‑task's answer carries a replay
   receipt; surface the proof grade honestly to the user.
4. **Produce a decomposed receipt** — a `BlueprintProgramRunRecord` whose trace
   shows which capability's steps ran in which spans. *(Still evidence‑only; this is
   the split's input, not the split.)*
5. **Close the supply on‑ramp:** capture one Khala‑guided session into a candidate
   capability (the brain audit's loop), through GEPA + a Release Gate. *(FUTURE.)*
6. **Arm the settlement machine for one real decomposed split over Lightning.**
   **OWNER‑GATED.** The conjunction the audit names — proof loop and payment loop
   running *together* — fired once, on one message, with a receipt.

The audit closes: "The 2024 store proved the sats can flow. Blueprint proved the
contracts can hold. The Tassadar lane has now proven … that the goods can carry
their own evidence. The third store is the first one where all three are true at the
same time." Khala is the surface where all three become a single transaction — a
request that becomes a typed program that buys a proven capability and pays its
author from the trace. Same discipline as the first two generations closing
honestly: **receipts or it did not happen.**

## Pointers

- [`../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)
  — "The Store We Built Twice" (the supply‑side audit this companions).
- [`2026-06-24-khala-brain-and-blueprint-hookup-audit.md`](2026-06-24-khala-brain-and-blueprint-hookup-audit.md)
  — Khala's brain: refusal posture + Khala‑as‑typed‑Blueprint‑programs (the loop §V relies on).
- [`2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
  — Khala on the Blueprint/DSPy program system + Tassadar plugin extensibility.
- Live fusion code: `apps/openagents.com/workers/api/src/blueprint/` (schemas
  incl. the Tassadar module bindings, `services/{chat-program-runtime,tassadar-module-step,replay-module}.ts`,
  `repositories/tassadar-module-registry.ts`); selector
  `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`.
- Settlement + rails: `apps/openagents.com/workers/api/src/omni-accepted-outcome-*.ts`;
  `docs/nips/SKL.md`, `docs/nips/AC.md`; promises `docs/promises/registry.md`.
- Supply‑side capability honesty: `TassadarCapabilityEnvelope` (W4.1, openagents#4750);
  psionic `docs/TASSADAR_STARTER_PLUGIN_{AUTHORING,CATALOG,RUNTIME}.md`.
</content>
