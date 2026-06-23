# Khala on the Blueprint/DSPy Program System + Tassadar Plugin Extensibility

> Status: architectural direction note, 2026-06-23. This doc states a
> direction and labels current-vs-future explicitly. It is **not** a product
> promise, a served capability, or public-claim copy. Nothing here widens any
> promise registry entry, asserts a public plugin marketplace, or upgrades any
> launch claim. The starter plugin catalog's own boundary holds throughout:
> "the catalog does not imply public plugin publication, arbitrary external
> plugin admission, or a public plugin marketplace" (psionic
> `docs/TASSADAR_STARTER_PLUGIN_CATALOG.md`). The Tassadar disclosure flow and
> the product-promise registry govern anything that ever becomes a public
> claim. Speculative/future sections are marked **FUTURE**.

## The direction in one paragraph

Khala should not stay a model-alias router. Its inference should run as
**typed, optimizable Blueprint programs** — DSPy-style signatures, the GEPA
optimizer over their prompts/policies, and the learned coordinator as the
composition layer — so that quality and cost are improved by *optimization
against executed evals and acceptance receipts*, not by manual prompt edits.
And that Blueprint/program layer should be **extensible via independently
authored capability units** along the lines of the Tassadar plugin-marketplace
audit: capabilities discovered, composed into Khala programs, metered per use,
and (FUTURE) paid in Bitcoin with a revenue split to the component authors —
so Khala grows capabilities without core changes. This aligns with the
inference-engineering-book read that "Khala should be treated as an inference
platform and control plane, not just as a model alias"
([`inference-engineering-book/README.md`](inference-engineering-book/README.md)).

## 1. Khala on Blueprint/DSPy: inference as typed, optimizable programs

### The concrete "Blueprint/DSPy system" in this repo

The workspace-level **Blueprint** repo is deprecated as a standalone service
(sunset 2026-05-24), but the *concept* — typed Programs, stable Signatures,
swappable Module Versions, immutable decision-evidence Program Runs, Optimizer
Runs, Source Authority, evidence, and receipts — is being rebuilt natively in
the product surface and is already partially present. The pieces, by real
surface:

- **The typed signature layer (DSPy inheritance).**
  `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts` is the
  Blueprint Signature Lookup Service: it matches a *structured typed request*
  (program-signature IDs, program-type IDs, allowed surfaces, risk ceilings,
  backend kinds) against a Blueprint registry projection and returns a
  validated selection carrying tool scopes, evidence refs, receipt refs, and
  release-gate refs. This is a typed program-call mechanism — the central
  semantic selector the workspace's no-keyword-routing rule wants — not a
  string match.
- **Typed contributions / program signatures.**
  `packages/probe/packages/runtime/src/blueprint/contribution.ts` defines
  Blueprint contribution kinds (`signature_contribution`, `program_signature`)
  and the StudyBench authoring/judging kinds, mapping each to a capability
  family and carrying no runtime authority by default.
- **The evidence-only write boundary.**
  `apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`
  is the Action Submission repository: proposals reference evidence refs
  (including StudyBench closeout refs) and carry evidence-only authority — no
  direct execution, mutation, or payout flags. This preserves the historical
  Blueprint invariant that "Program Runs are decision evidence; they do not
  authorize writes."
- **The optimizer (GEPA).** GEPA-class reflective prompt/program optimization
  is the offline improvement loop. The bounded scheduled runner contract is
  [`../artanis/2026-06-08-bounded-gepa-scheduled-runner.md`](../artanis/2026-06-08-bounded-gepa-scheduled-runner.md)
  (status-projection only, explicitly denying training/promotion/settlement
  authority), and the StudyBench MVP feeds Psionic GEPA candidate feedback via
  `psionic.probe_gepa_candidate_manifest.v1` from rubric-score artifacts and
  claim-level failures
  ([`../research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md`](../research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md)).
  (Historical naming note: earlier drafts referred to a `probe.li_candidate.v1`
  / `psionic.probe_li_candidate_manifest.v1` lane; the surface that exists
  today is the GEPA candidate manifest above.)
