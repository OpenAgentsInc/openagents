# Decentralized model serving (shard-WAN + every-Pylon-serves) folded into the inference gateway

Date: 2026-06-19. Status: **initial thinking doc**, not a spec. Third supply pillar
companion to `2026-06-19-inference-gateway-business.md` (the gateway/credits business)
and `2026-06-19-pricing-vs-factory.md` (the pricing/margin model). Where those two cover
**Vertex quota** and **partner passthrough** as supply, this doc covers the third lane:
**our own decentralized serving fabric** — every Pylon can load weights and serve
inference, with large models split across many Pylons via the shard-WAN pipeline.

> Two honest framings up front. (1) The **execution substrate** for sharded serving is
> owned by Psionic and is mostly **planned / partly hardware-blocked** — see
> `../../../psionic/docs/audits/2026-06-19-shard-wan-pipeline-implementation-roadmap.md`
> (the 808-line roadmap; most large-model phases are status `planned`). This doc does not
> claim that capability exists today. (2) What we *can* design now is the **product-layer
> fold-in**: how serving-fabric supply routes through the gateway, how serving Pylons earn
> Bitcoin revshare through the revenue-loop spine, and how referral revshare attaches — so
> that as Psionic's phases land, the money plumbing is already shaped to receive them.

## 1. The vision — every Pylon serves inference

Today a Pylon node runs coding-agent execution and the Tassadar exact-execution substrate.
The next capability: **a Pylon can load model weights and serve inference**.

- **Small models, served whole on one Pylon.** A Pylon with enough RAM/VRAM loads a small
  model (a small Qwen/Gemma-family target is the recommended first correctness lane per the
  Psionic roadmap) and answers requests end-to-end. Single-node, low-risk, near-term.
- **Large models, served sharded across N Pylons (shard-WAN pipeline).** No single Pylon
  holds the whole model. Contiguous transformer **layer-blocks** are assigned to different
  Pylons (stage 0 holds layers `[0..a)`, stage 1 holds `[a..b)`, …). A token's hidden-state
  activations stream stage → stage; the tail produces logits; the coordinator selects the
  token. WAN round-trip latency is hidden via speculative decode (draft-K then verify in one
  traversal), direct-return (tail → coordinator without traversing the ring backward),
  async in-flight pipelining, and topology-aware route selection. Psionic implements this
  Rust-native across `psionic-cluster` (planning/topology/receipts), `psionic-net`
  (authenticated activation transport), `psionic-runtime`/`psionic-models` (per-stage
  layer-block execution + KV lifecycle + exact-greedy parity), and `psionic-serve`
  (coordinator serving + OpenAI-compatible mapping).

The product consequence: **the Pylon network becomes a decentralized model-serving fabric**
— a third inference supply source for the gateway, sitting alongside Vertex quota and
partner passthrough, and the **only** supply lane where margin can fan back to contributors
(because we, not a third party, own the compute).

## 2. How it folds into the gateway/credits business

The gateway (`2026-06-19-inference-gateway-business.md` §4) routes each request to the
cheapest viable supply. That doc named two lanes; this adds the third:

| Supply lane | What it is | Margin shape | Status |
| --- | --- | --- | --- |
| **Vertex quota** | First-party Google Cloud / Vertex Anthropic quota (project `openagentsgemini`) | Best margin; structurally below retail | Live quota; standard hosted-model reselling |
| **Partner passthrough** | Direct Anthropic / OpenAI / others | Lowest margin (pure passthrough); maximal coverage | Available |
| **Our serving fabric (this doc)** | Pylons serving inference — whole-small or shard-WAN-large | **The lane where margin fans to contributors** | Whole-small: near-term; shard-WAN-large: Psionic-planned |

Routing precedence is unchanged from the gateway doc's "cheapest viable supply" rule; the
fabric slots in as a routable target. Two practical placements:

- **Near-term, low-risk start:** route **small/cheap-tier** model requests to whole-model
  Pylon serving, and keep **large frontier** requests on Vertex quota + partner passthrough.
  This mirrors the pricing doc's finding (`2026-06-19-pricing-vs-factory.md` §2) that the
  margin lives in the **cheap/open tier** — and here that cheap tier *is our own network*,
  so its margin is exactly what we can fan to contributors.
- **Higher-value capability as Psionic delivers it:** as shard-WAN large-model serving lands
  (Psionic Phases 5–12), the fabric can also serve **large open-weight** models we don't
  hold Vertex quota for, splitting them across many smaller Pylons that individually could
  never hold the whole model. That is the differentiated, hard, high-value lane.

The gateway treats the fabric as a supply adapter like any other: it asks the fabric "can
you serve model M now, at what posture and price?" and the fabric answers with a plan (which
Pylons, which layer-blocks, privacy posture). The gateway does **not** reach into Psionic
execution — it consumes the fabric's offered plan and, after the run, its receipt.

