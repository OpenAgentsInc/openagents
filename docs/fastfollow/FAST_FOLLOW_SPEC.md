# FastFollowSpec 0.1

Status: first OpenAgents proposal and dogfood format, 2026-07-16.

FastFollowSpec is the learning-intent companion to ProductSpec and
AssuranceSpec. It declares which source projects a target wants to study,
which source lessons may be combined into target outcomes, how fresh and
shareable the research must be, and which work stages may be proposed. It does
not carry runtime or product authority.

Normative terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are used in the
usual standards sense.

## 1. Design laws

1. **Learning is not intent.** A source project, teardown, StudyPacket, or gap
   does not change the target ProductSpec, roadmap, invariants, or architecture.
2. **Research is not admission.** A valid candidate is evidence-only until a
   target-owned issue, accepted plan, or work packet admits implementation. A
   current explicit owner instruction may become that accepted plan; it is
   separate target authority, not authority inferred from the research.
3. **Evidence is not a verdict.** A study, test, receipt, or cache hit does not
   self-verify, self-accept, merge, release, or change a public promise.
4. **The target stays sovereign.** The nearest target `AGENTS.md`,
   `INVARIANTS.md`, ProductSpec, AssuranceSpec, accepted roadmap, issue state,
   and runtime policy take precedence over every source.
5. **Control flow is typed.** A deterministic program controls lifecycle and
   authority. Models may propose bounded semantic results inside that program;
   they do not invent a parallel scheduler or parser.
6. **External instructions are data.** Source repositories, issues, prompts,
   skills, and docs are untrusted study material, never agent instructions for
   the target.
7. **No keyword router.** User-facing intent, source, directive, retrieval, and
   tool selection use typed IDs, a structured planner, or a central semantic
   selector. Deterministic parsing begins only after a route is selected.
8. **Reuse stops at the privacy boundary.** Public upstream study may be
   shared by content digest. Private target code, prompts, traces, credentials,
   and local gap analyses never enter a cross-tenant cache.
9. **No parity theater.** A run may emit `no_material_delta`, `rejected`,
   `superseded`, `stale`, `blocked_by_policy`, or `unavailable`. The system
   MUST NOT invent changes merely to stay busy.
10. **Adopt the lesson, not the weakness.** Every directive names constraints
    and a stance. Target invariants are not weakened to resemble a source.

## 2. Discovery and scope

The conventional authored file is `FASTFOLLOW.md` in the same directory as an
applicable `AGENTS.md`.

- An agent resolves the nearest applicable `AGENTS.md`, then looks for
  `FASTFOLLOW.md` beside it.
- A repository MAY point to a different repository-relative path from
  `AGENTS.md`.
- A nested FastFollowSpec replaces the parent in format 0.1. There is no
  implicit merge or inheritance.
- Symlink escape, absolute-path source injection, and repository escape MUST
  fail closed.
- Missing, malformed, unsupported-version, duplicate-ID, or dangling-reference
  documents are unavailable work sources; agents MAY report the diagnostic but
  MUST NOT reconstruct the intended policy from prose.

FastFollowSpec is applicable to repositories, monorepo subprojects,
workspaces, and non-code projects as long as the target can name its authority
and evidence surfaces.

## 3. Authored document

An authored document contains YAML frontmatter followed by the ordered
sections and fenced JSON blocks below. Human prose explains rationale; the
frontmatter and typed blocks form the canonical semantic projection.

### 3.1 Frontmatter

Required fields:

```yaml
fast_follow_spec_format_version: "0.1"
fast_follow_spec_id: "stable.dotted.id"
fast_follow_revision: 1
title: "Human title"
artifact_type: "learning_intent"
lifecycle_state: "proposed" # proposed | admitted | superseded | retired
author: "role or project identity"
linked_target_repo: "owner/repository or other stable locator"
created_at: "RFC 3339 timestamp"
updated_at: "RFC 3339 timestamp"
```