- **The learned coordinator (Khala's composition brain).** TRINITY (a tiny
  head over a frozen LM's hidden state, trained by separable CMA-ES against a
  terminal verification reward) and the Conductor (a 7B GRPO-trained NL planner
  emitting workflows over a worker pool) — see
  [`../sakana/psionic-coordinator-roadmap.md`](../sakana/psionic-coordinator-roadmap.md)
  (primitives P1–P5),
  [`../sakana/conductor-2512.04388v5.md`](../sakana/conductor-2512.04388v5.md),
  and the M6/M7 lanes in
  [`khala-buildout-roadmap.md`](khala-buildout-roadmap.md). Psionic already
  has the verify/govern half (candidate/shadow contract, module-eval receipts,
  promotion gates); the coordinator rides that machinery unchanged.

### Why these are one system, not three

A DSPy program is: a **signature** (typed I/O contract), a swappable
**module/implementation** behind it, an **optimizer** that improves the module
against a metric, and **evidence/receipts** that ground promotion. Khala's
surfaces map cleanly:

| DSPy / Blueprint concept | Khala surface (this repo) |
|---|---|
| Signature (typed I/O contract) | `signature-lookup.ts` selection; `program_signature` contributions |
| Module version (swappable impl) | a route/role choice + prompt/policy the coordinator selects; Psionic candidate artifacts |
| Optimizer (improve against a metric) | GEPA candidate feedback (`psionic.probe_gepa_candidate_manifest.v1`); TRINITY sep-CMA-ES; Conductor GRPO |
| Metric / terminal reward | the **executed verifier verdict** + acceptance receipt (e.g. the khala-code crossy-road rubric, `test_passed`) |
| Decision evidence (no writes) | Program Run / Action Submission evidence-only authority |
| Promotion gate | Psionic promoted/candidate + shadow governance; cost-per-accepted-outcome |

The thesis: a Khala request is (FUTURE, fully) a **typed Blueprint program
call** — a signature selects a composed module (plan/write/verify), GEPA tunes
its prompts/policies offline, the learned coordinator picks the composition,
and the terminal reward is the *executed* acceptance receipt
([`2026-06-22-verified-work-must-execute-the-artifact.md`](2026-06-22-verified-work-must-execute-the-artifact.md)).
Quality and cost then improve by **optimization against executed evals + paid
acceptance**, which is exactly the lever the inference-engineering-book notes
say matters most for Khala
([`inference-engineering-book/khala-investigation-notes.md`](inference-engineering-book/khala-investigation-notes.md)):
the learned coordinator needs reward inputs that reflect accepted outcome per
sat and per second. The **M7 Conductor** is the composition layer where this
becomes "compose to win the benchmark" rather than "route to one model."

## 2. Blueprint extensibility via the Tassadar plugin marketplace

