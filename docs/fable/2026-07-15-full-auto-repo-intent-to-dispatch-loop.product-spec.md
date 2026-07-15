---
spec_format_version: "0.1"
title: "Full Auto: Repo-Intent-to-Dispatch Loop"
artifact_type: "openspec_proposal"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-15T00:00:00Z"
updated_at: "2026-07-15T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-prior-art"
    label: "Prior Art And Naming Collision"
    after: "problem"
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_lane: "docs/fable strategy proposal; not an admitted plan of record"
  openagents_status: "proposed, unreviewed, zero implementation"
  openagents_depends_on: "MVP-01 #8756 (closed); Codex parity epic #8831 (open, esp. CAP-10 #8842); PSEL-2 #8758 (open)"
  openagents_supersedes_claim_for: "autopilot.desktop_full_auto_guidance.v1 (red, docs/promises/registry.md)"
---

## Problem

OpenAgents Desktop's MVP gives a developer one signed, local-first Codex
workroom: grant a repository, author or open a ProductSpec, accept a plan,
watch Codex agents work criterion-linked packets to evidence and verified
disposition. Every step still requires the developer to originate the intent
by hand — write the problem statement, name the hypothesis, enumerate
acceptance criteria — before the workroom can do anything. That is correct for
the MVP's narrow, dogfooded, human-authored posture, but it leaves a large
class of real work stranded: a developer who already has a working repository
with docs, a README, open issues, and a rough sense of "what this project is
trying to become," but who has not yet — and may never — sit down to write a
formal ProductSpec for it.

At the same time, this workspace already runs on two typed artifacts that
exist for exactly this purpose once they are populated: **ProductSpec**
commits *what we intend* (problem, hypothesis, scope, acceptance criteria,
success metrics), and **AssuranceSpec** commits *how we intend to know*
(obligations, falsifiers, evidence policy, authority boundaries) bound to an
exact ProductSpec digest and revision. Both are mechanically validated,
both are already wired into the Desktop workroom's packet/evidence/
verification/disposition loop, and both are designed to be produced by
tooling, not only by a human typing into a guided conversation. Nothing in
the current implementation, however, closes the loop from "here is a
repository with unstructured intent already latent in its docs and issues"
to "here is a validated ProductSpec/AssuranceSpec pair, admitted, dispatched
to Codex agents, and re-run against the repository's next state."

**Full Auto** is a proposed toggleable mode that closes exactly that loop: given
a granted GitHub repository, infer what its owner is actually trying to build
from its own docs and issues, draft a validator-clean ProductSpec proposal
from that inference, wrap it in an AssuranceSpec proposal that states how the
resulting work would be verified, route the pair through an explicit admission
gate, dispatch the admitted plan's work packets to Codex agents through the
workroom's existing packet/dispatch machinery, and — once evidence, verification,
and disposition land — repeat the whole cycle against the repository's new
state. The problem this spec solves is narrower than "build an autonomous
software engineer": it is "remove the requirement that a human hand-author the
ProductSpec/AssuranceSpec pair before OpenAgents' existing, already-trustworthy
execution loop can run," while keeping every authority boundary that loop
already enforces exactly where it is today.

## Hypothesis

If OpenAgents ships a toggleable Full Auto mode that (1) infers a repository's
latent product intent from its own docs, README, and open issues through a
cited, semantic synthesis pass — never keyword matching — (2) mechanically
drafts a validator-clean ProductSpec proposal and a digest-bound AssuranceSpec
proposal from that inference, (3) requires those proposals to pass through an
explicit, typed admission gate — either manual owner review or a narrow,
owner-signed auto-admit envelope scoped by risk category, path, spend, and
packet count — before any Codex dispatch occurs, and (4) reuses the existing
ProductSpec workroom packet lifecycle, Pylon/Codex fleet dispatch contract,
and evidence/verification/disposition loop unmodified rather than building a
parallel execution path, then developers will be able to point Full Auto at a
repository they already own, receive a proposed, reviewable plan of real work
derived from their own project's stated direction, and — within an envelope
they explicitly signed — let bounded categories of that work proceed to
verified, owner-visible completion without hand-authoring a spec first,
without the loop ever gaining authority the existing MVP contract does not
already grant to a human-authored ProductSpec.