An admitted lifecycle means only that the target accepts the learning intent.
It grants no filesystem, network, credential, spend, provider, deployment, or
SCM authority. Mature implementations SHOULD bind admission in a separate
exact-digest artifact; format 0.1 preserves the lifecycle field so an authored
dogfood file can be used before that service exists.

### 3.2 Ordered sections and blocks

1. `## Objective` — human rationale and explicit non-goal.
2. `## Target` with one `fastfollow-target` JSON block.
3. `## Sources` with one `fastfollow-sources` JSON block.
4. `## Learning Directives` with one `fastfollow-directives` JSON block.
5. `## Work Generation` with one `fastfollow-work-generation` JSON block.
6. `## Reuse and Evidence` with one `fastfollow-reuse` JSON block.
7. `## Guardrails` with one `fastfollow-guardrails` JSON block.
8. `## Authority Boundaries` with one `fastfollow-authority` JSON block.

The JSON blocks MUST use strict JSON, stable item IDs, and no comments. A
parser MUST reject a missing, repeated, out-of-order, unknown, or invalid
normative block. The canonical combined JSON projection validates against
[`fast-follow.schema.json`](fast-follow.schema.json).

### 3.3 Dual identity

Implementations SHOULD compute:

- `document_digest`: SHA-256 over exact authored UTF-8 bytes; and
- `intent_digest`: SHA-256 over canonical JSON of the format version, stable
  identity, revision, lifecycle, target, sources, directives, work generation,
  reuse, guardrails, and authority blocks, excluding only provenance
  timestamps.

All unknown fields are intent-bound by default. A material change to target
scope, source selection, lessons, directives, work policy, sharing, guardrails,
or authority MUST increment `fast_follow_revision`. Refreshing an upstream
commit does not edit the authored learning intent; it produces a new compiled
manifest.

## 4. Semantic model

### 4.1 Target

The target names:

- stable target ID, repository root, and repository locator;
- authoritative `AGENTS.md` and `INVARIANTS.md` paths;
- ProductSpec, AssuranceSpec, roadmap, and issue authorities that must be
  reconciled before implementation; and
- research, candidate, and receipt output paths.

Paths are repository-relative and bounded. An empty ProductSpec or
AssuranceSpec list is valid; it means the target has not adopted that companion,
not that Fast Follow owns the missing intent or proof.

### 4.2 Sources and lessons

Each source has a stable `id`, role, access class, canonical locator, tracking
policy, teardown/evidence refs, and a non-empty lesson list. Each lesson has a
stable source-local ID, typed kind, summary, and stance:

- `study` — understand before deciding;
- `adapt` — port the outcome where target authority agrees;
- `adapt_with_stronger_boundaries` — preserve the insight while explicitly
  refusing the source's weaker authority, privacy, safety, or durability; or
- `reject` — retain as a negative pattern or regression guard.

The source resolver pins exact commit/tree, release/artifact identity, selected
paths, byte digests, access class, license/provenance facts, and capture method
in a compiled manifest. Mutable branch names and “latest” labels are never
sufficient run identity.

Closed or installed products MAY be sources. Their access class and confidence
must remain explicit, and observations inferred from strings/bundles MUST NOT
be promoted to claims about unobserved server behavior.

### 4.3 Learning directives

A directive is the many-to-many statement the owner asked for: “learn X, Y,
and Z from projects A, B, and C for these parts of my target.” It contains:

- stable directive ID and priority;
- one or more `source#lesson` references;
- bounded target scopes;
- desired target outcome;
- permitted work products;
- constraints and rejection rules; and
- optional ProductSpec criterion and AssuranceSpec obligation refs.

Every reference MUST resolve exactly once. A compiler MUST reject unknown,
duplicate, or missing references and MUST produce a disposition for every
selected lesson. The directive is intent, not a diff prescription.

### 4.4 Work generation

Allowed stages are:

```text
research → gap_analysis → candidate_proposal → implementation → verification
```