Read [`../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)
in full for the lineage; the short version is that OpenAgents has built "a
marketplace where independently authored units of machine capability are
listed, discovered, composed into agents, metered per use, and paid for in
Bitcoin, with revenue flowing automatically to everyone whose component did
work" twice (the 2024 agent store; Blueprint signatures) and is building the
verifiable third generation (Tassadar digest-pinned modules whose execution is
its own receipt).

The direction for Khala: the Blueprint/program layer is the natural **host**
for that extensibility. Independently authored capability units —
starter-plugin-class deterministic units today; (FUTURE) Tier-E
conformance-tested compiled modules — become **module versions behind Khala
program signatures**. They are:

- **Discovered** semantically (typed selectors / embeddings per the
  no-keyword-routing rule), via the signature lookup, never by string match.
- **Composed** into Khala programs by the coordinator: a planner routes across
  conformance-tested, digest-pinned modules behind explicit ABI tokens (the
  audit's "modules as organs," and psionic's `tassadar_module_linker.rs`
  preparation).
- **Metered per use** through the existing Khala metering/receipt path
  (`metering-hook.ts`, the `openagents` receipt block).
- **(FUTURE) Paid in Bitcoin with a revenue split** to component authors —
  the audit's "the trace itself decomposes who computed what," reviving the
  2024 store's 60/20/20 split *grounded on evidence* rather than asserted by
  bookkeeping. This is gated on a real settlement loop and owner arming; today
  payment work is Bitcoin/Spark-only and deliberately bounded
  ([`khala-buildout-roadmap.md`](khala-buildout-roadmap.md)).

This is how Khala grows capabilities **without core changes**: a new capability
is a new admitted module + signature, not a code change to the gateway. The
admission discipline is inherited intact — the starter catalog's
`StarterPluginRegistration` (typed packet schemas, refusal sets, capability
class, origin class, negative claims) and the W2 quarantine-before-admission
posture become the program layer's extension contract.

### The boundary (in force)

Per the audit and the starter catalog, this is **not** a public plugin
marketplace, **not** arbitrary external plugin admission, and **not** a promise
of public plugin publication. The store is "the last thing built this time, not
the first": the sequencing is window → factory → conformance-tested module
library behind ABI tokens → *only then* listing/discovery/settlement surfaces.
Any proposal to build marketplace/listing UI ahead of a conformance-tested
module library is the 2024 mistake attempting a comeback. Tier-S (learned)
goods may never borrow Tassadar's exactness vocabulary; Tier-N (effectful)
goods inherit the full Blueprint governance (Source Authority, Action
Submission, approval, receipts).

## 3. Current vs Future (honest split)

| Capability | Today (exists) | FUTURE (direction; gated) |
|---|---|---|
| Typed signature layer | `signature-lookup.ts` selection, `program_signature` / `signature_contribution` kinds, evidence-only Action Submissions | Khala request fully expressed as a typed Blueprint program call (signature → composed module → executed reward) |
| Optimizer | bounded GEPA status-projection runner (no train/promote authority); StudyBench → `psionic.probe_gepa_candidate_manifest.v1` candidate feedback | GEPA-optimized Khala prompts/policies promoted on cost-per-accepted-outcome, behind release gates |
| Learned coordinator | Psionic verify/govern half (candidate/shadow contract, module-eval receipts, promotion gates); M6/M7 scaffold + primitives P1–P5 roadmap | TRINITY router shadow→promoted; M7 Conductor composing plan/write/verify to win the head-to-head |
| Capability units / plugins | starter plugin catalog: 6 cataloged deterministic plugins, typed `StarterPluginRegistration`, no-marketplace boundary | conformance-tested Tier-E compiled modules behind ABI tokens, composed into Khala programs |
| Discovery | semantic signature lookup (typed selectors) | open-protocol listing/discovery adapter (SKL / NIP-DS class), owner-gated, post-module-library |
| Metering | `metering-hook.ts`, `openagents` receipt block, Bitcoin/Spark payout path (bounded test) | per-trace revenue *decomposition* → automatic split to component authors |
| Public marketplace | **none** — boundary explicitly closed | **none until** a workstream produces evidence that needs one + disclosure-flow + owner sign-off |

## Pointers

- [`khala-buildout-roadmap.md`](khala-buildout-roadmap.md) — the sequenced
  M0–M8 buildout; M6 (learned coordinator) and M7 (Conductor) are the
  composition layer this direction depends on.
- [`inference-engineering-book/khala-investigation-notes.md`](inference-engineering-book/khala-investigation-notes.md)
  and [`inference-engineering-book/README.md`](inference-engineering-book/README.md)
  — "Khala is an inference platform + control plane"; reward inputs must
  reflect accepted outcome per sat and per second.
- [`../sakana/psionic-coordinator-roadmap.md`](../sakana/psionic-coordinator-roadmap.md),
  [`../sakana/conductor-2512.04388v5.md`](../sakana/conductor-2512.04388v5.md),
  [`../sakana/coordinator-as-verified-work.md`](../sakana/coordinator-as-verified-work.md)
  — the learned coordinator / Conductor.
- [`../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)
  — the three-generation marketplace lineage and its no-marketplace boundary.
- [`../artanis/2026-06-08-bounded-gepa-scheduled-runner.md`](../artanis/2026-06-08-bounded-gepa-scheduled-runner.md)
  — the bounded GEPA runner contract and its denied authorities.
- [`../research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md`](../research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md)
  — StudyBench → GEPA candidate feedback.
- Code surfaces:
  `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`,
  `packages/probe/packages/runtime/src/blueprint/contribution.ts`,
  `apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`,
  psionic `docs/TASSADAR_STARTER_PLUGIN_CATALOG.md`.
- **Not edited here:** `khala.md` (an active telemetry-scorecard lane owns it).