## Scope

```productspec-scope
in:
  - a toggleable Full Auto mode, off by default, exposed per granted repository
  - the observe stage: a bounded, typed repository-signal extraction (README, docs tree, CONTRIBUTING, existing *.product-spec.md and *.assurance-spec.md files, open issues within the granted scope, recent commit subjects) reusing the AssuranceSpec RepositoryInventorySchema shape rather than inventing new repo-scanning machinery
  - the infer stage: a cited semantic synthesis pass that produces a typed InferredRepoIntent (problem draft, hypothesis draft, candidate scope in/out, candidate acceptance-criterion drafts, confidence, exact source citations) using a central semantic selector or LLM synthesis, never ad hoc keyword or regex matching, per this workspace's Semantic Routing And Retrieval invariant
  - the draft stage: mechanical rendering of InferredRepoIntent into a validator-clean ProductSpec proposal (openagents profile, structured AC-<n>/SM-<n> criterion and metric IDs from the first revision so the artifact never inherits the CW-AC legacy-ID migration debt tracked by PSEL-2 #8758) and a companion, digest-bound AssuranceSpec proposal with one obligation per criterion and every AssuranceAuthoritySchema boolean at its structural false default
  - the admission gate: manual owner review (default) and an optional, explicitly owner-signed FullAutoEnvelope that bounds auto-admission by risk category, forbidden path globs, maximum packet count, and maximum spend/time per loop iteration
  - reuse, unmodified, of the existing ProductSpec workroom packet lifecycle (planned/active/blocked/evidence_present/verified/failed/superseded/cancelled), the existing Pylon to Khala to Codex codex_agent_task dispatch contract, and the existing evidence-envelope/verification-receipt/owner-disposition loop
  - a bounded, narrow auto-disposition path restricted to the envelope's lowest-risk category (documentation and test-only packets with full green verification), with every other category always requiring human disposition
  - the loop stage: re-running observe against updated repository state on a scheduled cadence (not a tight spin loop) after each disposition, producing either a new spec_revision of the same ProductSpec or a distinct new proposed ProductSpec when inferred intent materially diverges
  - a global owner kill switch, per-repository allowlist, durable spend/time/packet budgets, and durable owner-visible logging of every stage transition, reusing existing receipt/evidence infrastructure rather than a parallel logging system
  - explicit non-claim: adopting this spec does not itself flip any promise-registry entry green
out:
  - fully unattended admission or disposition outside a signed FullAutoEnvelope's narrowest category
  - any new execution engine, dispatch protocol, or evidence/verification model distinct from the one the Desktop MVP already ships
  - Codex, provider, or model selection changes; Full Auto dispatches through the same Pylon/Codex fleet contract every other coding-delegation path uses
  - direct, unreviewed commit, merge, force-push, or history rewrite by any Full Auto packet; dispatched agents open proposals exactly as the MVP's rollout boundary already requires
  - running Full Auto against a repository that was not explicitly granted by its owner
  - claiming, reopening, or altering the autopilot.desktop_full_auto_guidance.v1 promise-registry entry's disposition; only a separate, explicit registry pass may do that, after real receipts exist
  - scanning or inferring intent from private, unrelated, or non-granted repositories, issues, or account data
cut:
  - CUT-FA-01: cross-repository intent inference (proposing work on repo B because of something inferred from granted repo A)
  - CUT-FA-02: any budget-exceeding auto-escalation; hitting a budget always stops and requires explicit owner re-authorization, never a soft warning that continues spending
  - CUT-FA-03: multi-account or Fleet-wide Full Auto orchestration; this proposal is scoped to one owner, one granted repository, one envelope at a time
  - CUT-FA-04: any change to how release or public-claim authority works; the promise registry alone still governs claims
```

## User Experience

A developer who has already granted OpenAgents Desktop access to a repository
sees a **Full Auto** toggle beside that repository's existing ProductSpec
entry point, off by default. Turning it on with no further configuration does
exactly one thing: it starts the observe/infer/draft loop and surfaces
proposed ProductSpec/AssuranceSpec pairs in the same review surface used for
hand-authored specs, exactly as if a very fast, well-read collaborator had
opened a pull request against the project's own backlog. Nothing dispatches
without the owner accepting a proposed plan, identical to today's manual flow.