## 3. Revenue model built in — serving Pylons earn Bitcoin revshare

This is the first key ask. **A Pylon that serves inference earns a revshare cut, paid in
Bitcoin, settled through the revenue-loop spine (EPIC #5457, RL-1/2/3).**

### 3a. The payment gate: born-verified

Payment clears against a **checkable outcome**, not a self-report. Psionic's non-negotiable
acceptance gate is **exact-greedy parity**: a sharded/speculative run must produce tokens
**identical** to the same-engine reference greedy decode (roadmap Phases 1, 3, 6, 8;
"Definition of Done"). The gateway requires that the serving plan declares a verifiable mode
and that the run's receipt carries the parity check (where a same-engine reference is
feasible) before any revshare dispatches. No parity / no reference ⇒ the run can still be
served but is flagged, and payout policy decides whether to pay against weaker evidence —
defaulting to **pay only against a checkable outcome**.

### 3b. The proof of who served what: per-stage receipts

Psionic emits typed receipts that are the **dereferenceable proof** the product layer pays
against. From the roadmap (§6 "Receipts And Proof Posture", schema names
`psionic.serve.pipeline_sharded_run_receipt.v1`, `psionic.cluster.topology_measurement.v1`,
`psionic.serve.spec_decode_round_receipt.v1`), a sharded run receipt records:

- the **nodes** that participated (more than one admitted node) and their **layer ranges**
- **GPU UUID** / residency (a stage must refuse layers outside its range — no whole-model
  fallback faking a split)
- **RTT edges** / measured topology facts used for placement
- **model** artifact digest, **quantization**, declared backend
- **throughput** windows (warm/cold, prompt length, generated tokens, decode mode)
- **prompt/output token hashes** + exact-reference parity result

This is exactly the data the payout split needs: it names **who served which layer-block**.

### 3c. The payout split across pipeline stages

For a whole-model Pylon, the contributor cut is simple: one node served the whole request,
it gets the fabric-lane contributor share of that request's margin.

For a shard-WAN run, the contributor cut is **split across the stages**, each Pylon paid for
its layer-block contribution. The receipt is the apportionment input. A defensible default
split is **per-layer-block weighted** (a stage holding more layers, doing more compute per
token, earns proportionally more), with the coordinator/draft roles paid for their distinct
work (token selection, draft proposal) under the same plan. The exact weighting (pure
layer-count vs FLOP-aware vs latency-contribution) is an open product decision (§8); the
receipt carries enough to compute any of them deterministically and reproducibly.

### 3d. Settlement through the revenue-loop spine