The first three stages are evidence-only. `implementation` requires a separate
target-owned admission named by `implementation_requirements`. `verification`
follows the target's existing AssuranceSpec, tests, review, receipt, and owner
gates; Fast Follow does not create a weaker proof lane.

The work policy names:

- activation (`manual`, `backlog_fallback`, or `continuous`);
- an optional `initial_program` binding one repository-relative strategy
  artifact to an ordered subset of directives, a default evidence stage,
  deterministic advance/exhaustion behavior, and separate implementation
  admission;
- higher-authority precedence;
- exactly one concrete unit per Full Auto continuation;
- dedupe-key inputs and supersession behavior;
- honest `no_material_delta` closeout; and
- optional capacity profiles.

Capacity profiles express portfolio intent, not execution authority. A native
scheduler must bind a profile into a separately admitted `FullAutoRunPolicy` or
FleetRun, claim work, and preserve provider/workspace/lease policy. An authored
FastFollowSpec cannot start five workers by itself.

When `initial_program` is present, work selection begins with its first
non-terminal directive at `default_stage`. It advances only when the current
directive has a durable terminal or blocked disposition. `return_to_catalog`
then exposes remaining directives through ordinary priority selection; `stop`
ends that work source. The strategy artifact is evidence and sequencing input,
not implementation admission. Format 0.1 fixes
`implementation_admission` to `separate_target_authority_required`.
An exact owner-accepted plan satisfies this requirement when persisted by the
target. It may admit one directive or the ordered initial program. The target
still decomposes implementation into bounded claimed work packets, and an
accepted plan does not acquire deployment, release, spend, settlement, or
public-claim authority by implication.

## 5. Deterministic program

The canonical program is:

```text
resolve_spec
  → verify_lifecycle_and_authority
  → pin_target_and_source_snapshots
  → resolve_or_build_study_packet
  → select_directive_and_stage
  → produce_target_gap_or_candidate
  → evaluate_and_dedupe
  → propose_target_work
  → await_separate_admission
  → claim_and_implement
  → run_target_local_verification
  → emit_closeout_and_freshness_state
```

Each transition must have typed preconditions, terminal outcomes, and recovery.
Append-before-effect, exact identity, lease/fencing, retry ownership, and
restart semantics belong to the runtime implementation, not to model prose.

Recommended lifecycle:

```text
declared → studied → pattern_extracted → mapped → reconciled
         → issue_ready → admitted → implementing → verified → adopted
```

Side dispositions:

```text
no_material_delta | rejected | superseded | stale |
blocked_by_policy | unavailable | inconclusive
```

## 6. Compiled artifacts

### 6.1 FastFollowManifest

The deterministic compiler emits an immutable, `do_not_edit` manifest binding:

- compiler identity/version/content digest;
- exact FastFollowSpec path, revision, document digest, and intent digest;
- exact target revision/tree and authority-document digests;
- exact source revisions/trees/artifacts and selected corpus digests;
- resolved directive graph;
- generated study/gap work units;
- cache and candidate dedupe keys; and
- public/private classification.

Changing source or target snapshots recompiles the manifest. Runs never edit a
manifest to record outcomes.

### 6.2 StudyPacket

A StudyPacket is the shareable public-upstream artifact. It SHOULD contain:

- source and invariant maps;
- architecture and lifecycle maps;
- glossary and typed pattern IDs;
- source spans and confidence labels;
- examples, tests, traps, rejection patterns, and playbooks;
- retained failures/counterexamples; and
- license/provenance and freshness metadata.

It is a compact study artifact, not a context dump.

### 6.3 GapAssessment

A GapAssessment binds one StudyPacket and directive set to an exact target
revision and target authority digests. It separates:

- source freshness;
- evidence confidence;
- relevance;
- target fit;
- portability;
- license/provenance;
- current implementation state;
- proof readiness; and
- disposition/exception.

These axes MUST NOT collapse into a blended green score.