A developer who wants bounded unattended progress additionally opens **Full
Auto envelope settings** and signs a narrow policy: which risk categories may
auto-admit (starting with documentation and test-only changes), which path
globs are always excluded (auth, payments, secrets, schema/migration, public
promise copy), and a spend/time/packet ceiling per loop iteration. Signing
that envelope is a single explicit action, distinct from the toggle itself —
toggling Full Auto on without an envelope never implies auto-admission.

At any point the developer can see, per loop iteration: what was observed
(exact file/issue refs), what was inferred and why (cited synthesis, not an
opaque summary), the proposed spec pair, its admission disposition (manual,
envelope-admitted, or rejected), the dispatched packets and their evidence,
and the loop's next scheduled tick. A single kill switch stops new admission
and new dispatch immediately; already-dispatched packets either finish or are
told to stop cleanly, matching the existing workroom's stop/steer semantics.

## Solution

### Stage 1 — Observe: bounded repository-signal extraction

Full Auto's first stage produces a typed, bounded repository-signal snapshot,
not an arbitrary crawl. It reuses the shape AssuranceSpec's
`RepositoryInventorySchema` already defines — git head/tree state, tracked
file count, candidate artifact refs, declared scripts, an inventory digest,
explicit truncation, and diagnostics — so Full Auto's "what does this repo
currently look like" question is answered with the same typed, digest-pinned
mechanism the AssuranceSpec environment model already uses, rather than a
second bespoke scanner. The signal set is narrow and named: README, top-level
docs tree, CONTRIBUTING, any existing `*.product-spec.md` and
`*.assurance-spec.md` files (an existing spec is signal, not noise — Full Auto
must read what has already been declared before proposing something new),
open issues within the granted scope (via the same GitHub access already used
for repository grant), and recent commit subjects. Nothing outside this named
set is read; there is no "also browse whatever looks interesting" step.

### Stage 2 — Infer: cited semantic synthesis, never keyword matching

The bounded signal set feeds one semantic synthesis pass that produces a typed
`InferredRepoIntent`: a problem draft, a hypothesis draft, candidate scope
`in`/`out` bullets, candidate acceptance-criterion drafts, a confidence score,
and — non-negotiably — an exact citation for every claim (the file path, line
range, or issue number the inference drew from). An inference with an uncited
claim is invalid output, not a lower-confidence one. This stage must run
through a central typed semantic selector or embedding/LLM synthesis path, per
this workspace's Semantic Routing And Retrieval invariant; it must never
degrade into ad hoc string or keyword matching over README prose to guess
intent, even as a fallback or performance shortcut.

### Stage 3 — Draft: mechanical, validator-clean spec rendering