The money path reuses the already-wired clearing layer (EPIC #5457 CLOSED; RL-1/2/3):

- **RL-2 (firm-up escrow → Bitcoin):** a customer's Bitcoin-funded request that the fabric
  served produces accepted-outcome economics; the contributor cut firms up from escrow and
  dispatches as a **Bitcoin payout** to the serving Pylon(s), split per §3c.
- **RL-3 (asset-boundary / no-resale guards, live):** Bitcoin revenue fans out on the
  Bitcoin-only revshare path; the credit↔Bitcoin asset boundary is enforced. API-inference
  resale is the **allowed** lane under the no-resale invariant (subscription-seat resale is
  the forbidden lane; API/OpenRouter-style inference resale is permitted) — so a serving
  Pylon earning Bitcoin off inference it served is invariant-clean by construction.
- **Honest gate:** the spine is wired but the **first real dispatched payout is owner-armed**
  (JUNE19_ROADMAP: "the first real dispatched payout is owner-armed … `NEEDS-OWNER:` arm
  the first gated payout"). Fabric revshare inherits that gate — design is ready; the first
  serving-Pylon Bitcoin payout is owner-armed like every other first real payout.

## 4. Referral revshare — point a business to us, earn a cut

This is the second key ask. **Referring a user / agent / business to the gateway earns the
referrer an ongoing revshare cut of that referee's inference spend**, reusing the RL-1
referral ledger.

### 4a. Reuse RL-1 — attribution → eligibility → dispatch

RL-1 (the referral ledger) already models the three stages we need; the inference-credits
context just supplies the events:

- **Attribution.** A referral binds a referee (a funded gateway account: a person, an agent,
  or a **business org key**) to a referrer at signup/first-fund. Same attribution mechanism
  RL-1 uses for sites/agent referrals — the new surface is the gateway account, not a Sites
  page. "Point a business to us → you're attributed to that org's account."
- **Eligibility.** Each metered inference request the referee pays for produces a spend
  event in the gateway's metering/credit ledger (the openagents.com Worker, gateway doc §6).
  A configured **referral % of that inference spend** accrues to the referrer's RL-1 balance.
  Ongoing, not one-time: the referrer earns on the referee's spend for the agreed window.
- **Dispatch.** Accrued referral revshare settles as a **Bitcoin payout** through RL-2/RL-3,
  same spine and same owner-armed first-payout gate as the serving-fabric cut.

### 4b. How referral attribution works in the inference-credits context

- The attributable unit is **paid inference spend by the referee**, read from the same
  metering events that decrement credits (receipt-first, so the referral cut is computed off
  the same accounting that bills the customer — no parallel, drift-prone counter).
- A referral cut and a serving-Pylon cut are **independent and can stack** on one request: if
  Business B (referred by Referrer R) sends a request that Pylon P serves, that one request
  can fan **R's referral %** *and* **P's serving %** out of the margin, both through RL-1/2/3.
  The gateway's per-request economics (gateway doc §5) must reserve margin for both before
  computing house take.
- B2B is the highest-value referral target (gateway doc §2: the `~$5K/$60K` client pipeline).
  "Point a business at us → earn a % of their inference spend" is a concrete, ongoing,
  Bitcoin-settled deal that rides plumbing already built.

## 5. The Psionic boundary (respect it)

The roadmap is explicit (Executive Summary; "Implementation Boundary"): **pricing, payout,
marketplace, and identity authority stay OUTSIDE Psionic. Psionic emits evidence/receipts
those outer systems consume.** This doc honors that. Mapping:

| Piece | Owner | Notes |
| --- | --- | --- |
| Contiguous layer-block split, no whole-model residency | **Psionic** (`psionic-cluster`, `psionic-runtime`) | The sharded execution substrate |
| Authenticated activation transport, edge health, direct-return | **Psionic** (`psionic-net`) | Typed Rust activation frames |
| Per-stage execution, KV lifecycle, exact-greedy parity | **Psionic** (`psionic-runtime`/`psionic-models`/backends) | Born-verified correctness |
| Topology measurement + route selection | **Psionic** (`psionic-cluster`) | Measured RTT facts → route |
| Sharded-run + per-stage + topology **receipts** | **Psionic** emits | The dereferenceable proof |
| Coordinator serving + OpenAI-compatible mapping | **Psionic** (`psionic-serve`) | Request execution, response provenance |
| **Pricing / per-model multipliers / credit decrement** | **Product** (openagents.com Worker) | `2026-06-19-pricing-vs-factory.md` |
| **Metering** (usage → credit ledger) | **Product** (openagents.com Worker) | Receipt-first |
| **Per-stage payout split + Bitcoin settlement** | **Product** (RL-2/RL-3) | Consumes Psionic receipts |
| **Referral attribution + payout** | **Product** (RL-1) | Reuses referral ledger |
| **Routing to the fabric** (which supply lane) | **Product** (gateway) | Fabric is one supply adapter |
| **Marketplace / who-may-serve identity** | **Product** | Psionic exposes capability/attestation evidence only |

The seam: the gateway asks the fabric for a **plan + offered posture**; Psionic executes and
emits a **receipt**; the product layer **meters, prices, splits, and settles** off that
receipt. Psionic never holds money authority; the product layer never reaches into execution.

## 6. Phasing — product hooks per Psionic phase

Mirrors the Psionic roadmap's phases at a high level, calling out the **product-layer hook**
each phase needs. Psionic phase statuses are all `planned` for the sharded path today;
whole-small-model serving is the near-term product start that does not depend on the harder
phases.

| Stage | Psionic phase(s) | Product-layer hook needed |
| --- | --- | --- |
| **Start: whole-small-model on one Pylon** | (single-node serving; not the multi-stage roadmap) | Fabric supply adapter in the gateway; meter a whole-model Pylon request; **single-node contributor payout** (RL-2) against a basic serve receipt. Big models stay on Vertex/passthrough. |
| **Trusted local/N-stage split** | Phases 1, 3 | Parse the sharded-run receipt; **per-stage payout split** logic (read layer ranges → apportion). No real WAN yet; correctness/plumbing only. |
| **WAN topology + plain WAN serving** | Phases 4, 5 | Route large-model requests to the fabric when a verifiable WAN plan exists; record LAN/tailnet/public-WAN posture in the customer-facing provenance; honor exact-parity gate before payout. |
| **Speculative / direct-return / async** | Phases 6, 7, 8 | Receipt-mode awareness (plain vs spec vs direct-return vs async) so throughput/cost and the per-stage split stay correct across modes; no payout change in semantics, just richer evidence. |
| **Backend fast-verify** | Phase 9 | Capability-gated routing: only route to fabric backends that declare verified fast-verify; price accordingly. |
| **Quantized large-model stage execution** | Phase 10 | Route large open-weight models (gpt-oss / GLM-class) to the fabric **only** with a hardware-backed receipt; until then, typed refusal → fall back to passthrough. |
| **Self-managing trusted swarm + permissionless envelope** | Phases 11, 12 | Marketplace/identity: who may serve (trusted vs permissionless posture), capability/attestation evidence consumed by the product layer; referral + serving payout extend to permissionless serving Pylons once trusted posture is stable. |
| **Privacy + hardening** | Phase 13 | Surface the activation-visible privacy posture to the customer at request time; refuse untrusted activation-visible routing by default. |

**Build order, honest:** ship the **fabric supply adapter + whole-small-model serving +
single-node contributor payout** first (low-risk, depends only on single-node Pylon serving
and the already-wired RL spine), keep **big models on Vertex quota + partner passthrough**,
and **add shard-WAN large-model serving as Psionic's phases land** with real hardware
receipts. Referral revshare (RL-1) can ship independent of the fabric — it attaches to *any*
gateway spend regardless of which supply lane served it.

## 7. Honest gaps

- **Large-model shard serving is Psionic-planned / partly hardware-blocked.** Per the
  roadmap, the full shard-WAN WAN-pipeline path is status `planned`; only the planning and
  evidence substrate is `implemented_early`. Large-model claims (gpt-oss-120B MXFP4,
  GLM-5.2 NVFP4) **stay blocked until the backend stage executor, quantized layer-block
  loader, and receipts have real hardware evidence**. No large-model fabric claim ships
  without a hardware-backed receipt or a typed refusal. The near-term real lane is
  whole-small-model serving.
- **Activation-visible privacy posture.** Sealed/encrypted transport protects activations
  *in transit*; it does **not** hide them from the Pylon that must compute on them. A
  middle-stage Pylon can see the activations it processes. The honest public language is:
  encrypted transport, trusted-worker routing, activation-visible-worker disclosure, **no
  trustless privacy guarantee** (roadmap §7, Phase 13). Customers on the fabric lane must be
  told the posture; untrusted activation-visible routing refuses by default.
- **Trust — who may serve.** Trustless verification of an arbitrary worker's internal
  computation is **unsolved** (roadmap Risk Register; Phase 12 keeps it `planned`/`research`).
  Early fabric serving is **trusted-posture** (known/admitted Pylons). Permissionless serving
  extends only after trusted posture is stable, and even then receipts prove same-engine
  reproducibility + node evidence, **not** proof of every internal FLOP. The marketplace
  identity authority for "who may serve" is a product-layer surface, not Psionic.
- **Exact-parity is the non-negotiable payment gate.** Revshare clears against a checkable
  outcome. A run with no exact-greedy parity / no feasible same-engine reference does not pay
  against a strong proof by default; payout policy must decide explicitly and conservatively
  rather than paying against self-report. Speculative/async modes must remain
  token-identical to target greedy decode or they do not clear.
- **First real payout is owner-armed.** Both the serving-fabric cut and the referral cut ride
  RL-1/2/3, whose **first real dispatched payout is owner-armed** (JUNE19_ROADMAP). The
  plumbing can be built and tested, but the first real serving/referral Bitcoin payout needs
  the owner to arm the gate.
- **Split fairness is an open product decision.** The per-stage payout weighting (layer-count
  vs FLOP-aware vs latency-contribution) is unresolved (§3c). The Psionic receipt carries
  enough to compute any of them; the product layer must pick and publish the rule so the
  split is legible and reproducible to serving contributors.

## 8. Initial next steps (product-layer, doc-only here)

1. **Fabric supply adapter** in the gateway — ask-plan / serve / consume-receipt interface to
   Psionic; start with whole-small-model single-Pylon serving.
2. **Per-stage payout split** — parse `psionic.serve.pipeline_sharded_run_receipt.v1`,
   apportion the contributor cut by layer-block (publish the chosen weighting rule), settle
   via RL-2/RL-3.
3. **Referral attribution for gateway accounts** — bind referee (incl. business org keys) to
   referrer via RL-1; accrue a referral % off receipt-first metered inference spend; dispatch
   via RL-2/RL-3.
4. **Routing policy** — when to prefer the fabric (cheap tier near-term; large open-weight as
   Psionic lands) vs Vertex/passthrough; capability-gated and parity-gated.
5. **Privacy posture surfacing** — show the activation-visible posture to customers on the
   fabric lane; default-refuse untrusted activation-visible routing.

> Build order is gated by Psionic's phase delivery (large-model serving) and by the
> owner-armed first-payout gate. The whole-small-model lane + the RL-1 referral attachment
> can proceed now against single-node Pylon serving and the already-wired revenue-loop spine.
