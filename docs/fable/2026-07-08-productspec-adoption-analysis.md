# ProductSpec adoption analysis — the intent layer for OpenAgents planning and business agents

Date: 2026-07-08
Status: analysis + recommended adoption design (owner review requested on §8 gates)
Reference: `projects/repos/ProductSpec` (github.com/gokulrajaram/ProductSpec, MIT, repo v0.7.0, format `spec_format_version: "0.1"`)
Audience: owner + fleet agents + business agents (Sarah, BF engine, future AI employees)

## 0. Verdict up front

Adopt the **format**, not the tooling. ProductSpec is a small, well-designed,
MIT-licensed open standard for exactly the artifact we currently do not have a
uniform shape for: **committed product intent before implementation** — the
what/why layer that sits above MASTER_ROADMAP sequencing, GitHub epics, behavior
contracts, Eval Suites, and the promise registry. Its two central distinctions
(acceptance criteria = pre-launch **build contract**; success metrics =
post-launch **market contract**) are distinctions we already enforce culturally
(Eval Suite gates vs. receipted promise flips) but have never had a portable,
validatable document shape for.

Recommended posture:

- **Adopt** `.product-spec.md` as the standard intent artifact for consequential
  work across the openagents monorepo, sub-project repos, and business-agent
  output.
- **Implement our own validator** on Bun/Effect Schema (L1 compliance + the
  structured blocks), using the upstream conformance fixtures as the
  compatibility oracle. Do not take the npm parser as a production dependency.
- **Extend via the standard's own extension points** (`custom-*` sections,
  `tool_metadata`) for the OpenAgents-specific spine: owner gates, receipts,
  promise links, behavior-contract links, credit budgets. Never fork the core
  vocabulary.
- **Map Decision Trace onto our receipts discipline** as a portable projection,
  not a new source of truth.
- Wire "no consequential epic without a Product Spec" into agent contracts and
  the QA gate — with an explicit smallness threshold so it never violates
  constant motion.

## 1. What ProductSpec is (30-second version)

A Product Spec is a Markdown file (`<name>.product-spec.md`) with YAML
frontmatter and a canonical section vocabulary. Five mandatory sections, in
order:

1. `problem` — who is hurting and why it matters
2. `hypothesis` — the causal bet ("if X ships, behavior Y changes because Z")
3. `scope` — in / out / deliberately cut (optional structured
   `productspec-scope` fenced block)
4. `acceptance_criteria` — pass/fail pre-launch build gates; AI eval thresholds
   live here in structured `productspec-ai-evals` blocks (id, type, input_set,
   evaluator, pass_threshold, checks)
5. `success_metrics` — post-launch real-user behavior in structured
   `productspec-success-metrics` blocks (id, metric, target, window, segment,
   source)

