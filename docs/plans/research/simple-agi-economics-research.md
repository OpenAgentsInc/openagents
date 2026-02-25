Below is a deep read of **“Some Simple Economics of AGI” (Catalini, Hui, Wu; Feb 24, 2026)** and a concrete mapping to OpenAgents—what we already have that *matches the paper’s prescriptions*, what we’re missing, and what systems we’d need to add to fully implement the paper’s “verification-first” economy.

---

## 1) The paper’s core claim: execution is collapsing, verification is the bottleneck

The paper argues we’re mis-modeling the AGI transition by treating “cheap machine output” as automatically becoming “realized value.” Instead, as agents become capable of broad action, the limiting factor becomes **human verification bandwidth**—the scarce capacity to validate outcomes, audit behavior, and underwrite meaning/responsibility once execution is abundant.

They formalize this as two racing curves:

* **Cost to Automate** (c_A): driven down by compute and accumulated knowledge.
* **Cost to Verify** (c_H): constrained by human time, expertise, and feedback latency.

This creates a widening **Measurability Gap** (\Delta m): what agents can do vs what humans can afford to verify. The productive part of agentic labor is the **verifiable share** (s_v). Everything beyond that becomes a “Trojan Horse” risk externality: unverified output that looks good on metrics but violates intent and accumulates hidden debt.

This is basically the economic framing of “reward hacking + scalable oversight failure”: once agents can optimize measurable proxies faster than we can check reality, the economy can drift into a **Hollow Economy** (high nominal output, decaying true utility).

---

## 2) The regime map: four zones and why the “human-in-the-loop” is unstable

The paper’s regime geometry (see the diagram around page 44) partitions tasks by:

* **x-axis:** cost to automate (falls with compute scale)
* **y-axis:** cost to verify (rises with feedback latency and scarcity of expertise)

Result: four zones:

1. **Safe Industrial Zone**: cheap to automate, affordable to verify
2. **Runaway Risk Zone**: cheap to automate, *not* affordable to verify
3. **Human Manual / Artisan Zone**: hard to automate, verifiable
4. **Pure Tacit Zone**: hard to automate and hard to verify

Key point: scaling compute shifts tasks left (more automatable), while erosion of human expertise shifts tasks up (harder to verify). This makes the “human-in-the-loop” equilibrium **dynamically unstable**: the same forces that expand agentic capability also cannibalize the human verification capacity needed to keep it safe.

---

## 3) The three failure engines: Missing Junior Loop, Codifier’s Curse, Alignment Drift

### A) Missing Junior Loop (collapse of training pipeline)

They model human experience (S_{nm}) (the stock of tacit verification skill) as something that only grows via doing and practice. If automation drives routine measurable work (T_m) toward zero, then the apprenticeship pipeline collapses unless we substitute **synthetic practice** (T_{sim}) at scale.

**Translation:** if we don’t intentionally build “flight simulators for verification,” society loses the ability to produce verifiers right when verifiers become the scarce complement.

### B) Codifier’s Curse (experts generate their own replacements)

Every time experts do high-value verification (T_{nm}), their corrections/logs become training data (public knowledge (A) or proprietary knowledge (K_{IP})), which accelerates automation and shrinks their moat. This produces an unavoidable “tacit extraction” dynamic.

**Translation:** the act of supervising agents creates the data to automate supervision.

### C) Alignment Drift (alignment is maintenance, not a one-shot spec)

They treat alignment (\tau) as a stock variable that decays when (\Delta m) is positive, unless we keep allocating real verification effort (T_{nm}) and reduce drift sensitivity (\eta) (graceful degradation, robustness).

---

## 4) The Trojan Horse externality: why markets over-deploy agents

They formalize the “resource leak”:

[
X_A = (1-\tau)(1-s_v)L_a
]

Where (L_a) is deployed agentic labor, (s_v) is the verifiable share, and (1-\tau) is misalignment exposure. This is “counterfeit utility”: agents consume real resources to generate outputs that satisfy metrics but violate intent, and the harm may only appear after long feedback latency.

The killer economic claim is: **unverified deployment becomes privately rational**, especially when liability is weak or diffuse. So the default equilibrium is too much deployment, too little verification—unless we force risk internalization (liability/insurance), and build verification as a production technology.

---

## 5) The “AI verifies AI” trap: correlated blind spots

