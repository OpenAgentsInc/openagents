# AssuranceSpec vs upstream ProductSpec — three-way gap analysis

Date: 2026-07-13 (updated same day for upstream v0.21.0/v0.22.0 **Agent Run**,
verified against `origin/main` `c7250a8` after the founder's "open intent
harness" announcement post — see §5.1)
Status: analysis; no implementation claims beyond those named with exact paths
Historical-status note (2026-07-14): this document preserves the pre-dogfood
gap snapshot and proposed sequencing. Current implementation and distribution
truth lives in [`README.md`](./README.md), the admitted MVP Evidence Index, and
[`assurance-spec-public-registry-receipt.json`](../../assurance/assurance-spec-public-registry-receipt.json);
do not read the historical "no npm publication" rows below as current state.
References:
- Ours, implemented: `packages/assurance-spec/` (`@openagentsinc/assurance-spec`
  0.1.0), `packages/product-spec/`, `apps/openagents-desktop/src/product-spec-workroom*`
- Ours, designed: [`ASSURANCE_SPEC.md`](./ASSURANCE_SPEC.md),
  [`OBSERVER_PRODUCT_PLAN.md`](./OBSERVER_PRODUCT_PLAN.md),
  [`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md),
  [`PRODUCTSPEC_EVIDENCE_LOOP.md`](./PRODUCTSPEC_EVIDENCE_LOOP.md)
- Theirs, shipped: `gokulrajaram/ProductSpec` `origin/main` at `c7250a8`,
  parser `@productspec/parser` 0.22.0 on main (**npm latest is still 0.19.0**
  — v0.20.0–v0.22.0, including all Agent Run functionality, are merged but not
  published to the registry, so the README's own
  `npm exec --package @productspec/parser -- productspec init-run` does not
  work from npm today); read-only clone `projects/repos/ProductSpec`
Companion documents: [`AGENT_TOOLING.md`](./AGENT_TOOLING.md) (our agent-surface
design), [`../fable/2026-07-13-productspec-assurance-qa-program-analysis.md`](../fable/2026-07-13-productspec-assurance-qa-program-analysis.md)
(strategy analysis)

## 0. How to read this

Three columns, kept honestly separate throughout:

- **Us — implemented.** Code on `origin/main` today: the ~1,290-LOC
  `@openagentsinc/assurance-spec` AS-1 slice (schema, parser, serializer,
  structural validator, adequacy assessment, deterministic proposal,
  committed-HEAD repository inventory, CLI with `propose`/`validate`/`coverage`),
  the generated 18-criterion MVP proposal in `docs/mvp/`, the shipping
  ProductSpec-native Desktop workroom loop, and read-only Agent Run 0.1 ingest
  in `packages/assurance-spec/src/agent-run.ts` / `handlers.ts`.
- **Us — designed.** The `docs/assurance/` dossiers: a full proof-design
  standard (14 laws, 9-section document, obligations/oracles/falsifiers/seams,
  Environment Profiles, adapter protocol, admission lifecycle, deterministic
  compiler, receipts with 8 status axes, Decision Trace, 4 conformance levels,
  a 12-layer authority matrix) plus the Observer product plan. Designed means
  designed. None of it is code unless the implemented column names it.
- **Them — shipped.** Upstream ProductSpec v0.22.0 (on main): the intent-layer
  standard plus a complete agent-adoption toolchain — 14-tool stdio MCP server,
  spec sessions with hash+revision pinning, two installable skills, a drop-in
  starter kit, a GitHub Action, a spec dependency graph, Decision Trace
  validation, a conformance corpus, JSON schemas, npm distribution (currently
  lagging at 0.19.0), and — new in v0.21.0/v0.22.0 — the **Agent Run**
  self-reported execution receipt (§5.1).

The one-sentence verdict: **our design goes materially deeper than upstream on
proof semantics; upstream is materially ahead of us on agent-adoption
ergonomics; our implementation is a deliberate thin slice of our own design.**
The gap that matters most in the near term is the middle one — agents can pick
up ProductSpec in any repo in about a minute, and they cannot do that with
AssuranceSpec yet. [`AGENT_TOOLING.md`](./AGENT_TOOLING.md) is the response.

A scope note: the two artifacts answer different questions. ProductSpec commits
product intent; AssuranceSpec commits proof design. Several rows below are
therefore not "upstream beat us" — upstream simply does not attempt oracle
adequacy, environment binding, or deterministic compilation, and we do not
attempt to restate intent. The rows where the comparison is direct are format
mechanics, tooling ergonomics, distribution, and conformance discipline.

## 1. Document format and data model

**Us — implemented.** `.assurance-spec.md` bounded proposal profile
(`packages/assurance-spec/src/schema.ts`, `parser.ts`, `serializer.ts`):
frontmatter (`assurance_spec_format_version: "0.1"`, id, revision, title,
`artifact_type: "product_assurance"`, `lifecycle_state: "proposed"`, author),
the 9 mandatory ordered sections (`assurance_objective`, `subject`,
`risk_model`, `assurance_scope`, `environments`, `obligations`, `gates`,
`evidence_policy`, `authority_boundaries`), typed fenced blocks, and a
round-trip-stable deterministic serializer with canonical JSON. Since #8760,
`custom-<kebab-name>` sections round-trip byte-stable after the mandatory
sections (the custom id is the heading in the bounded profile) and unknown
flat frontmatter keys are preserved verbatim; non-custom unknown sections
still fail `unsupported_section` and malformed custom ids fail
`invalid_custom_section_id`.

**Us — designed.** ASSURANCE_SPEC.md §3 adds the optional canonical sections
(`oracle_design`, `behavior_contracts`, `product_promises`, `test_data`,
`formal_models`, `observability`, `security_and_privacy`, `human_evaluation`,
`exception_policy`, `known_gaps`, `open_questions`, `rollout`,
`hosted_data_policy`) and `custom-<kebab-name>` extension sections that must
round-trip.

**Them — shipped.** `.product-spec.md` with 5 mandatory sections, 12 optional
sections, `custom-*` round-trip preservation, structured fenced blocks for
scope/AC/EVAL/SM/related-artifacts, durable `AC-`/`EVAL-`/`SM-` IDs with
never-renumber discipline, and unknown frontmatter preserved under
`parser_metadata.unknown_frontmatter`.

**Gap and action.** Custom-section and unknown-frontmatter preservation landed
with the conformance corpus (#8760). The remaining format gap is the optional
canonical sections (designed, unparsed); they ride along AS-MVP admission
review rather than landing as ad hoc parser patches. The mandatory-section
vocabulary itself needs no change — it is deliberately different because the
artifact is different.

## 2. Parser, validator, and diagnostics

**Us — implemented.** Hand-written Markdown parser → Effect Schema decode;
19 stable structural error codes exported as registries
(`ASSURANCE_STRUCTURAL_ERROR_CODES`: format-plane codes including
`unsupported_version`, `missing_required_section`, `duplicate_section`,
`invalid_section_order`, `unsupported_section`, `invalid_custom_section_id`,
plus referential-integrity codes for duplicate/dangling/uncovered criterion,
environment, and gate refs — run at parse time in the same pass), two
structural warning codes (`empty_required_section`, `thin_required_section` —
skeleton-narrative honesty that fires on the generated MVP proposal), all
separated from adequacy diagnostics (`obligation_needs_design` and coverage
counts; `design_ready` requires every obligation ready and zero warnings).
The structural-vs-adequacy split and code↔corpus parity are tested.

**Us — designed.** ASSURANCE_SPEC.md §12 specifies the full diagnostic
vocabulary as API: structural codes (`subject_document_digest_mismatch`,
`duplicate_obligation_id`, `dangling_source_ref`,
`cyclic_obligation_dependency`, …) and adequacy codes (`weak_oracle`,
`missing_falsifier`, `mock_only_coverage`, `missing_seam_coverage`, …).

**Them — shipped.** 17 typed validation error codes plus 3 warning classes
(`empty_required_section`, `thin_required_section`,
`unusual_related_artifact_target`), item-ID referential integrity for Related
Artifacts, duplicate-durable-ID rejection, 14 Decision Trace error codes, CRLF
and tilde-fence robustness.

**Gap and action.** The two upstream ideas worth adopting directly —
thin/empty-section **warnings** and referential-integrity checks at parse time
— landed with #8760. The remaining vocabulary gap is deliberate: the designed
adequacy codes (`weak_oracle`, `missing_falsifier`, …) should land only as
their underlying objects (oracles, falsifiers, seams) become parseable — a
diagnostic for a field the parser cannot see is theater.

## 3. Digests and subject binding

**Us — implemented.** One profile: `openagents_executable_v0.1_exact_document`.
Subject pins ProductSpec path, revision, and SHA-256 `document_digest` of exact
UTF-8 bytes, plus `criterion_refs`; the proposal utility also digests each
source claim. Every byte change to the subject stales the AssuranceSpec. The
checked-in MVP proposal binds revision 6 at
`fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`.

**Us — designed.** The dual-digest model (ASSURANCE_SPEC.md §4): `document_digest`
(exact bytes) plus `intent_digest` (canonical semantic projection), so an
evidence-attachment-only edit can be classified `evidence_index_changed` —
but only after a typed semantic diff proves that nothing but attachments
changed. This is the single most load-bearing design idea we have that upstream
lacks, and it depends on PSEL work (structured items) to compute.

**Them — shipped.** `begin_spec_session` pins `spec_revision` + SHA-256
`content_hash`; `check_spec_session` classifies drift and returns a typed
`recommended_action` (`continue_against_pinned_revision`,
`replan_before_continuing`, `resolve_invalid_current_spec`). Session state is
in-memory with a stateless fallback (client re-presents `started_hash` +
`started_revision`).

**Gap and action.** Upstream shipped the *session* half (pin, detect, recommend)
of what we designed; we shipped the *binding* half (exact digest in the
artifact). Neither of us has the intent digest — that is genuinely ours to
build (PSEL-0/PSEL-2), and it is what makes "the spec changed but only its
evidence index changed, keep working" safe instead of vibes. Action: adopt
upstream's session-pinning ergonomics in our MCP/CLI surface now against the
exact-document profile (see AGENT_TOOLING.md §4), and make the dual digest the
first PSEL deliverable rather than a nice-to-have.

## 4. Agent surfaces: MCP, CLI, skills, starter kit, CI

This is the widest gap and the reason AGENT_TOOLING.md exists.

**Us — implemented.**
- CLI: `assurance-spec propose|validate|coverage` (`src/cli.ts`; usage exits 2,
  failures exit 1). No session, no MCP, no init/scaffold.
- One production skill precedent, for **ProductSpec** not AssuranceSpec: the
  Desktop builtin `productspec-work` skill
  (`apps/openagents-desktop/resources/builtin-skills/productspec-work/SKILL.md`),
  hash-pinned, installed only into the named isolated Codex skill root, bound
  to the typed `product_spec` host-tool namespace (`get_run`, `propose_edit`,
  `propose_plan`, `report_blocked`, `record_evidence`).
- No AssuranceSpec MCP server, no AssuranceSpec skill, no starter kit, no
  GitHub Action, no npm publication of `@openagentsinc/assurance-spec`.

**Us — designed.** ASSURANCE_SPEC.md §7 defines a framework-neutral adapter
protocol (`describe`/`validate`/`compile`/`execute`/`normalize`/`publicProject`)
— an execution-side contract, not an agent-interaction surface. The design docs
do not yet specify agent tooling; AGENT_TOOLING.md now does.

**Them — shipped.** Three complementary surfaces plus distribution:
1. **MCP server** — `productspec mcp`, hand-rolled JSON-RPC 2.0 over stdio
   (no SDK), protocol `2024-11-05`, 14 tools: `begin_spec_session`,
   `check_spec_session`, `list_product_specs`, `get_product_spec`,
   `validate_product_spec`, `get_scope`, `get_acceptance_criteria`,
   `get_ai_evals`, `get_success_metrics`, `get_related_artifacts`,
   `get_spec_graph`, `get_evidence_checklist`, `draft_agent_run` (v0.22.0),
   `check_completion_claim`.
   Path resolution confined to `root`, symlinks skipped, deterministic, never
   calls an LLM, never judges code correctness.
2. **Two skills** — `productspec` (implement under a spec) and
   `productspec-authoring` (write/validate/convert), installable via
   `npx skills add gokulrajaram/ProductSpec`.
3. **Starter kit** — drop-in `AGENTS.md`/`CLAUDE.md` stanzas, PR template,
   `.github/workflows/productspec.yml`, `docs/product-specs/` +
   `docs/decision-traces/` layout.
4. **GitHub Action** (`gokulrajaram/ProductSpec@main`) validating
   `**/*.product-spec.md` + decision traces in CI; npm distribution runnable
   as `npx --yes -p @productspec/parser@latest productspec mcp`.

**Gap and action.** Upstream is simply ahead here, and the pattern is good:
deterministic read-mostly tools, session pinning, a checklist tool, and a
completion-claim tool that returns what still needs verification instead of a
verdict. We should build our own three-surface equivalent for AssuranceSpec —
not by copying, but by exposing the objects upstream doesn't have (obligations,
oracles, falsifiers, seams, gates, environments, the three coverage ledgers,
the 8 status axes) through the same ergonomic shape. Full design, including the
exact tool table and what can ship against the current package versus what
waits for the compiler: [`AGENT_TOOLING.md`](./AGENT_TOOLING.md).

## 5. Evidence loop

**Us — implemented.** The Desktop workroom loop is real and shipping: accepted
plans, work packets, leases, evidence envelopes, independent-verification refs,
owner packet disposition (`apps/openagents-desktop/src/product-spec-workroom*`).
It is work-state authority, not assurance-state authority (Law 14). Separately,
`packages/product-spec` now implements the pinned upstream structured
AC/EVAL/SM, Related Artifact, dual-digest/evidence-edit, and Decision Trace
v0.1 surfaces. The upstream dependency graph and MCP Evidence Loop checklist
remain unsupported. The admitted MVP ProductSpec's `CW-AC-*` profile is still
not valid upstream; the portable revision 7 plus Decision Trace are proposed,
not silently adopted.

**Us — designed.** PRODUCTSPEC_EVIDENCE_LOOP.md adopts upstream's Related
Artifacts as the portable attachment layer (PSEL-0…PSEL-4) rather than building
a competing ledger, with the `CW-AC-01…18` → `AC-1…18` ID migration
(PSEL-2) gated on a machine-readable ID map plus a Decision Trace. Downstream,
ASSURANCE_SPEC.md §10 designs typed Assurance Receipts, the
`openagents.assurance_receipt_bridge.v1` into the workroom, and public
projection rules (Law 11). None of that receipt path exists.

**Them — shipped.** Related Artifacts (item-level links from durable IDs to 14
target kinds), `get_evidence_checklist` (per-AC/EVAL/SM `evidence_needed` with
attached artifacts, release-blocking flags), Decision Trace as the
reconciliation record, and worked examples (`examples/evidence-loop/`).
ProductSpec stores no evidence itself and never claims a link is a verdict —
the same boundary as our Law 13.

**Gap and action.** The former document-format parity debt is closed for the
pinned profile; adoption of the portable MVP revision remains owner-gated.
The remaining direct parity debt is agent ergonomics (dependency graph and MCP
Evidence Loop checklist/session behavior). **Our extension:** the
receipt pipeline (normalized receipts → workroom bridge → Related Artifact
publication) has a narrow implemented `AO-CW-AC-04-01` dogfood path, not a
general automatic publication system.

### 5.1 Agent Run (upstream v0.21.0/v0.22.0, verified 2026-07-13)

The founder's 2026-07-13 announcement repositions ProductSpec as "the open
intent harness for AI-native software work" and introduces **Agent Run**, "a
receipt for one agent execution against a pinned Product Spec." Verified
against `origin/main` `c7250a8`:

**Shipped on main.**
- v0.21.0 "Agent Harness Records": the `.agent-run.json` companion artifact,
  `productspec validate-run`, `schema/agent-run.schema.json`, an
  agent-ready-repo example, and the README/agent-docs "intent harness
  contract" repositioning.
- v0.22.0 "Agent Run Drafting": `productspec init-run <spec.product-spec.md>
  [run.agent-run.json]` (output path optional, unlike the post's two-argument
  form), the MCP `draft_agent_run` tool, the `draft` run status, and
  conformance fixtures (`conformance/valid/minimal.agent-run.json`,
  invalid-status and missing-required-field fixtures).
- In the "Unreleased" changelog section but on main: the GitHub Action's
  optional `agent_runs` globs and a starter-kit Agent Run example validated in
  CI.

**Announced but not distributed.** npm latest is 0.19.0; none of the Agent Run
functionality is installable from the registry, so the post's quoted command
only works from a source checkout today.

**Exact shape** (`agent_run_format_version: "0.1"`). Required: `run_id`,
`agent { name, version? }`, `product_spec { path, spec_revision,
content_hash? }` — note the hash pin is *optional*, weaker than their own spec
sessions — `started_at`, `status` ∈ `draft`/`completed`/`blocked`/`failed`,
`checked_items[]` (`item_id` matching `^(AC|EVAL|SM)-[1-9][0-9]*$`, `status` ∈
`passed`/`failed`/`not_checked`/`blocked`, optional `evidence[]` links from
the same 14-type vocabulary, optional `notes`), and `drift { detected,
decision_trace_path?, summary? }`. Optional: `completed_at`,
`completion_claim` (free text).

**What it is not.** `validate-run` validates the JSON in isolation: shape
only. It does not cross-check that `checked_items` IDs exist in the referenced
spec, does not recompute `content_hash`, and does not dereference evidence
links. There is no `agent_run` type in `RELATED_ARTIFACT_TYPES`, so a Product
Spec cannot attach a run with first-class vocabulary — the binding is
directional (run pins spec), and a spec-side pointer must use `other`.
Most importantly, **the receipt is self-reported**: the same agent that did
the work fills in `passed`/`failed`, drift, and the completion claim. One
status axis per item, no independence policy, no oracle or falsifier concept,
no environment binding, no producer identity beyond a self-declared
`agent.name`.

**Read and action.** This validates the receipts thesis — the intent layer's
own maintainer now agrees that "done" must be a durable per-criterion record
against a pinned spec, which is the "AI-accountable work" market both layers
serve. It also marks, precisely, where our layer begins: an Agent Run is a
*claim*; an Assurance Receipt is an adapter-produced *observation* with
provenance, environment, oracle sensitivity, independence
(`producer_may_verify: false`), and eight axes that never round up (Law 7,
Law 10). Upstream shipping the self-report rung does not compress our
differentiation — it names the baseline we verify above. Implemented action:
`.agent-run.json` is now an ingestable low-rung evidence *pointer* (never a
verdict — Law 13), mapped into our typed model with `producer == claimant`
flagged. The CLI `agent-run ingest` verb and read-only MCP `ingest_agent_run`
tool validate the 0.1 shape, cross-check ProductSpec revision and cited item
IDs, verify an optional digest, and return a typed gap when that hash is
absent. They never promote the observation axis or satisfy independent-
producer policy. Emit remains deferred. The upstream-proposal candidate list in the
companion strategy doc (fable §4.2) gains a third cheap item: an `agent_run`
Related Artifact type, so the artifact they created has first-class attachment
vocabulary in their own spec.

## 6. Dependency graph

**Us — implemented (#8761).** Obligation-level `dependency_refs` are a
schema field; the validator emits the designed `cyclic_obligation_dependency`
structural code plus `self_obligation_dependency` and
`dangling_dependency_ref`; a pure `projectObligationGraph` returns
designable-now vs blocked (`waits_on`) vs gated with a dependency-respecting
`design_order`, exposed as the CLI `graph` command and the MCP
`get_obligation_graph` tool through one shared handler. Still no spec-to-spec
graph (deliberate — see below).

**Us — designed.** Obligation dependencies and activation gates (§5);
manifest-level ordering falls out of the deterministic compiler (§9).

**Them — shipped.** `product_spec`-typed related artifacts with
`depends_on`/`blocks`/`supersedes`/`relates_to`, a pure
`resolveProductSpecGraph`, `productspec graph` CLI and `get_spec_graph` MCP tool
returning `buildable`/`blocked` (+`waits_on`)/topological `order`, with typed
graph warnings (`dependency_cycle`, `missing_link_target`, …).

**Gap and action.** Upstream's buildable/blocked/order projection is directly
useful for fleets and costs little. For us the equivalent near-term win is
obligation-level: cycle detection plus a "which obligations are designable now
versus blocked on a dependency/gate" projection over the existing parsed model.
Spec-to-spec graphs matter for us only after there is more than one
AssuranceSpec — do not build that shelf yet.

## 7. Conformance, schemas, and distribution

**Us — implemented.** Bun test suite covering round-trip determinism, coverage
exactness, structural-vs-adequacy separation, committed-HEAD inventory binding,
and (since #8760) a committed conformance corpus at
`packages/assurance-spec/conformance/` — `valid/` seeded from the MVP proposal
plus minimal/designed/custom-section/unknown-frontmatter fixtures that must
round-trip byte-stable, `invalid/` with one filename-coded fixture per
implemented error code, and `review/` for the portable review-annotation
format (parse/serialize/validate plus exact subject binding of revision and
digest across the 12 recommended axes). Code↔corpus coverage and
schema/parser frontmatter parity are enforced by test. Still true: no
published JSON schema, not published to npm (workspace-only), and no
review-annotation *tooling* (grading UX, aggregation, admission — deliberately
deferred).

**Us — designed.** Four conformance levels (AS-L1 document … AS-L4 evidence
lifecycle), stable error codes as API, portable review annotations grading 12
axes (oracle adequacy, falsifier strength, seam reality, …), golden-byte
compiler fixtures. Law 12: conformance is interoperability, not quality.

**Them — shipped.** A `conformance/` corpus (9 valid, 7 invalid, 4-spec graph
fixture), three JSON Schemas (product-spec, decision-trace, review-annotation)
with schema/parser parity enforced by test, npm publication with `--yes`
non-interactive ergonomics, CI validating fixtures and examples plus a publish
dry-run.

**Gap and action.** A conformance corpus is how a format stops being "whatever
the one parser accepts" — landed (#8760), with the growth rule enforced
mechanically: a new error code cannot merge without an invalid fixture, and
any change that can invalidate a previously valid document must bump
`assurance_spec_format_version` and freeze the prior corpus per version.
npm publication should still wait until the format survives its first dogfood
revision — publishing a format that churns weekly is upstream's mistake to
learn from, not copy (they shipped 20 minor versions while
`spec_format_version` stayed "0.1", and old documents fail new validators).
Schema/parser parity is already enforced by test ahead of any publication.

## 8. Deterministic compilation, receipts, status axes

**Us — implemented.** Nothing beyond the deterministic *proposal* (which is a
pure function and tested byte-stable, but is scaffolding generation, not
compilation).

**Us — designed.** The pure compiler (§9: canonical JSON, stable ordering, no
network/clock/random/model calls, golden-byte fixtures), Environment Profiles
(§6), adapter lock, normalized receipts with 8 never-collapsing status axes
(§10), and the Assurance Decision Trace (§11).

**Them — shipped.** Nothing comparable, deliberately — upstream stops at intent
and evidence pointers. Their validator "judges structure, not quality" and
their MCP "never judges code correctness." There is no upstream equivalent of
manifests, environments, oracle sensitivity, or status axes.

**Gap and action.** This is our designed moat, and it is 100% unbuilt. The
honest sequencing is already written down (AS-2 → AS-3 in ASSURANCE_SPEC.md
§16; AS-MVP-0…7 in MVP_FIRST_ASSURANCESPEC.md): compile exactly one admitted
obligation (`AO-CW-AC-04-01`) against one Environment Profile
(`ENV-OA-DESKTOP-MVP-VITE-PLUS-1`) through one adapter
(`openagents.vite_plus_test.v1`) and land
one receipt through the workroom bridge. Every generalization before that
single vertical slice runs is speculative shelf-building — the Observer plan
names this risk itself ("building a universal taxonomy before one useful
vertical slice").

## 9. Hosted and product surface

**Us — implemented.** No hosted assurance surface. An Observer marketing
landing lane for `openagents.com/observer` is in flight in a concurrent lane;
at the time of writing that path serves a redirect to the homepage, so treat
any "live landing page" claim as pending its own deploy receipt.

**Us — designed.** OBSERVER_PRODUCT_PLAN.md: local OSS/BYO core (validation,
compiler, adapter SDK, no account required), a later hosted service (managed
browser/device/simulator matrices, parallel private swarm, retained encrypted
evidence, share pages), and Observatory as a possible multi-project public
evidence surface reading only approved projections. "The paid service sells
managed environments, compute, expertise, and evidence retention. It does not
hold the basic proof contract hostage."

**Them — shipped (or positioned).** ProductSpec.io, a hosted browser editor
(referenced, closed, optional), and a "managed implementation" described as
vision in `docs/vision.md` — not open source, not required by the standard.
The open repo keeps everything an agent needs local and free.

**Gap and action.** Both sides keep the open core self-sufficient and sell
convenience above it; the postures are compatible and neither has shipped the
paid layer. Nothing to build here until AS-4 evidence exists — a hosted
evidence surface with no evidence is a landing page, which is exactly what the
concurrent lane is scoped to.

## 10. Where our design deliberately goes further

Not gaps — commitments upstream does not attempt, kept here so the previous
sections' "upstream is ahead" reading stays calibrated:

- **14 design laws** with an authority matrix (§18) — upstream has design
  principles, not a layered authority model.
- **Obligations as reviewed proof claims** with oracles, falsifiers,
  independence, and proof rungs — upstream ACs are intent statements.
- **Oracle sensitivity** (Law 4): a required oracle must name something it
  rejects. No upstream analogue.
- **Seams as first-class objects** (Law 5): two green component tests do not
  prove a connection between real sides.
- **Environment-bound evidence** (Law 6) with digest-pinned Environment
  Profiles — upstream has no environment concept at all.
- **Deterministic compilation** to an immutable manifest (Law 3).
- **8 non-collapsing status axes** (Law 7) versus upstream's binary
  valid/invalid plus checklist prose.
- **Typed gaps instead of skip-and-green** (Law 8).
- **Dual document/intent digests** versus content-hash-only sessions.
- **Receipts report; people and policy decide** (Law 10) — upstream's
  `check_completion_claim` gestures at this; our version is a lifecycle
  commitment. Agent Run (§5.1) sharpens the contrast rather than closing it:
  their receipt is filled in by the agent whose work it certifies, which is
  the exact false-green failure mode `producer_may_verify: false` exists to
  exclude.

And, symmetrically, where upstream is simply ahead and we should not
rationalize it: agent ergonomics (14 tools, sessions, skills, starter kit,
Action, npm), conformance corpus, custom-section preservation, graph
projection, and sheer distribution surface. Their velocity (v0.7 → v0.22 in
days) also bought them format drift we must not import — and, as of v0.21, a
three-release npm publication lag that leaves their announced headline feature
uninstallable.

## 11. Consolidated action list

Ordered by leverage, each mapped to its ladder home:

1. **PSEL-0** — catch `packages/product-spec` up to upstream structured items +
   Related Artifacts; compute the intent digest. Unblocks §3, §5, §6.
2. **Agent tooling first slice** (AGENT_TOOLING.md) — shipped CLI
   session/checklist commands, read-only MCP server, and Agent Run 0.1 ingest
   over the implemented parser/validator/coverage surfaces.
3. **Conformance corpus + custom-section preservation + thin-section warnings**
   — completes AS-1 honestly. Landed (#8760), including the portable
   review-annotation format with exact subject binding.
4. **AS-MVP vertical slice** — admit, compile, execute, and bridge exactly
   `AO-CW-AC-04-01`. The first real test of everything designed in §8.
5. **Obligation-graph projection** (designable-now vs blocked) — small, useful,
   feeds both CLI and MCP. *(Shipped, #8761: `graph` CLI command +
   `get_obligation_graph` MCP tool over `projectObligationGraph`.)*
6. Defer: npm publication (until post-dogfood format stability), spec-to-spec
   graphs, review-annotation tooling, anything hosted.