`InferredRepoIntent` is mechanically rendered into a ProductSpec proposal
using the existing `@openagentsinc/product-spec` schema and serializer, then
validated with the existing `openagents` profile before it is ever shown to a
human or touched by anything downstream. Unlike the live MVP spec's legacy
`CW-AC-*` IDs, Full Auto's generated criteria use structured `AC-<n>` and
`SM-<n>` IDs from `spec_revision: 1` onward, so a generated spec is
constructed to also satisfy the stricter `upstream` profile from day one and
never accrues the migration debt PSEL-2 (#8758) exists to pay down for
hand-authored legacy specs. `tool_metadata` on the draft records
`openagents_full_auto: "proposed"`, the exact inference citations, and the
observe-stage inventory digest it was drawn from; it is stripped on any public
export exactly like every other ProductSpec's `tool_metadata`.

Immediately after the ProductSpec proposal validates, Full Auto compiles a
companion AssuranceSpec proposal via `packages/assurance-spec`: one obligation
per acceptance criterion, `subject.product_spec` bound to the draft
ProductSpec's exact `document_digest`, `spec_revision`, and criterion refs,
falsifiers where the criterion's shape supports one, and
`evidence_policy.missing_evidence_verdict: "INCONCLUSIVE"` with
`policy_state: "needs_design"` until a human or the envelope's rules complete
proof design per obligation. Every `AssuranceAuthoritySchema` field —
`proposal_may_self_admit`, `proposal_may_execute`, `proposal_may_verify`,
`proposal_may_release`, `proposal_may_change_public_promises` — stays at its
literal-`false` type-level default. Full Auto does not merely choose not to
set these to true; the schema makes it structurally impossible for a
proposal-lifecycle AssuranceSpec to grant itself any of that authority.

### Stage 4 — Admit: the actual boundary between "proposed" and "real work"

A proposed ProductSpec/AssuranceSpec pair is inert until it passes an explicit
admission gate. Two paths exist:

- **Manual (default):** the pair appears in the same Desktop workroom review
  surface used for hand-authored specs. The owner edits, accepts, or rejects
  it exactly as they would their own draft. Full Auto contributes nothing here
  that a human collaborator opening a well-researched proposal would not
  already contribute.
- **Bounded auto-admit envelope (opt-in, explicit, signed once):** the owner
  signs a `FullAutoEnvelope` — an owner-authored, versioned policy object,
  not a Full Auto output — naming: which risk categories may auto-admit
  (starting only with documentation and test-only changes), forbidden path
  globs (auth, payments, secrets, credentials, schema/migration, public
  promise/registry copy are excluded by the shipped default and cannot be
  removed from the exclusion list by the envelope itself), a maximum packet
  count per loop iteration, and a maximum spend/time budget per iteration.
  A proposal outside the envelope's named category or touching an excluded
  path always falls back to manual admission, with no exception path.

This mirrors the existing Pylon dispatch-gate pattern — bounded, typed,
explicit, capacity-advertised — rather than introducing a blanket "autopilot
on" switch. Toggling Full Auto on without ever signing an envelope is fully
safe: it produces proposals for review and dispatches nothing on its own.

### Stage 5 — Compile and dispatch: no new execution path

Once admitted, by either path, the plan's work packets are governed entirely
by the existing `product-spec-workroom` packet lifecycle
(`planned → active → blocked → evidence_present → verified → failed →
superseded → cancelled`). Dispatch reuses the existing Pylon → Khala → Codex
`codex_agent_task` request/proof contract for owner-local, Desktop-attended
execution. For genuinely unattended, scheduled runs — the loop ticking while
the owner is not present — dispatch instead routes through
`oa-codex-control`/GCE placement on OpenAgents' own managed Cloud
infrastructure, never as an ambient background job inside an interactive
session; this matches the standing rule that autonomous or unattended
execution belongs on OpenAgents' own Cloud, not folded into any single
session. The Cloud-routed variant is sequenced behind the open Codex
app-server parity program's CAP-10 (#8842, "experimental environments,
processes, terminals, realtime, and remote control") landing or an equivalent
reviewed background-execution surface; until then, Full Auto's dispatch stays
Desktop-attended only.

### Stage 6 — Evidence, verification, disposition: unchanged

Evidence envelopes, verification receipts (`verifierRef !== producerRef`),
and owner disposition (`accepted`/`waived`) follow the identical contract the
MVP already ships. AssuranceSpec obligations feed the workroom's evidence
index by reference only; neither the AssuranceSpec proposal nor a green
Observer/QA receipt ever grants release or public-claim authority by itself.
Auto-disposition — skipping a human `accepted`/`waived` decision — is only
ever available inside the signed envelope's lowest-risk category, and only
when every one of that category's obligations reports a `CONFIRMED`
verification receipt; anything else, including any packet touching an
envelope-excluded path, always stops for human disposition.

### Stage 7 — Loop: repeat against the repository's next state

After disposition, Full Auto re-runs Stage 1 against the repository's updated
state on a scheduled cadence — an hourly or daily tick, or a push/issue
webhook trigger — never a tight spin loop chasing constant motion inside a
single session. The new observation either continues the same initiative (a
new `spec_revision` of the same ProductSpec, with the existing digest/revision
mismatch handling from CW-AC-05/CW-AC-09 governing how in-flight work
reconciles across the bump) or, when inferred intent has materially diverged,
produces a distinct new proposed ProductSpec rather than silently overwriting
the prior one's intent. A spec that was already admitted and is still active
is never retargeted by a later loop tick without the same explicit
reconciliation the MVP already requires for any revision change.

### Autonomy policy, stop controls, logging, budgets, review gates

These five items are named explicitly because they are the exact bar the
existing red promise-registry entry (see Prior Art, below) already identifies
as missing before anything resembling "full auto" could be claimed:

- **Autonomy policy:** the `FullAutoEnvelope` is the whole policy surface —
  owner-authored, versioned, narrowly scoped by category/path/budget — never
  an implicit "trust the loop" default.
- **Stop controls:** one global kill switch per owner, one per-repository
  allowlist entry, and the existing packet-level stop/steer semantics; a
  kill switch halts new admission and new dispatch immediately.
- **Logging:** every stage transition (observe, infer, draft, admit, dispatch,
  evidence, verify, dispose, loop-continue) is a durable, owner-visible
  receipt, reusing the workroom's existing receipt/evidence machinery rather
  than a parallel log store.
- **Budgets:** spend, wall-clock, and packet-count ceilings per loop
  iteration; exceeding a budget always stops the iteration and requires
  explicit owner re-authorization rather than a soft warning that keeps
  spending.
- **Review gates:** manual admission is the default for everything; the
  envelope only ever narrows a specific, named, low-risk category into
  bounded auto-admission — it never widens Full Auto's authority beyond what
  a human reviewer already has in the existing workroom.

## Acceptance Criteria

- **FA-AC-01:** Full Auto is off by default per repository and requires an
  explicit per-repository toggle. Toggling it on alone never dispatches
  Codex work and never bypasses manual admission; only a separately signed
  FullAutoEnvelope can enable bounded auto-admission.
- **FA-AC-02:** The observe stage reads only the named, bounded signal set
  (README, docs tree, CONTRIBUTING, existing product-spec/assurance-spec
  files, in-scope open issues, recent commit subjects) and produces a typed,
  digest-pinned inventory reusing the AssuranceSpec RepositoryInventorySchema
  shape. Any additional source requires an explicit spec revision, not a
  silent scope expansion.
- **FA-AC-03:** Every claim in an InferredRepoIntent carries an exact source
  citation (file path/line range or issue number). An inference containing an
  uncited claim is rejected before it reaches the draft stage.
- **FA-AC-04:** The infer stage runs through a central typed semantic selector
  or embedding/LLM synthesis path. No keyword, regex, or substring matching
  over repository text may be used to determine inferred intent, mode, or
  routing at any point in this pipeline.
- **FA-AC-05:** Every drafted ProductSpec proposal validates cleanly under the
  `openagents` profile before it is surfaced to a human or an admission path,
  uses structured `AC-<n>`/`SM-<n>` IDs from its first revision, and would
  also validate under the stricter `upstream` profile.
- **FA-AC-06:** Every drafted AssuranceSpec proposal binds to the exact
  ProductSpec `document_digest`, `spec_revision`, and criterion refs it was
  compiled from, and every `AssuranceAuthoritySchema` field remains at its
  structural `false` default; no code path in this feature may set any of
  those fields to `true`.
- **FA-AC-07:** No proposed pair reaches Codex dispatch without passing the
  admission gate. Manual admission behaves identically to hand-authored spec
  admission today. A signed `FullAutoEnvelope`'s auto-admit path only ever
  applies to the exact named low-risk category and non-excluded paths it
  declares; any packet touching an excluded path (auth, payments, secrets,
  credentials, schema/migration, public promise copy) always falls back to
  manual admission with no override.
- **FA-AC-08:** Admitted work dispatches exclusively through the existing
  product-spec-workroom packet lifecycle and the existing Pylon/Codex
  `codex_agent_task` contract (or, once sequenced behind CAP-10 or an
  equivalent reviewed surface, OpenAgents-owned Cloud placement for
  unattended runs). No new dispatch protocol, execution engine, or provider
  path is introduced.
- **FA-AC-09:** Auto-disposition (skipping human accept/waive) is reachable
  only inside the signed envelope's lowest-risk category and only when every
  relevant obligation reports a `CONFIRMED` verification receipt with
  `verifierRef !== producerRef`. Every other packet always stops for human
  disposition, and a false-completion incident (workroom or Full Auto
  reporting complete without a matching terminal outcome and review
  post-image) is a release-blocking defect with target `0` confirmed
  incidents.
- **FA-AC-10:** A global kill switch halts new admission and new dispatch
  immediately across every repository for that owner; already-dispatched
  packets either reach a clean terminal state or an explicit stopped state,
  never a silent continuation.
- **FA-AC-11:** Every stage transition is recorded as a durable, owner-visible
  receipt. An owner can reconstruct, for any loop iteration, exactly what was
  observed, what was inferred and why (with citations), what was drafted,
  how it was admitted or rejected, what was dispatched, and how it was
  verified and disposed.
- **FA-AC-12:** Exceeding any budget (spend, wall-clock, or packet count) for
  a loop iteration always halts that iteration and requires explicit owner
  re-authorization before the next tick; it never degrades to a warning that
  allows continued spending.
- **FA-AC-13:** No Full Auto packet performs a direct unreviewed commit,
  merge, force-push, or history rewrite. Dispatched agents open proposals for
  human integration exactly as the existing MVP rollout boundary already
  requires.
- **FA-AC-14:** Full Auto never runs against a repository the owner has not
  explicitly granted, and never uses signal from one granted repository to
  propose work in another.
- **FA-AC-15:** Adopting this spec, by itself, changes no promise-registry
  entry's disposition. The `autopilot.desktop_full_auto_guidance.v1` entry
  remains exactly as recorded until a separate, explicit registry pass links
  real receipts produced under this design.

## Success Metrics

```productspec-success-metrics
- id: full_auto_inference_citation_completeness
  metric: drafted_inferred_repo_intents_with_every_claim_carrying_an_exact_source_citation
  target: "100%"
  window: release acceptance and first 30 days of any pilot repository
  segment: all Full Auto observe/infer cycles on granted pilot repositories
  source: consented_public_safe_full_auto_stage_receipts
- id: full_auto_manual_admission_default_integrity
  metric: dispatched_packets_that_passed_through_either_manual_admission_or_an_explicitly_signed_envelope_category
  target: "100%"
  window: release acceptance and ongoing
  segment: all Full Auto dispatched packets across pilot repositories
  source: consented_public_safe_admission_gate_receipts
- id: full_auto_excluded_path_containment
  metric: auto_admitted_packets_that_touched_an_envelope_excluded_path
  target: "0"
  window: release acceptance and ongoing
  segment: envelope-governed auto-admission attempts
  source: consented_public_safe_envelope_enforcement_receipts
- id: full_auto_false_completion
  metric: confirmed_incidents_where_full_auto_reported_a_packet_complete_without_a_matching_terminal_outcome_and_review_post_image
  target: "0"
  window: release acceptance and first 30 days of any pilot repository
  segment: all Full Auto packets with consented diagnostic receipts
  source: acceptance_exception_register_and_public_safe_support_receipts
- id: full_auto_budget_containment
  metric: loop_iterations_that_exceeded_a_signed_envelope_budget_without_halting_and_requiring_reauthorization
  target: "0"
  window: release acceptance and ongoing
  segment: envelope-governed loop iterations
  source: private_ref_only_budget_ledger
- id: full_auto_proposal_usefulness
  metric: owner_reviewed_full_auto_proposals_accepted_or_edited_and_accepted_rather_than_rejected_outright
  target: ">= 50%"
  window: first 30 days of invited pilot use
  segment: opted-in pilot repositories with at least one completed observe/infer/draft cycle
  source: consented_public_safe_admission_disposition_receipts
```

## Risks

- Misinferred intent can produce a plausible-looking but wrong ProductSpec.
  Mandatory per-claim citation and mandatory manual review by default are the
  primary mitigations; confidence scoring alone is not sufficient because a
  confident wrong inference is the dangerous case, not a hedged one.
- An auto-admit envelope can accumulate scope creep one small widening at a
  time until it resembles a blanket autopilot switch. The excluded-path list
  (auth, payments, secrets, schema/migration, public promise copy) is shipped
  as a floor the envelope itself cannot remove, specifically to prevent this.
- A loop that ticks too aggressively risks becoming a spend or noise problem
  even when every individual admission is well-scoped. Budgets are per
  iteration and hard-stop rather than advisory, and the cadence is a
  scheduled tick, not a tight spin loop, by design.
- Reusing the name "Full Auto" carries reputational baggage: it is the exact
  phrase already recorded as a red, unbuilt promise
  (`autopilot.desktop_full_auto_guidance.v1`) tied to a now-deleted
  application. This spec must not be read as, and must not be used to
  justify, flipping that entry green; see Prior Art and Promise Links below.
- Auto-generated ProductSpecs could quietly become a second source of product
  intent that drifts from `docs/sol/MASTER_ROADMAP.md` sequencing authority.
  Full Auto proposals are explicitly scoped as candidate work for one granted
  repository's own backlog, not a parallel roadmap; they do not reorder or
  override Sol's sequencing.
- Skill or inference prose could, if implemented carelessly, be given implicit
  authority to admit its own output. The schema-level `false` defaults on
  `AssuranceAuthoritySchema` and the explicit, separate admission gate exist
  specifically so this cannot happen by omission.
- Running the unattended Cloud-routed dispatch variant before CAP-10 or an
  equivalent reviewed background-execution surface lands risks reintroducing
  unsandboxed or unreviewed remote execution ahead of its own gate. This spec
  sequences that variant explicitly behind that work rather than building
  around it.
- A repository owner could be misled into granting broader access than they
  intended by an aggressively-worded Full Auto pitch. Grant scope, the
  toggle, and the envelope must each be named and confirmed as separate,
  legible actions, never bundled into one broad consent click.

## Open Questions

- Should the observe stage additionally weigh a repository's own
  `docs/sol`-equivalent roadmap file (if one exists) as a stronger intent
  signal than README/issues alone, or does that risk Full Auto anchoring on
  stale roadmap language over live issue activity?
- What is the right default cadence for the loop tick — fixed-interval,
  webhook-triggered on push/issue events, or owner-configured per repository?
- Should a rejected proposal's citations and confidence be fed back into the
  next observe/infer cycle to avoid repeating the same rejected framing, and
  if so, how is that done without letting rejected intent quietly re-enter
  through a reworded proposal?
- Is documentation/test-only the correct first auto-admit category, or should
  the initial shipped envelope default to zero auto-admit categories until a
  pilot owner explicitly adds one?
- What evidence from Phase 0/1 piloting would justify proposing an update to
  the `autopilot.desktop_full_auto_guidance.v1` promise-registry entry, and
  who owns drafting that update?
- Should Full Auto's generated ProductSpecs live beside the granted
  repository's own spec tree (following `specs/CONVENTIONS.md`) or in a
  clearly labeled `full-auto/` subpath so generated and hand-authored specs
  remain visually distinct at a glance?
- How should Full Auto behave when a granted repository already has an
  actively admitted ProductSpec whose intent the inference contradicts —
  always defer to the existing admitted spec's authority, or surface the
  contradiction as a distinct reviewable finding?

## Rollout

### Phased pilot, not a single launch

- **Phase 0 — Shadow (observe and draft only, zero dispatch):** run the
  observe/infer/draft stages against one owner-selected pilot repository.
  Every proposed ProductSpec/AssuranceSpec pair is written to a review
  location and inspected offline by the owner; nothing reaches the admission
  gate and nothing dispatches. This phase exists purely to validate citation
  completeness, inference quality, and validator cleanliness before any real
  work is at stake.
- **Phase 1 — Manual admission, real dispatch:** proposals reach the actual
  Desktop workroom review surface and can be manually admitted like any
  hand-authored spec. No envelope exists yet; every admission is a human
  decision. This phase proves the full observe-to-disposition loop end to end
  with a human at every gate.
- **Phase 2 — Bounded auto-admit envelope, narrowest category only:** the
  owner signs a `FullAutoEnvelope` scoped to documentation and test-only
  packets on the same pilot repository, with a conservative spend/packet
  ceiling. This phase proves the envelope's exclusions actually hold under
  real inferred proposals before any broader category is considered.
- **Phase 3 — Broader rollout:** any additional repository, auto-admit
  category, or budget increase requires a fresh, explicit owner decision
  informed by Phase 0–2 receipts, not an automatic graduation.

### Relationship to current infrastructure

The Desktop-attended dispatch path (Phase 1–2) requires nothing new from the
Codex app-server parity program and can proceed against the existing
`codex_agent_task` contract today. The Cloud-routed unattended dispatch
variant described in Stage 5 is explicitly sequenced behind CAP-10 (#8842) or
an equivalent reviewed background/remote-execution surface landing in the
open Codex parity epic (#8831); this spec does not propose building a
separate unattended-execution path ahead of that gate.

### Issue and evidence cadence

If this proposal is adopted, it should be tracked the same way MVP-01 was:
one parent issue as the claim ledger and evidence index, with the accepted
plan (this spec plus its companion AssuranceSpec, once both are actually
authored and admitted rather than proposed) holding future work rather than
one speculative issue per criterion.

## Risks Addendum: honest scope of this document

This document is a proposal authored in the `docs/fable/` strategy lane. It is
not an admitted plan of record, it has not been reviewed by the owner, and no
code exists for any stage described above. Per this repository's own
convention, `specs/` (or, for the MVP's single co-located exception,
`docs/mvp/`) is the home for an actually-adopted ProductSpec that participates
in the `pnpm run test:product-spec` validation sweep; if this proposal is
adopted, its ProductSpec and AssuranceSpec artifacts should be authored fresh
under `specs/` following `specs/CONVENTIONS.md`, validated with the existing
CLI, and this `docs/fable/` document should be updated to point at them rather
than duplicated.

## Owner Gates

- Approve or reject reusing the name "Full Auto" for this design, given its
  exact collision with the red, unbuilt `autopilot.desktop_full_auto_guidance.v1`
  promise-registry entry tied to the deleted `apps/autopilot-desktop`.
- Select the pilot repository (or repositories) for Phase 0–2 and confirm the
  exact grant scope for each.
- Approve the shipped default `FullAutoEnvelope` exclusion list (auth,
  payments, secrets, credentials, schema/migration, public promise copy) and
  the initial auto-admit category (proposed default: documentation and
  test-only) before any Phase 2 pilot begins.
- Approve the telemetry/consent copy for the success metrics above before any
  post-pilot metric collection; metrics remain absent rather than inferred
  when consent is off, matching the MVP's own telemetry posture.
- Decide whether the Cloud-routed unattended dispatch variant requires its
  own separate owner sign-off once CAP-10 or an equivalent surface lands, or
  whether this spec's Phase 3 gate already covers it.
- Approve any future update to the `autopilot.desktop_full_auto_guidance.v1`
  registry entry; this spec proposes no change to that entry on its own.

## Receipts

- Per-loop-iteration stage receipts: observe (inventory digest, exact signal
  set read), infer (InferredRepoIntent with citations and confidence), draft
  (validator pass/fail for both the ProductSpec and AssuranceSpec proposal),
  admit (manual or envelope-category disposition, with the exact envelope
  version referenced), dispatch (packet refs and dispatch contract used),
  evidence/verify (receipt refs, `verifierRef`/`producerRef` pair), and
  dispose (accepted/waived/auto, with the auto path's satisfied category
  named explicitly).
- Deterministic rejection receipts for: an uncited inference claim, a
  validator-failing draft, a proposal touching an envelope-excluded path
  under an active envelope, a budget-exceeded iteration, and a kill-switch
  invocation mid-loop.
- A bounded exception register, matching the MVP's own convention, that
  distinguishes design-only, fixture-proven, pilot-proven, owner-accepted,
  and closed for each acceptance criterion above.

## Promise Links

Adopting this spec changes no promise-registry entry. The existing red
`autopilot.desktop_full_auto_guidance.v1` entry remains red, and this document
does not authorize any public claim about a "full auto" capability existing,
working, or being available. If Phase 0–2 piloting under this design produces
real, reviewed receipts, a separate, explicit registry pass may then propose
updating that entry — quoting its own stated review bar back at itself:
autonomy policy, stop controls, logging, budgets, and review gates, each of
which this spec's Solution section names and assigns a concrete mechanism.
Until that separate pass happens, treat this document as design intent only.