Optional sections: `user_experience`, `customer_truth`,
`solution_alternatives`, `solution`, `strategic_positioning`, `adoption`,
`pricing`, `risks`, `ai`, `open_questions`, `rollout`. Custom sections use
`custom-<kebab-name>` and must round-trip. Frontmatter carries
`spec_format_version` (the standard's version, currently `"0.1"`),
`artifact_type` (`hypothesis` | `prd` | `openspec_proposal`), authorship,
timestamps, and an optional `spec_revision` integer that increments when
**intent** materially changes (Git keeps the diff; the revision number is the
portable citation handle: "implements spec X at `spec_revision: 3`").

Companion pieces:

- **Validator/parser** (`@productspec/parser`, TypeScript reference, ~600 LOC)
  with typed error codes (`missing_required_section`, `invalid_ai_eval`,
  `invalid_success_metric`, ordering, custom-section shape…) and warnings for
  thin/empty sections. JSON Schemas mirror the parse output. A
  `conformance/` fixture corpus (valid + invalid) defines compatibility.
- **Review annotations** (L2): portable per-section review verdicts
  (`pass`/`fail`/`warn` per axis) — a serialization for reviewer output.
- **Decision Trace** (optional companion, separate JSON file): typed events for
  how intent changed after commitment — `scope_drift`,
  `acceptance_criteria_drift`, `ai_eval_drift`, `implementation_tradeoff`,
  `spec_revision`, `outcome_review` — each with observed drift
  (spec claim vs. observed reality) and the explicit decision
  (update spec / update implementation / accept tradeoff / reopen / learning).

The standard is deliberately scoped: **structure and portability, not taste.**
It defines what a spec must contain to be parseable and comparable, and leaves
"is this a good bet" to the org's own review layer. Design principle quoted
from the README: "structure the parts machines must execute or compare; leave
the parts humans must reason about readable."

## 2. Why this fits us unusually well

We already run most of the ProductSpec worldview — we just run it without a
uniform artifact. The mapping is almost embarrassing:

| ProductSpec concept | Existing OpenAgents mechanism | Gap the format closes |
|---|---|---|
| Acceptance criteria = build contract | Eval Suites, expected-* fixtures as oracles, QAM release gate, feature-ladder rungs | Today the build contract is scattered across issue checklists, lane specs, and test files. No single citable pre-launch gate list per piece of work. |
| `productspec-ai-evals` (id, input_set, evaluator, pass_threshold, checks) | Blueprint-modeled Eval Suites; fixture-first authoring under QAM-7 | Nearly 1:1. Our Eval Suites gain a portable declaration that lives with intent instead of only in the test tree. |
| Success metrics = market contract (id, target, window, segment, source) | Product-promise registry, receipted greens, exact-only counters, Aiur rollups | A promise flip is our "success metric passed." The spec gives the *pre-launch declaration* of what will be measured, before the registry carries the claim. |
| `spec_revision` + living document | Owner decisions recorded on issues; roadmap rev numbers (MASTER_ROADMAP rev 6.4 works exactly like this) | We already version intent informally (rev 2, rev 6.3…). The format makes the handle machine-readable and per-decision instead of per-roadmap. |
| Decision Trace (drift → explicit decision) | Receipts discipline; "do not weaken an oracle to make a change pass — that is a contract change and needs owner sign-off"; NEEDS_OWNER.md; NO-GO/AAR docs | Same law, different serialization. Our receipts are richer; theirs are portable. |
| Scope in/out/**cut** | "Explicit non-moves" sections in migration plans; deliberate-cut notes in lane specs | The `cut` list is the single best small idea in the standard — deliberate cuts today get buried in prose and rediscovered as scope drift. |
| Review annotations (L2) | Judge panels, adversarial verify, code-review swarms, QAM strict issues | Our multi-agent reviewers currently emit ad-hoc prose. A portable per-section verdict shape is a natural output contract for review agents. |
| "Intent should survive handoff" | The whole fleet model: owner → Fable → subagent waves → Codex/Claude/Grok workers | The more handoffs, the more the spec matters. We have more handoffs per unit of work than any normal team — humans hand to agents, agents hand to agents, harnesses hand to harnesses (MH lanes). |
| "No epic without a Product Spec" | "Behavior contracts land in the same change"; "route claims through docs/promises before broadening copy" | Same enforcement pattern we already use — a registry + a gate — applied one layer earlier. |

Two deeper alignments worth naming:

**It matches the Blueprint-lite direction.** P4's company brain adopts typed
objects with per-fact provenance and Action-Submission writes. A Product Spec
is precisely a typed object for the "committed intent" fact class, with
`spec_revision` as its version chain and Decision Trace as its provenance for
changes. When CB-1 lands, `.product-spec.md` files are an ingestion-ready
corpus (trust-cost order: they are owner-committed, Git-versioned, structured).

**It gives business agents a work-product format.** Sarah qualifies a prospect
and produces… what, exactly? Today: CRM rows, a conversation transcript, a
checkout link. The missing artifact between "qualified conversation" and
"fleet does the work" is a **hypothesis-type Product Spec**: problem (the
prospect's own words → `customer_truth`), hypothesis, scope with explicit
cuts, acceptance criteria the fleet builds against, success metrics the
engagement is judged by. The same shape then serves AE-3 templates (a
template's verification rubric *is* acceptance criteria; its receipted
external outcome *is* a success metric reading) and BF engagements.

## 3. Where it does NOT fit / honest risks

- **Pre-1.0, single-author standard.** Format v0.1, repo v0.7.0, essentially
  one maintainer, no visible ecosystem yet. Mitigation: we implement the
  format ourselves against their conformance fixtures; if upstream dies or
  turns, we own our validator and our files remain plain Markdown. Worst case
  we are left with a good in-house convention. Low downside.
- **It is not a replacement for any of our enforcement layers.** A spec's
  `acceptance_criteria` section is *declared intent*; our behavior-contract
  registry and Eval Suites are *enforcement*. The spec must link to contract
  IDs and suite names — never duplicate their content and drift. Same for
  success metrics vs. the promise registry: the registry stays the only
  authority for public claims; the spec declares, the registry proves.
- **Ceremony risk vs. constant motion.** ProductSpec's own docs are clear it is
  for *consequential* work. If we require a spec for every lane, agents will
  generate boilerplate specs that nobody reads — negative value. The threshold
  needs to be explicit (see §5.1).
- **`author`/`approved_by` fields vs. our metadata rule.** We do not put
  individual names in committed metadata. Agent-authored specs use agent/role
  identity ("OpenAgents fleet", "Sarah", "owner"); Decision Trace
  `approved_by` uses roles ("owner", "product lead").
- **`tool_metadata` leakage.** The standard says exports for public sharing
  strip `tool_metadata` by default — which is where our epic refs, budgets,
  and gate states will live. Our public-safe projection law applies: anything
  in a spec destined for a public repo is public the moment it lands; private
  engagement specs (customer work, pricing) live in the private repos
  (`alpha`, `sarah`, customer workspaces), same routing law as everything
  else.
- **`openspec_proposal` artifact type**: we don't use OpenSpec; ignore that
  value. Our engineering-spec layer downstream of a Product Spec is what it
  already is — lane specs, INVARIANTS updates, issue work packages. We do not
  need to adopt OpenSpec/Spec Kit to use ProductSpec upstream (their own docs
  are explicit that the layers are independent).

## 4. Adoption design

### 4.1 Where spec files live

- **openagents monorepo:** `specs/<area>/<name>.product-spec.md`
  (e.g. `specs/khala-code/mobile-codex-connect.product-spec.md`). A top-level
  `specs/` keeps them out of `docs/` (which is analysis/receipts) — a spec is
  a *control file*, not documentation.
- **Sub-project repos** (`effect-native`, `tap-ldk`, `psionic`, `sarah`,
  `cloud`-successor crates, etc.): `specs/` at each repo root, same convention.
  The owning repo holds the spec for work that lands in it.
- **Cross-repo programs** (the kind MASTER_ROADMAP phases describe): the spec
  lives in the repo that owns the *product surface*, and other repos link to it.
  MASTER_ROADMAP itself remains the sequencing authority — it references specs;
  specs never re-state sequencing ("when sequencing disagrees, the roadmap
  wins" already covers this).
- **Business engagements:** private by default —
  `sarah` repo or the CRM-linked engagement workspace, exported public-safe
  only through the existing projection gates.

### 4.2 Frontmatter and section policy

- `artifact_type`: `hypothesis` for bets/experiments/business-agent proposals;
  `prd` for committed product lanes. (Skip `openspec_proposal`.)
- `spec_revision` mandatory for us from revision 1 (the standard makes it
  optional; our citation discipline wants it always).
- `linked_github_repo` always set; issues and PRs cite
  `specs/<path> @ spec_revision: N` — the same citation pattern we already use
  for roadmap revs.
- Required optional-sections by convention: `risks` and `open_questions` for
  anything touching an invariant surface; `user_experience` for anything with
  a screen, endpoint, or CLI (our receipts culture already produces the
  links this section wants — staging URLs, screenshot receipts, storybook).
- OpenAgents custom sections (round-trip-safe, standard-compliant):
  - `custom-owner-gates` — the NEEDS_OWNER items this work will hit, stated up
    front (compute rate, DNS, arming decisions…).
  - `custom-receipts` — the receipt kinds that will prove acceptance criteria
    (which Eval Suite, which behavior-contract IDs, which counters).
  - `custom-promise-links` — promise registry IDs this work feeds; the spec's
    success metrics must be consistent with the promise verification gates.
- `tool_metadata.openagents`: epic/issue refs, credit budget
  (`maxCreditsPerDay` for employee-run work), assurance level
  (hosted / BYO / regulated_private / Reactor), marginal-cost class of the
  execution capacity. Stripped on any public export, per the standard's own
  default.

### 4.3 The pipeline position (what changes, what doesn't)

```text
Product Spec (intent)            ← NEW uniform artifact
  → MASTER_ROADMAP (sequencing)  ← unchanged authority
  → epics/lanes (GitHub issues)  ← now cite spec@revision
  → behavior contracts + Eval Suites (enforcement)  ← spec links IDs, never duplicates
  → receipts (evidence)          ← unchanged; Decision Trace = portable projection
  → promise registry (public claims)  ← unchanged sole authority for claims
```

The spec is the *first* durable record for consequential work — the thing that
today exists as a dated `docs/fable/*.md` dossier in inconsistent shapes. Those
dossiers stay (they are analysis, evidence, argument); the spec is the distilled
committed-intent extract that machines can validate, diff, and hand off.

### 4.4 Business agents as authors and consumers (the main event)

- **Sarah (P1):** qualification output includes a draft
  `hypothesis`-type Product Spec per engagement — problem in the prospect's own
  words (`customer_truth`), scope with explicit cuts (which is where scope
  negotiation and deal rules meet: SR-2's `deal_rules.v1` prices what is `in`;
  `cut` is the honest record of what the price does not buy), acceptance
  criteria the fleet will build against, success metrics the engagement is
  judged by. The draft rides the existing approval queue — a spec is a
  *proposal* until owner/operator approval receipts it, exactly like outbound
  sends. This turns "Sarah closed a deal" into "Sarah produced a validated,
  citable work contract."
- **The BF engine / AE employees (P3–P5):** `agent_definition.v1` runs that do
  product work take a spec ref + revision in their work context; the
  definition's verification rubric is generated from (or checked against) the
  spec's acceptance criteria. AE-3 templates ship with a template-level
  Product Spec whose success-metrics section is the outcome ledger's
  declaration (AE-3.2's "no template lists without a receipted external
  outcome" becomes: outcome receipts must dereference to the template spec's
  declared metrics).
- **Fleet coding workers (MH lanes):** for spec-backed epics, the dispatch
  prompt includes the spec path + revision; workers report which acceptance
  criteria their PR satisfies (the standard's suggested PR text is already
  exactly our closeout-report shape). The claims/verify/refill law gains a
  stable "what does done mean" reference per claim.
- **Review agents (L2, later):** judge panels emit portable review annotations
  (per-section, per-axis verdicts) instead of only prose — giving us
  comparable review artifacts across Codex/Claude/Grok reviewers, which the
  harness-conformance program (MH-1) will want anyway.

### 4.5 Implementation: our own validator, their conformance corpus

Per the no-third-party-SaaS law and our Bun/Effect substrate:

- `packages/product-spec/` (`@openagentsinc/product-spec`): Effect Schema
  model of the parsed document + parser/validator + CLI (`bun productspec
  validate|init`). The reference implementation is ~600 LOC of TypeScript —
  a port into Effect Schema is small, and Effect Schema is *better* than the
  upstream JSON Schemas at expressing the structured blocks.
- Vendor the upstream `conformance/` fixtures (small, MIT, with attribution)
  as our compatibility oracle; add our own fixtures for the custom sections
  and `tool_metadata.openagents`. Track upstream releases through the existing
  `projects/` sync lane — `projects/repos/ProductSpec` stays the read-only
  reference clone.
- QA gate wiring: validate `specs/**/*.product-spec.md` in the existing static
  tier (same place depcruise/typecheck live). Errors block; warnings surface.
  Cross-checks unique to us, run in the same sweep:
  - every `custom-receipts` behavior-contract ID exists in the registry;
  - every `custom-promise-links` promise ID exists in the registry;
  - every spec cited by an open epic parses at the cited revision.
- Semantic-routing note: parsing this format is deterministic parsing of a
  bounded, explicitly-modeled document — allowed by the routing invariant
  (it is exactly the "explicitly modeled parser" case).

### 4.6 Decision Trace posture

Adopt the **format** as a projection, not as new authority. Our receipts,
owner-decision issue comments, and NEEDS_OWNER entries remain the source of
truth. What Decision Trace adds is a portable, per-spec digest:
`<name>.decision-trace.json` beside the spec, appended when (a) an owner
decision changes committed intent (`spec_revision` event), (b) drift is caught
between spec and reality (our oracle-weakening law, scope drift found in
review), or (c) an outcome review reads the success metrics
(promise flip / NO-GO). Business agents get a typed way to answer "why is v3
different from v2" without transcript archaeology — which is precisely the
company-brain provenance shape P4 wants.

Defer building drift *detection* — the vision doc's reconciliation loop is a
product we could build later (it rhymes with our seam-testing and QA-swarm
work), but adoption must not wait on it.

## 5. Operating rules (proposed law, lands with PS-1)

### 5.1 When a spec is required

A Product Spec is required before opening an epic when the work is
**consequential**: it changes a customer promise or public claim; it spans
multiple lanes/agents/repos; it will be executed by business agents or sold to
a customer; it changes an invariant surface; or it is a phase-level roadmap
item. One-lane mechanical work, refactors, migrations-by-plan-doc, and
fixes stay spec-free — the existing dated-plan-doc pattern (like the cloud
consolidation plan) remains correct for *engineering* programs; ProductSpec is
for *product intent*. When in doubt: if the work has a hypothesis about user
or buyer behavior, it gets a spec; if it only has a definition of done, it
gets a plan doc.

### 5.2 The three-line rule (adapted from upstream)

```text
No consequential epic without a Product Spec.
No fleet dispatch against a spec-backed epic without citing spec@revision.
No promise flip whose spec declared different success metrics — reconcile first.
```

### 5.3 What a spec may never do

- Duplicate behavior-contract oracles or Eval Suite content (link IDs instead).
- Carry public claims (registry only), secrets, customer data, or private
  pricing in public repos.
- Be edited to match implementation without a `spec_revision` bump and a
  Decision Trace entry — accidental behavior never silently becomes intent
  (this is the same law as "do not weaken an oracle," one layer up).

## 6. Rollout lanes (PS-*)

- **PS-1 — convention + first specs (no tooling).** Land `specs/` +
  `docs/specs/CONVENTIONS.md` (this doc's §4–§5 distilled), AGENTS.md routing
  paragraph, and 3 retro-specs to calibrate the shape against real work:
  one product lane in flight (WEB-1 landing), one business artifact
  (SR-2 deal engine), one template-shaped one (the lead-gen definition).
  Validate with the upstream CLI ad hoc for now. Exit: three specs reviewed,
  convention doc merged.
- **PS-2 — `@openagentsinc/product-spec`.** Effect Schema parser/validator/CLI,
  upstream conformance fixtures green, our extension fixtures added, wired
  into the QA static tier. Exit: gate blocks an invalid spec in CI evidence.
- **PS-3 — generator + agent contracts.** `productspec init`-equivalent in our
  CLI with our custom sections pre-stubbed; AGENTS.md + fleet dispatch
  instructions require spec citation for spec-backed epics. Exit: one fleet
  wave dispatched citing spec@revision with closeout reports naming satisfied
  acceptance criteria.
- **PS-4 — Sarah + BF integration.** Sarah's qualification tool emits a draft
  hypothesis spec into the approval queue; engagement provisioning consumes
  approved specs. Exit: first customer-facing engagement with an approved
  spec and receipts dereferencing to it.
- **PS-5 — Decision Trace projection.** Trace files generated from owner
  decisions and spec revisions for spec-backed work; company-brain ingestion
  lane for specs + traces (CB-1 dependency, not before P4). Exit: one spec
  with a v1→v2 revision fully explained by its trace.
- **PS-6 — L2 review annotations (opportunistic).** Review agents emit
  portable per-section verdicts for spec review; feeds the multi-harness
  conformance discipline. Value-gated; skip if MH work saturates capacity.

PS-1 is cheap and immediately useful; PS-2/PS-3 are small; PS-4 rides P1 Sarah
work that is already in flight; PS-5/PS-6 wait for their dependencies. None of
this blocks P0/P1 critical path — it slots under the "config, not fork" and
receipt-first disciplines that bind every phase.

## 7. What we deliberately do NOT adopt

- The npm parser as a runtime dependency (reference + conformance only).
- OpenSpec / Spec Kit downstream (our engineering layer already exists).
- The managed-GitHub-workflow product direction in their vision doc (we *are*
  the managed implementation for our own use; if a ProductSpec-editor product
  ever matters to us it is a separate business decision, not adoption).
- Review calibration serialization (L3) — no current consumer.
- Their `author`-as-person convention — roles/agents only, per our metadata law.

## 8. Owner gates

- Approve the required-when threshold (§5.1) and the three-line rule (§5.2) as
  standing law — these bind agents, so they are contract changes.
- Approve Sarah emitting engagement specs into the approval queue (PS-4) —
  customer-facing artifact class.
- Naming check: we say "Product Spec" for the artifact and "ProductSpec" for
  the standard (upstream's own convention); no conflict with owned vocabulary,
  and it must not be called anything Blueprint-branded — Blueprint remains our
  own modeling vocabulary, and the two must not blur.