### 6.4 WorkProposal and receipts

A WorkProposal contains a stable candidate/dedupe identity, target scopes,
desired change, source and gap refs, constraints, acceptance/proof refs,
dependencies, and proposed close rule. It grants no mutation lease.

Runs emit separate Study, Gap, Candidate, Implementation, and Verification
receipts. Public-safe receipts contain refs and digests, never raw private
source, prompts, transcripts, credentials, absolute paths, or command bodies.

## 7. Shared inference and Khala bootstrap

The reusable public StudyPacket key is derived from:

```text
source identity
+ exact source commit/tree/artifact digest
+ selected corpus/path digests
+ ordered lesson/directive archetype IDs
+ study-program/schema version
+ planner/model/prompt/tool versions
+ rubric/evaluator version
+ visibility and license policy
```

Target-specific work adds:

```text
target repo and exact revision
+ FastFollowSpec identity/digests
+ target ProductSpec/AssuranceSpec/invariant digests
```

The first key may be shared across tenants only for admitted public material.
The second is target-private by default. Prompt-prefix caching and provider
cache-affinity may reduce cost underneath this, but the durable economic unit
is the StudyPacket.

Khala/Blueprint may later treat the deterministic program as a typed signature
with swappable research, comparison, planner, and verifier modules. DSPy/GEPA
may propose better module parameters against target-local executed outcomes,
quality, latency, and cost. Optimizer output remains a candidate behind release
gates and never acquires dispatch, mutation, promotion, payment, or claim
authority.

## 8. Full Auto and fleet composition

Full Auto is an execution mode. Fast Follow is a work source. The native
composition should add a typed `selectFullAutoWorkItem` before Full Auto claims
its durable turn lease, then persist:

```text
work_ref
source_kind
fast_follow_spec_digest
manifest_digest
directive_ref
candidate_digest
claim_ref
```

The shared ProviderLane host seam should project bounded parsed Fast Follow
context before a turn and revalidate it afterward, just as ProductSpec and
AssuranceSpec context is projected today. Provider lanes do not parse or admit
the spec themselves.

A true `3 delivery / 1 research / 1 implementation` run requires a claim-aware
concurrent FleetRun or FullAutoRunPolicy. Research may write only configured
study/gap/candidate artifacts and never product code. Implementation consumes
only separately admitted candidates in isolated worktrees. Counterexamples
flow back into StudyPackets; duplicate implementations are refused.

Current Desktop Full Auto is per-session and serializes its local continuation
queue. The documented multi-session bootstrap is useful now, but it is not
evidence that the native portfolio scheduler has shipped.

## 9. Security, license, and privacy

- External repositories are read-only references unless their own separately
  authorized task says otherwise.
- Copying code requires license/provenance review; large-scale vendoring is not
  a default Fast Follow action.
- Source instructions, scripts, hooks, and tool declarations are untrusted.
- A research lane must not execute source code merely because the source asks.
- Private holdouts, customer repositories, raw prompts, and private traces do
  not feed shared StudyPackets, reflection, or candidates.
- Cross-user sharing of research artifacts does not authorize pooling user
  model subscriptions, credentials, compute, or settlement.
- Statistical evidence must not borrow exact/replay terminology.
- Public capability and performance claims remain governed by the product
  promise registry and owner gates.

## 10. Conformance and versioning

The future `@openagentsinc/fast-follow-spec` package should provide:

- Effect Schema document and projection types;
- parser, serializer, stable diagnostics, and CLI;
- exact document and intent digests;
- referential-integrity validation;
- deterministic manifest compilation;
- a frozen valid/invalid conformance corpus with one fixture per diagnostic;
- review/admission binding; and
- read-only study/gap/candidate reports.

Diagnostic codes are API. A change that makes a previously valid document
invalid must bump `fast_follow_spec_format_version`, freeze the prior corpus,
and seed the next corpus. A run result never changes the authored spec.