They explicitly warn that substituting AI for verification *appears* to shrink verification cost, but creates a correlation penalty ( \kappa_{corr} \gg 1 ): checker and doer share blind spots, producing false confidence.

This is extremely relevant to OpenAgents: “just run another model to review it” is not a safe general strategy unless the verification is **independent** (different model families, different data, objective checks, adversarial testing).

---

# 6) How this maps onto OpenAgents (what we already have)

OpenAgents’ architecture already aligns with the paper’s thesis in several deep ways:

## A) “Verification as production” → Verified Patch Bundles + pay-after-verify

Your core wedge (Autopilot → verified PRs with replay artifacts) is exactly the paper’s Safe Industrial Zone: tasks where verification is cheap relative to generation. The Verified Patch Bundle pipeline, deterministic sandbox runs, and receipts are the mechanism for expanding (s_v).

## B) Budgets + approvals → binding La to verifiable throughput

The paper says: scale must be gated by verifiable share, otherwise (X_A) explodes. OpenAgents budgets and approvals are the scaffolding for “no unbounded autonomy.” They’re the beginnings of the risk budget constraint.

## C) Objective vs subjective job taxonomy → measurability boundary

Your marketplace taxonomy (objective jobs like `oa.sandbox_run.v1` vs subjective jobs with best-of-N + adjudication + human sampling) is almost a direct implementation of the paper’s measurable vs non-measurable split.

## D) Receipts + cryptographic provenance → lowering cH via provenance

The paper argues cryptographic provenance expands easy verification by reducing verification costs of process/provenance. OpenAgents’ receipt doctrine (canonical hashes, replay logs, policy bundle ids) is a direct “provenance premium” primitive.

## E) Hydra (LLP + CEP) → scaling autonomy without top-up bottlenecks

Hydra’s credit envelopes are exactly a mechanism to prevent the “top-up bottleneck” while keeping spending bounded. It also sets you up for the paper’s “liability-as-a-service” direction because envelopes, fees, receipts, and circuit breakers are the raw materials of underwriting.

---

# 7) What we’re missing to “fully implement the paper” (systems to add)

The big gap is: we have **verification mechanics**, but we don’t yet have a full **verification economy** and **underwriting stack**. The paper’s prescription goes beyond “add receipts”—it’s “turn verification + liability into the moat and the product boundary.”

Below are the key systems we should add.

---

## 7.1 A first-class “Verification Plane” (measure sv, Δm, XA continuously)

### What it is

A dedicated subsystem that computes, per run/job/market lane:

* **sv (verifiable share)**: what fraction of agent work is genuinely verified (objective checks, human signoff, independent audits)
* **Δm pressure**: proxy for how much activity is happening in unverifiable zones (subjective tasks, long feedback latency tasks, weak checks)
* **XA proxy**: “unverified throughput × misalignment exposure” in operational terms

### Why it matters

Right now we can *do verification*, but we don’t treat verification as the core production metric. The paper says “verified throughput” is the growth constraint. We need the dashboard and gating logic to match.

### Concrete OpenAgents implementation

* Add a **VerifiabilityClassifier** that tags every task/run with:

  * verification type: objective | human | independent-model | correlated-model | none
  * feedback latency class: ms | minutes | days | months | years
* Store **verifiability receipts** as first-class objects (separate from payment receipts).
* Expose `sv` and “unverified throughput” in `/stats` as top-tier metrics.

---

## 7.2 Jagged-Frontier Deployment Policy (bind La scaling to sv)

The paper’s operational rule is: you can’t scale deployment without scaling verifiability, otherwise the leak grows.

### What we should add

A policy engine that gates autonomy *dynamically* based on measured verification capacity:

* **Risk budget X** (max allowed “unverified work” per day/org)
* Enforced rule: if verifiability drops, autonomy must degrade (graceful degradation)

### Concrete mechanism

* “Autonomy throttle” that automatically shifts:

  * Fastest → Balanced → Cheapest → “Requires approval”
  * Moves subjective work into redundancy + human sampling tiers
  * Blocks long-horizon unverifiable actions unless bonded/insured (see below)

---

## 7.3 Independent Verification Layer (avoid AI-verifies-AI correlation)

The paper is explicit: “AI verifying AI” can create false confidence due to correlated errors.

### What we should add

A verification architecture that makes independence explicit:

* **Verifier diversity policy**: require different model families / different providers for checks
* **Adversarial verification**: specialized “red team” verifiers that look for proxy gaming
* **Objective-first**: aggressively convert workflows into objective checks (tests, invariants, proofs)

### OpenAgents-native implementation

* Extend your subjective tiering engine so “verification tier” is a required input:

  * Tier 0: correlated model check (cheap, low trust)
  * Tier 1: heterogeneous model check (medium)
  * Tier 2: best-of-N with heterogeneity + adjudication (higher)
  * Tier 3: human underwriter sampling (highest)
* Make “correlation risk” a metric in `/stats`.

---

## 7.4 Underwriting + Liability-as-a-Service (turn verification into the product boundary)

This is the paper’s biggest strategic punch: as execution commoditizes, firms are valued by the ability to **absorb tail risk**, price it, and warrant outcomes—“Liability-as-a-Service.”

### What we should add

A full underwriting system:

* **Warranties**: “this output is insured up to X”
* **Bonds / collateral**: require stake for high-risk actions (you already have this planned in your roadmap—make it a Hydra-native contract)
* **Claims + disputes**: standardized evidence bundles, replay artifacts, receipts
* **Reserves**: treasury partitions for underwriting
* **Pricing**: risk-priced fees based on verifiability + historical incident rate

### Where it plugs into OpenAgents

* Hydra CEP already has the right shape:

  * envelopes + fees + receipts + circuit breakers
* Extend Hydra with:

  * **bond issuance** (escrowed sats)
  * **claim workflow** (attach replay + verification evidence)
  * **settlement rules** (refund vs payout vs slash)
* Pair with the marketplace commerce grammar (`commerce.proto`) so warranty/claim becomes a native contract surface.

---

## 7.5 Verification-grade KIP (ground truth as moat)

The paper distinguishes “execution-grade proprietary data” vs “verification-grade” data: failure logs, near-misses, postmortems, adjudication history—data that lowers cH.

### What we should add

A **Ground Truth Registry**:

* incident taxonomy + storage
* near-miss capture (not only failures)
* link every incident to:

  * replay logs
  * receipts
  * affected systems and versions
  * resolution and later outcomes

### Why it matters

This becomes the compounding asset that expands sv over time (the “precedent library” effect the paper discusses for verification networks).

---

## 7.6 Synthetic Practice System (replace the missing junior loop)

The paper is blunt: apprenticeship collapses unless we create deliberate simulation ladders (Tsim) to rebuild Snm.

### What we should add

A first-class **Simulation / Synthetic Practice product** inside OpenAgents:

* “Flight simulator for code review / incident response / security”
* Agents generate realistic scenarios + hidden failure modes
* Humans practice verification and underwriting decisions
* Outcomes are scored and become verifiable career/role credentials

### OpenAgents-native implementation

* A new “Verifier Track” role:

  * tasks: review bundles, audit trails, dispute evidence
  * training: curated edge cases from your incident registry
  * scoring: measured ability to detect hidden issues (Snm proxy)
* This directly builds the stock of verifiers your economy needs.

---

## 7.7 “Verified Network Scale” metric (NV = ρN)

The paper warns networks will be flooded by synthetic activity and that durable moats depend on **verified participation** not raw volume.

### What we should add

For marketplaces and any “feed” surface:

* measure ρ (authenticated/verified share)
* expose NV as first-class KPI
* enforce anti-slop gates:

  * proof-of-work/receipts
  * identity attestations (not necessarily PoP, but “economic reality proofs”)

This aligns with your open /stats stance: publish these metrics publicly to prove health.

---

# 8) Putting it together: “OpenAgents is the Augmented Economy stack”

The paper’s “Augmented Economy” requires:

* Observability + provenance (lower cH)
* Verification-grade ground truth (Kver_IP)
* Human augmentation and simulation (raise Snm / effective Tnm)
* Graceful degradation (lower η)
* Liability/internalization (raise B via priced risk)

OpenAgents already has strong primitives for the first and last: **receipts, replay, policy/budgets, pay-after-verify, Hydra envelopes**.

To fully implement the paper, the missing center of gravity is:

> a verification plane + underwriting product layer + synthetic practice pipeline
> that makes **sv** the north star, and makes **warranted outcomes** the business boundary.

If you want, I can turn the “missing systems” into:

* a repo-ready ADR set (Verification Plane, Underwriting, Simulation Ladder), and
* an issue map that aligns to your OA-ECON gates and Hydra proto you just approved.
