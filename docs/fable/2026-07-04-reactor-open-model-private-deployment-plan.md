# Reactor — Private Open-Model Deployment for Businesses

Date: 2026-07-04
Status: product plan / draft in the Fable lane. **Nothing here is a product
promise, served capability, public claim copy, or a published price.** The
registry (`docs/promises/`) governs claims; every capability described below
enters as planned (or yellow where machinery already exists) through the
normal gates. This doc flips no promise state, broadens no copy, and files
no issues yet (§9 proposes the workstream map for a follow-up filing pass).

Codename note: **Reactor** is revived from an earlier owner concept (a plan
to install local/open models inside businesses). A full-history search on
2026-07-04 across the workspace, all child repos, the archive, and the old
clones found no surviving written spec — so this document is the canonical
first Reactor doc, written fresh against the current substrate. (The name
also happens to be the Terran add-on that doubles a structure's production —
a private Reactor bolted onto a customer's business so their own work runs
in-house, twice as fast. It slots beside Pylon/Khala/Artanis in the naming
system.)

Governing frames and companions:
[`2026-07-02-bf-3-4-private-sovereign-compute-tier.md`](./2026-07-02-bf-3-4-private-sovereign-compute-tier.md)
(the isolated-workroom/metering spec Reactor extends on-prem),
[`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) (BF-3 provision lane; vertical config
packs), [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) AW-0 (services engine),
[`2026-07-03-apollo-outbound-sales-plan.md`](./2026-07-03-apollo-outbound-sales-plan.md)
(Autopilot Lead Gen — Reactor becomes a target segment and upsell),
`docs/transcripts/external/2026-07-03-friedberg.md` (the market thesis,
verbatim from the buyer side),
`docs/transcripts/external/2026-07-04-mistral-ceo-enterprise-ai.md` (the
competitor playbook this doc's §10 reads and harvests), and Episode 247
(`docs/transcripts/247.md`, the sell-in-public revenue loop this product
feeds).

## 0. The one-paragraph product

**Reactor: your models, your data, your building.** We install and operate a
private open-model inference stack inside a customer's own infrastructure —
their server closet, their datacenter, their tenant of a cloud they control —
running the best open-weight models (NVIDIA Nemotron, Llama, GPT-OSS, Qwen,
DeepSeek, Mistral, Gemma, and successors) behind the same OpenAI-compatible
gateway shape Khala already serves, with a **typed model-provenance policy**
the customer sets on day one: some customers will say "US-origin models
only," some will say "Apache/MIT licenses only," some won't care — the
policy is a first-class, enforced configuration object, not a sales
conversation. On top of the private inference layer, the customer gets the
Autopilot fulfillment machinery (workrooms, approval gates, receipts,
fulfillment agents) grounded on their own corpus — which never leaves their
custody. Reactor is the answer to the buyer Friedberg describes: companies
that have woken up to the fact that handing proprietary data to frontier
labs commoditizes their core asset, and who are heading toward on-prem
inference "over the next couple of months" whether we show up or not.

## 1. Why now (the market thesis, from the buyer's own mouth)

The Friedberg transcript (All-In, 2026-07-03) is the cleanest statement of
the demand we keep hearing edge-on:

1. **Data custody is now a board-level "no."** Frontier labs are actively
   soliciting proprietary enterprise datasets (his example: Anthropic's
   life-sciences model program), and sophisticated buyers are refusing —
   "by handing it over… you are effectively commoditizing the asset that
   you have."
2. **The hub/spoke model is splitting.** His picture: a few large hubs do
   foundational training; medium hubs fine-tune enterprise-proprietary
   models; and a **distributed spoke layer runs inference on-prem** — "a
   set of servers… in your own data center or even in your own enterprise
   IT closet."
3. **The timeline is now.** "Everyone… is going to walk this path over the
   next couple of months."
4. **The competition is already selling it.** The Mistral CEO's 2026-07-04
   post (§10) makes the same case from the vendor side — closed providers
   "forcing data retention… gaining immense leverage on your business,"
   with "a track record of going after their most successful customers" —
   and packages the answer as a control plane + training platform deployed
   on customer infrastructure. The category is being named for us; our job
   is to sell it with receipts.

Our own corpus already carries the same signal from real buyers: the legal
design partner's first ask was a private pre-drafting workspace **on compute
they control**, trained on their own document corpus (the meditations doc,
§1); the health-vertical exemplar has a hard redaction-before-inference
requirement; customer #1's own problem list includes isolated VMs. BF-3.4
specced the cloud-side regulated-private lane. Reactor is the same product
one step further out: the customer's own hardware, open weights, zero
third-party model custody.

Open models are also, finally, good enough for the workloads these buyers
actually run (drafting, extraction, classification, RAG over their corpus,
internal agents) — the Nemotron/Llama/Qwen/DeepSeek/GPT-OSS tier plus a
per-customer fine-tune covers the 80% at a marginal cost that rounds to
electricity.

## 2. What Reactor is (and is not)

**Is:** a productized private deployment of the OpenAgents inference +
fulfillment stack: open-weight model serving behind an OpenAI-compatible
endpoint inside the customer's trust boundary, the model-provenance policy
enforced at provisioning and routing, corpus ingestion + redaction at the
boundary, Autopilot workrooms/approval gates/receipts running against the
private endpoint, and our fleet doing the install, upgrades, and operations
under a managed retainer.

**Is not:** a HIPAA/compliance certification (never claimed without the
audit receipts), a new money rail, a fork of the product per customer
(config, not fork — the standing BF-4.8 rule), a resale of anyone's
subscription capacity, or a replacement for the hosted Khala API (`openagents/khala`
remains the hosted model surface; Reactor is customer-side deployment).

## 3. The model catalog and the provenance policy (the differentiator)

The catalog is a curated, versioned registry of open-weight models with
**typed provenance metadata** per entry — because "which models may run on
our hardware" is a real procurement question with different answers per
customer, and nobody sells the answer as a policy object today.

Per-model metadata (the `model_provenance.v1` shape):

| Field | Example values | Why buyers care |
| --- | --- | --- |
| `originJurisdiction` | `us`, `eu`, `cn`, `fr`, `mixed` | Some customers (defense-adjacent, gov contractors, certain boards) mandate US/allied-origin only |
| `developer` | `nvidia`, `meta`, `openai`, `alibaba`, `deepseek`, `mistral`, `google` | Vendor-risk review |
| `license` | `apache-2.0`, `mit`, `llama-community`, `nvidia-open-model` | Legal sign-off; commercial-use terms |
| `weightsOpenness` | `open-weights`, `open-weights-restricted` | Can it be air-gapped, fine-tuned, redistributed internally |
| `trainingDataDisclosure` | `disclosed`, `partial`, `undisclosed` | Provenance-sensitive verticals |
| `distillationLineage` | upstream model refs where known | Some "US" models distill from restricted-origin teachers — the policy must see through the label |
| `evalRefs` | our own eval receipts per task class | Capability claims backed by receipts, not leaderboard vibes |

The **customer policy** (`reactor.model_policy.v1`) is a constraint set over
that metadata: allowlist/blocklist by jurisdiction, developer, license
class, disclosure level — plus routing preferences (quality-first vs
cost-first per task class). Examples:

- *"American-origin models only"* → `originJurisdiction ∈ {us}`, distillation
  lineage checked — catalog resolves to Nemotron, GPT-OSS, Llama, Gemma
  tiers.
- *"No Chinese-origin models"* → exclude `cn` — Qwen/DeepSeek/Kimi/GLM out,
  everything else in.
- *"Permissive licenses only"* → `license ∈ {apache-2.0, mit}`.
- *"Don't care — best model per dollar"* → empty constraint set; the router
  optimizes freely. (Many customers will pick exactly this; the point is
  that it is a **choice with a receipt**, not a default they never saw.)

Enforcement is structural, not aspirational: the provisioner refuses to pull
weights that violate the policy; the router refuses to route to a
non-conforming model; every policy decision and every model
install/upgrade emits a receipt naming the policy version it satisfied.
Policy changes are versioned customer decisions with sign-off, like every
other approval in the system. This is the semantic-selector discipline
applied to model choice — no ad hoc "we'll only use good ones, trust us."

Initial catalog seed (curation, not endorsement; every entry needs our own
eval receipts before it carries capability copy): NVIDIA Nemotron family,
Meta Llama family, OpenAI GPT-OSS, Google Gemma, Mistral/Magistral,
Qwen family, DeepSeek family, Kimi/GLM — spanning the policy space so every
constraint set above still resolves to a strong stack.

## 4. Architecture (owned seams, honest state)

```
Customer premises / customer-controlled cloud
┌─────────────────────────────────────────────────────────────┐
│  REACTOR NODE(S)                                            │
│  serving layer — Hydralisk lane by default (vLLM/SGLang/    │
│    TensorRT-LLM); Psionic lane by exception (§4.1)          │
│    └─ open-weight models per model_policy         [planned] │
│  OpenAI-compatible gateway + router                          │
│    └─ policy-enforced model selection             [planned] │
│  corpus store + redaction at boundary (BF-3.1/3.2)[planned] │
│  need-to-know access layer (hard rules + model    [planned] │
│    oracles; per-user scoped retrieval — §10 item 3)              │
│  workroom sidecar / lifecycle hooks (BF-3.4 shape)[spec]    │
│  metering → usage receipts (resource_usage_receipt.v1)      │
│    └─ receipts flow OUT; raw data NEVER does     [pattern]  │
└─────────────────────────────────────────────────────────────┘
        ▲ install/upgrade/ops by our fleet, approval-gated
        │ (air-gap mode: signed bundles, no callbacks)
OpenAgents side: catalog + policy registry, eval receipts,
Autopilot workroom/KPI surfaces, billing (cloud/openagents.com)
```

Reality check on the substrate (what this leans on vs invents):

- **Two owned serving lanes exist, with a clear default — see §4.1.**
  `hydralisk` is the standalone Python/NVIDIA inference lane (vLLM, SGLang,
  TensorRT-LLM, CUDA host runbooks, model profiles, smokes, public-safe
  receipts — it already serves the production `openagents/khala` lane on
  GCE) and is **Reactor's default serving layer**. `psionic` is the
  Rust-native ML substrate and remains the long-run owned-runtime path,
  used by exception where its properties are the point. Execution truth
  stays in psionic, conventional-serving truth in hydralisk,
  managed-node/ops truth in `cloud`, product surface in `openagents`.
### 4.1 Serving-lane policy: Hydralisk by default, Psionic by exception

Reactor has two owned serving lanes and must never blur them (the boundary
is already written into `hydralisk/AGENTS.md`: "Hydralisk exists beside
Psionic, not inside it").

**Default: the Hydralisk lane** (`hydralisk` — the standalone Python/NVIDIA
inference repo). Most customer Reactor nodes should run it, because for
conventional NVIDIA serving the Python ecosystem is simply the most mature
honest path:

- **Day-one model support.** New open releases (Nemotron, Llama, Qwen,
  DeepSeek, GPT-OSS successors) land in vLLM/SGLang/TensorRT-LLM within
  days, with quantization kernels, paged attention, tool-call parsers, and
  bugfix velocity no owned runtime can match this year.
- **It is already production-proven for us.** Hydralisk serves the live
  `openagents/khala` lane (vLLM on GCE L4) with model profiles, systemd
  runbooks, smokes, rollback, and public-safe receipts — the exact
  operational shape a customer install needs; RX-5's runbook extends what
  exists rather than inventing one.
- **Customer-side operability.** A customer's own IT staff (and future
  partner orgs) can reason about a vLLM/TensorRT deployment; hiring and
  vendor support exist for it. "The switch button in your hand" (§7 exit
  invariant) is more credible on a stack the world already knows.
- Hydralisk's own fail-closed discipline (explicit model revision, engine
  pin, image digest, GPU admission, quantization eval, receipt path per
  lane) is exactly the §3 policy-enforcement posture — RX-3 composes with
  it instead of fighting it.

**By exception: the Psionic lane** (`psionic` — the Rust-native ML
substrate). Choose it only when its properties are the point of the deal:

- **Non-NVIDIA / workstation-class targets** where the Python stack is the
  wrong fit (e.g. Apple-Silicon or CPU-constrained small-firm boxes).
- **Minimal-footprint / no-Python constraints**: air-gapped or
  audit-hardened environments that want a single static binary and a
  dependency surface a reviewer can actually read.
- **Verification-grade execution**: engagements that want Psionic's
  exact-execution/verification-by-replay direction as a differentiator,
  priced as such.
- **Strategic dogfood**: our own RX-6 node may run dual-lane so Hydralisk
  behavior becomes Psionic's behavior target — Hydralisk's stated role is
  producing evidence and targets for Psionic, and Reactor is the natural
  place that loop runs.

**The contract that keeps this clean:** everything Reactor-specific — the
model catalog, `reactor.model_policy.v1` enforcement, the gateway surface,
metering receipts, lifecycle events — is **lane-neutral by construction**
(RX-2/RX-3). A Reactor node declares `servingLane: hydralisk | psionic`
per model profile; policy enforcement and receipts are identical across
lanes; swapping lanes is a config change with an eval-gated cutover, never
a re-integration. Sales copy never leads with the engine: customers buy
custody, policy, and receipts — the lane is an implementation detail we
choose per the table above, disclosed honestly in the node profile.

- **BF-3.4 already defines** the isolation contract, lifecycle events, trust
  tiers, and metering receipt shape for the cloud-side regulated-private
  lane; Reactor reuses that vocabulary with a `customer_premises` placement
  class rather than inventing a parallel one.
- **The gateway shape is already our shape**: OpenAI-compatible endpoint,
  exact-only token accounting, typed policies. On-prem, the exactness
  discipline holds — metering receipts reconcile locally and project out as
  opaque totals only.
- **Hardware tiers** (guidance, not commitments): workstation-class (single
  pro GPU / Apple Silicon — small-firm drafting/RAG), server-class (2–8
  GPU box — the legal/health mid-market), rack-class (customer datacenter,
  quoted per engagement). Reactor sells outcomes on all three, not hardware
  — hardware can be customer-procured to our spec or bundled per deal.

## 5. What the customer buys (packages; modeled, owner-gated like all prices)

| Package | Shape | Modeled band |
| --- | --- | --- |
| **Reactor Assessment** (quick win) | Workload + data-custody audit, model-policy workshop (their provenance constraints become a signed `model_policy.v1`), hardware spec, pilot plan. Fixed scope, days. | $2.5–7.5k |
| **Reactor Pilot** | One node installed (their hardware or ours-specced), policy-conforming model set served, one real workload (e.g. the pre-drafting workspace pattern) grounded on their corpus behind their firewall, before/after receipts | $10–25k |
| **Reactor Managed** (retainer) | Ops, upgrades, model refreshes within policy, eval regressions on their tasks, metering/KPI reporting, fulfillment-agent lanes on top | $2.5–10k/mo |
| **Data Liberation** (quick win, standalone or bundled) | Migrate the customer's records out of walled-garden vendors into open, AI-accessible systems they control — the Mistral-named precondition ("your software vendors might block you… AI fortunately allows you to migrate quite fast"), and exactly fleet-shaped work: schema mapping, bulk export/transform, verification receipts per record class | $2.5–10k |
| **Harness evolution add-on** | The Mutalisk loop pointed at the customer's tasks: an offline proposer evolves the *harness* (deliverable landing, tool-call repair, context/matter fidelity, loop robustness) around their frozen policy-conforming model — one mechanism per iteration, accepted only on evidence, zero weight changes. The externally-published precedent (see `docs/research/2026-07-04-harness-optimization-evolve-the-harness-audit.md`): a frozen open model went 63.4%→80.1% on Harvey LAB, matching frontier quality at ~7× lower cost, with code mechanisms that **transfer across model swaps** — so the investment survives a provenance-policy change | quoted; rung zero of the improvement ladder |
| **Fine-tune / flywheel add-on** (later) | Per-customer adaptation on their corpus and — once running — the **continuous training flywheel**: improvement from their own interaction data, inside their boundary, producing weights *they own* ("the edges of your business into AI systems your vendors and competitors cannot replicate"); includes **distill-to-fit** — shrinking models to their observed input distribution to cut serving cost. Sequenced **after** harness evolution: weights carry the work only once the harness has demonstrably flattened | quoted |

The upsell path from the rest of the portfolio is natural in both
directions: services customers with data sensitivity graduate to Reactor;
Reactor customers consume Autopilot fulfillment, QA Swarm, and the
marketing packages on top of their private endpoint. And Reactor is a
**segment for Autopilot Lead Gen**: the outbound analyzer gains a
"model custody" conversation ("which frontier labs currently see your
data?") aimed at legal, health, biotech, finance, gov-adjacent — with the
Friedberg clip as third-party validation in the sequence. The named
outbound track is **Campaign B, "Own your AI"** (apollo plan §11):
founder-personal emails to 15–25 hand-picked accounts, an
independence-from-the-big-labs narrative, and deal shapes that extend the
bands above upward — Assessment $7.5–15k, Pilot + internal code forge
$25–75k, sovereignty retainer $5–20k/mo, replatform quoted — with the
custom-software quick wins folded in as sweeteners rather than headliners.

## 6. Fit with the two-product frame

Episode 246 named the frame: Khala Code plus **Autopilot, the business
operating system**. Reactor is Autopilot's substrate answer for the
customers who cannot or will not run on hosted inference — same workrooms,
same approval ladders, same receipts, different placement. Episode 247's
revenue loop gives it distribution: Lead Gen drives the funnel, partner
orgs (agencies) resell assessments into their client books, the Coding
Agent Pool builds the per-customer glue fast, and sell-in-public
case studies (opaque refs) compound demand. The one-engine rule holds:
**Reactor is a placement + policy configuration of the existing engine,
not a fork.**

## 7. Invariants (inherited + Reactor-specific)

All ROADMAP_BIZ §4 invariants persist (grounded-or-it-doesn't-ship,
approval-before-external, config-not-fork, no demo theater, commitments are
objects, no client-identifying info in-repo). Added:

- **Customer data never leaves the boundary.** Receipts, metrics, and
  opaque refs flow out; corpus, prompts, and outputs do not. Air-gapped
  mode must genuinely function (signed update bundles, no phone-home).
- **The model policy is enforced, versioned, and receipted** — provisioner
  and router both refuse non-conforming models; no silent substitutions,
  ever, including under incident pressure.
- **Provenance metadata is honest.** Distillation lineage and
  training-data disclosure recorded as known/partial/unknown — never
  laundered into a cleaner answer than the upstream facts support.
- **No compliance claims without audit receipts.** "Private" and
  "policy-enforced" are claimable when built; "HIPAA-grade"/"sovereign"
  are not, until the certification receipts exist (same rule as BF-3.4).
- **Exact-only accounting on-prem.** Local metering reconciles like the
  public counter does; `not_measured` over invented numbers.
- **Open-source posture holds.** The serving layer builds on open stacks;
  customer lock-in is operational excellence and the policy/eval/receipt
  layer, never data hostage-taking — we are selling the *escape* from
  that model, and copying it would be fatal to the pitch.
- **Exit-friendly by contract.** Knowledge transfer is a deliverable, not a
  concession: runbooks, configs, and weights hand over cleanly, and the
  engagement is designed so we *can* disappear once systems run — "the
  switch button fully in your hand" is the posture that wins this buyer,
  and our receipts discipline makes it verifiable rather than rhetorical.
- **Need-to-know is enforced, not assumed.** AI retrieval over a company
  corpus surfaces need-to-know errors mercilessly; per-user access scoping
  (hard rules in systems, soft rules checked by model oracles) gates every
  retrieval path before any org-wide assistant ships.

## 8. Honest gap list (what does not exist today)

1. No model catalog or provenance registry (the §3 schemas are this doc's
   proposal).
2. No policy-enforcing provisioner or router — the semantic-selector and
   entitlement patterns exist; this composition does not.
3. No packaged on-prem install (the stack runs where we run it; nobody has
   installed it in a stranger's closet).
4. No owned eval receipts across the catalog on customer-shaped tasks
   (psionic eval machinery exists; this suite does not).
5. No air-gap update path (signed release machinery exists for the desktop
   app; the node-bundle variant does not).
6. BF-3.1/3.2 corpus ingestion + redaction remain the shared prerequisite
   (already the BF-3 spine; Reactor raises their priority).
7. No pricing sign-off, no registry records, no copy. Everything above is
   sales-conversation-only until QS1-style records land.

## 9. Workstream map (RX — filed 2026-07-04 under epic [#8261](https://github.com/OpenAgentsInc/openagents/issues/8261))

Issue map: RX-1 [#8271](https://github.com/OpenAgentsInc/openagents/issues/8271),
RX-2 [#8272](https://github.com/OpenAgentsInc/openagents/issues/8272),
RX-3 [#8273](https://github.com/OpenAgentsInc/openagents/issues/8273),
RX-4 [#8274](https://github.com/OpenAgentsInc/openagents/issues/8274),
RX-5 [#8275](https://github.com/OpenAgentsInc/openagents/issues/8275),
RX-6 [#8276](https://github.com/OpenAgentsInc/openagents/issues/8276),
RX-7 [#8280](https://github.com/OpenAgentsInc/openagents/issues/8280),
RX-8 [#8281](https://github.com/OpenAgentsInc/openagents/issues/8281),
RX-9 [#8277](https://github.com/OpenAgentsInc/openagents/issues/8277),
RX-10 [#8278](https://github.com/OpenAgentsInc/openagents/issues/8278),
RX-11 [#8279](https://github.com/OpenAgentsInc/openagents/issues/8279).

| Task | Description | Gate/receipt |
| --- | --- | --- |
| RX-1 | Registry records: `reactor.private_deployment.v1` (planned) + model-policy and provenance record family; modeled rate card staged for the owner sitting | Records live at planned; no copy |
| RX-2 | Model catalog + `model_provenance.v1` / `reactor.model_policy.v1` schemas in a contracts package, with the initial curated seed and honest disclosure fields | Typed catalog with tests |
| RX-3 | Policy-enforced serving skeleton: one node profile (server-class) on the **Hydralisk lane** (§4.1 default; contracts lane-neutral with `servingLane` declared per profile), gateway + router refusing non-conforming models, exact local metering | A policy violation is structurally impossible in the smoke |
| RX-4 | Eval receipts: psionic-run task-class evals across the catalog seed (drafting, extraction, RAG, agent-tool-use). A score is a (model, harness) pair — every eval receipt carries a `harnessRef` naming the harness it was measured under (same-model harness variance ran 3.5%→80.1% in the 2026-07-04 harness-optimization audit) | Per-model, harness-attributed eval receipt refs the catalog can cite |
| RX-5 | Install/ops runbook + air-gap update path (signed bundles), fleet-executable | Clean install on a fresh box from the runbook alone |
| RX-6 | Dogfood deployment: Reactor node on our own hardware running a real internal workload under a strict policy (e.g. `us`-only) — customer number one, again | Metering + policy receipts from our own node |
| RX-7 | First customer pilot (likely the legal design partner's stated ask), BF-3.1/3.2 gated, opaque refs only | First paid Reactor receipt |
| RX-8 | Lead Gen segment: model-custody analyzer angle + Reactor sequence for regulated verticals | Quoted Reactor pipeline via Autopilot Lead Gen |
| RX-9 | **Need-to-know access layer**: per-user scoped retrieval over the corpus store — typed hard access rules enforced in the system + model-oracle checks for soft rules, with an adversarial "Bob must not see Alice" fixture suite | Access-violation smoke structurally fails closed |
| RX-10 | **Data Liberation offering**: walled-garden export/transform/verify pipeline as a packaged quick win (per-vendor adapters as config), verification receipts per migrated record class | First liberation engagement receipt |
| RX-11 | **Improvement ladder: harness evolution → distill-to-fit → flywheel.** Rung zero is the Mutalisk loop evolving harness-code mechanisms against the customer's tasks (evidence-gated candidates, one mechanism per iteration, cost term in the objective, transfer labels per model family — see the 2026-07-04 harness-optimization audit); then input-distribution-driven model shrinking; then the continuous-improvement training loop on customer interaction data (their boundary, their weights, consent recorded) — design + psionic/mutalisk hooks; no capability claims until receipts | Design doc + first dogfood harness-evolution receipt + first dogfood distill receipt |

Sequencing: RX-1/RX-2 are paper + schemas (start now); RX-3 gates RX-5/6/7;
RX-4 runs parallel in psionic; RX-6 (dogfood) precedes any external pilot,
per the standing pattern — we are always customer number one. BF-3.1/3.2
(ingestion + redaction) remain the shared critical path for any regulated
corpus touching any model, ours or theirs.

## 10. Competitive read: the Mistral playbook (harvested 2026-07-04)

Source: `docs/transcripts/external/2026-07-04-mistral-ceo-enterprise-ai.md`
— the Mistral CEO's enterprise-sovereignty post. Mistral is the
best-positioned direct competitor for exactly this buyer (their answer: the
**Studio** control plane + the **Forge** training platform — note the naming
collision with our own Forge; keep public copy unambiguous). Their argument
proceeds in five steps, and each one either validates a Reactor lane or
hands us a missing one:

1. **Open models or leverage** — closed providers forcing data retention
   "see it and learn from it" and have "a track record of going after their
   most successful customers." Validates §1; adopted as thesis signal 1.4
   and outbound copy raw material (their CEO making our pitch is
   third-party validation, like the Friedberg clip).
2. **Open data systems, not just open models** — walled-garden SaaS vendors
   can block AI access to *your own records*, and "AI fortunately allows
   you to migrate quite fast." This was our gap: harvested as the **Data
   Liberation** package (§5) and RX-10 — a fleet-shaped migration quick win
   that is also the natural first engagement for a customer not yet ready
   for a full Reactor install.
3. **Need-to-know access control** — "you don't always want Bob to see what
   Alice is doing"; hard rules in systems, soft rules checked by models.
   Previously implicit in our workroom visibility tiers; now explicit as
   the access layer in §4, an invariant in §7, and RX-9. This is a genuine
   hard problem ("hard and merciless") and therefore a durable service
   margin, not a commodity feature.
4. **The continuous training flywheel** — improving the customer's AI from
   its own interaction data, turning "the edges of your business into AI
   systems your vendors and competitors cannot replicate," plus
   **distill-to-fit** (shrink models to the observed input distribution to
   cut serving cost). Harvested into the §5 flywheel add-on and RX-11. Our
   twist is custody + receipts: the flywheel runs inside their boundary,
   the improved weights are *theirs*, consent and training runs are
   receipted — the same loop the Khala Code free-plan story tells, sold
   privately with the ownership inverted to the customer.
5. **Knowledge transfer and the exit** — "we transfer knowledge… we can
   disappear once the systems are up and running… the switch button can be
   fully in your hand." Adopted as the exit-friendly invariant (§7). This
   is where we can out-credential them: they assert it; our promise
   registry, receipts, and open-source posture let a customer *verify* it.

Where we differ and should say so: Mistral sells its own models first and
its zero-data-retention hosted tier second; Reactor is **model-neutral by
construction** — the provenance policy (§3) treats Mistral's models as
catalog entries like Nemotron or Llama, and neutrality is exactly what a
"no single-vendor leverage" buyer is shopping for. Their "applied AI
engineers working hand-in-hand" is a headcount-shaped delivery model; ours
is the fleet + operator-minutes discipline (BF-9.4) — the same agency-trap
falsifier applies to Reactor engagements as everywhere else.

## 11. Non-goals

- No HIPAA/FedRAMP/SOC2 or "sovereign" certification claims from this doc.
- No hosted-Khala changes: `openagents/khala` stays the single hosted model
  surface; Reactor never forks the model story.
- No hardware-resale business as a primary motion (bundling per-deal only).
- No training-data acquisition from customers — the entire point is that
  their data stays theirs; fine-tunes belong to the customer.
- No new money rails; billing rides the existing cloud/openagents.com
  surfaces.
