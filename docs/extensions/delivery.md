# Delivery and evaluation

Status: implementation baseline and target acceptance plan, 2026-09-21.
This documentation change ships a specification, not a plugin runtime.

## Implementation baseline

The baseline is OpenAgents revision
`2ddabaaedd93ce085d572ccdea7ebd8c3d18fc0d`. Implementation claims below refer
to that checkout, not to every historical design document or deployed host.

| Component | Baseline | Target work |
| --- | --- | --- |
| Capability registry and probes | Shared `crates/capability`; inert manifest reads; separately approved executable probes; bounded execution and explicit presence states. | Portable qualified identities, package bindings, and wider operation descriptors. |
| Program registry and selection | Five local programs; `openagents.program.v1`; admission and `none` path in the shared turn. | Task-specific discovery, explicit typed entry, broader selection evaluation, and complete dependency resolution. |
| Program execution | `query`, `check`, `decide`, and `delegate`; grants and bounded fan-out. | Typed dataflow, child `program` steps, Wasm `module` steps, and complete recovery. |
| Run-state storage | Standalone `coder::runstate` store with pinned identities, append-only records, retained worktree references, and unknown recovery marks. | Wire runtime transitions and reservations to the store; implement reconciliation and supported resume behavior. |
| Verification | Protected suite and artifact plans; `run-suite` and `review-changes` paths with bounded typed verdicts/findings. | Bind extension-produced evidence to those checks; do not give plugins verification authority. |
| Project work | Claims, dependency/conflict checks, source references, and isolated writing delegation. | Shared evidence/context bindings, background allowances, and cross-program scheduling. |
| Evidence and context | Repository evidence paths and ATIF traces; the general store/context builder remains proposed. | Versioned observations, derivatives, recipient-specific manifests, expansion, and reuse. |
| Wasm plugins | No OpenAgents guest host or PDK; runtime refuses `module`. | Validated ABI, host, PDK, fixtures, receipts, and bounded roles. |
| Skills and discovery | Repository skills provide agent guidance; the proposed structured runtime is not implemented. | Digested descriptors, scoped activation, supported hooks, and measured progressive selection. |
| Distribution | Local registry files and NIP definitions. | Package schema, locks, atomic lifecycle, explicit updates, and eventually public catalog semantics. |

Keep these distinctions in interface copy and release notes. Recognizing a
step kind is not implementing it. A declared schema is not tested enforcement.
An existing project ledger or standalone state store is not complete program
recovery. A protected verification path need not be available from an ordinary
chat invocation.

## Migration from the reference Coder system

The source repository contains a substantial plugin design and implementation
history that OpenAgents previously referenced only in part. This specification
reimplements its design in prose for this repository. It does not copy private
backend code, prompts, service endpoints, or credentials. Reference paths in
the following table identify the source material under `~/work/coder` at
revision `559b59983f894b9934b9dfff4519a15be283e72d`; they are provenance, not
runtime dependencies or instructions for this repository.

| Reference document | Retained design | OpenAgents change |
| --- | --- | --- |
| `docs/GLOSSARY.md` | Separate plugins, skills, programs, signatures, capabilities, hosts, guests, and distribution records. | Programs remain NIP-PRG workflows; use operation descriptors for discovery and extension packages for distribution. |
| `docs/plugins/plugin-mvp.md` | Typed packets, manifests, build freshness, fixtures, shared command service, receipts, fallback, and repair comparisons. | Bind guest input/output to shared evidence and program steps; publish a supported ABI before admitting modules. |
| `docs/plugins/terminal-plugins.md` | Host-driven preparation, output processors, callable operations, per-run selection, and observable failures. | Share behavior across terminal/headless; select context per task; preserve raw evidence and bounded expansion. |
| `docs/plugin-catalog.md` | Listings versus immutable releases, ownership, publication states, pinning, updates, revocation, and offline policy. | Generalize to component packages; local first, explicit public distribution design, and existing Nostr contracts where applicable. |
| `docs/plugin-suite.md` | Artifact-pinned complete-task comparisons, invocation records, suite-on/add-one/leave-one-out measurement, and opt-out. | Separate availability from activation; use workload-specific acceptance and uncertainty rather than a universal zero-regression rule. |
| `docs/plugins/2026-09-06-plugin-authoring-cli-audit.md` | One bounded authoring service behind human and model-facing tools, lifecycle reliability, and reviewable publication. | Treat the dated gaps as history; retain behavioral requirements and define new schemas without copying private service contracts. |
| `docs/plugins/2026-09-10-skill-loader.md` | Progressive bodies, revision checks, and allowed-tool constraints that only narrow authority. | Structured applicability, evidence requirements, and bounded host hooks; preserve mandatory instruction scope. |
| `docs/public/plugins.md` and `plugins/README.md` | Author-facing explanation, PDK, examples, and a broad candidate library. | New public documentation must describe actually shipped commands and profiles, not imply that reference packages are ported. |
| `docs/storage-cleanup-plugin.md` | A pure guest can classify an inventory while a native service rechecks and performs effects. | Use the same separation for patch/test/cleanup proposals; do not move native writes or deletion authority into Wasm. |

Some source documents reflect different delivery dates, bundled-versus-managed
installation, or historical gates. Carry over the invariant, not every old
mechanism. In particular, a prepared package is not published, a catalog
release is not enabled, and installation does not authorize invocation.

The source suite's September 11 `repo_context` result is a useful warning:
it was declared in all 12 held-out candidate attempts and invoked in none,
while adding 2,307 JSON bytes to each request. That establishes non-use and
declaration overhead for that experiment. It does not prove the same overhead
or outcome for OpenAgents, nor isolate the cause of all between-arm variance.
The reference default suite remained empty; do not describe the inherited
plugin library as an already proven default experience.

## Delivery sequence

Extend the [TypeSafe-native roadmap](../coder/design/typesafe-agent-roadmap.md)
and its existing ownership rather than creating a competing runtime plan.
These increments are ordered by dependency and are not promised dates.

| Increment | Roadmap alignment | Deliverable and completion evidence |
| --- | --- | --- |
| 1. Stable contracts and baseline | Phase 0 | Inventory current consumers, define descriptor and receipt identities, preserve ordinary `none` behavior, and pin complete-task baseline fixtures. No new semantic gate merely to use a model. |
| 2. Evidence-backed native operations | Phases 1–2 | Store bounded captures and source versions, derive useful deterministic representations, expand originals, and complete a native repair with protected verification. This is useful before Wasm exists. |
| 3. Local plugin host and authoring | Phases 1–3 | Version and test the packet ABI; ship pure mode, then explicitly granted snapshot reads; one diagnostic/outline example, PDK, fixtures, build provenance, receipts, and atomic local lifecycle. |
| 4. Progressive operations and skills | Phase 3 | Descriptor retrieval, scoped schemas/guidance, relevance/sufficiency functions, deterministic controls, and context inspection. Evaluate against simpler policies before default activation. |
| 5. Shared parallel/background work | Phase 4 | Snapshot reuse, recipient-specific context, conflict admission, isolated writes, duplicate-work control, expiration, cancellation, and foreground budget protection. |
| 6. Durable portable programs | Phase 5 | Versioned typed bindings, child/module composition, narrowed shared budgets, complete dependency locks, interruption recovery, and replay from recorded evidence. |
| 7. Distribution and measured defaults | Phase 5 | Ratified publication/identity/revocation contracts, end-to-end package lifecycle, explicit updates, workload evaluation, and independently disableable default components. |

Programs can use native operations while the plugin host is developed. The
first useful increment does not need a public marketplace. Binary ABI,
portable package encoding, typed program bindings, and remote catalog event
semantics are explicit contract deliverables; implementers must resolve them
with schemas and conformance vectors, not informal extra JSON fields.

## Acceptance for implementations

These are future component acceptance criteria. They do not require running
Rust or infrastructure gates for this documentation-only specification.
Use [repository verification guidance](../verification.md) for each actual
implementation change.

### Authority and compatibility

- An installed or selected component cannot widen a permit, program grant,
  probe approval, recipient policy, or parent budget. A semantic high-confidence
  answer cannot bypass a mechanical refusal.
- Unknown schema/ABI/import/step versions, unsupported required effects,
  incompatible dependencies, and cycles refuse before the first effect.
- Explicit workflow requests skip redundant intent inference while preserving
  admission. Ordinary questions, quoted commands, and bullet lists do not
  accidentally activate a workflow. `none`, refusal, and unavailability remain
  distinct in records and user-visible outcomes.
- Changed manifests, question sets, selector wording, source versions, policy,
  or dependencies cannot reuse evidence under an old identity.

### Guest and evidence boundary

- Malformed packets, pointers, lengths, schemas, output, and source references
  cannot cross the host boundary. Check limits before allocation.
- Trap, time, memory, fuel, blocked import, oversized output, and cancellation
  fixtures terminate under the stated bound and preserve attributable outcomes.
- Filesystem fixtures cover traversal, symlinks, changed snapshots, handle
  leakage, and unavailable grants. Pure guests cannot reach ambient resources.
- Optional transformation failure preserves bounded original evidence and
  records fallback. Required-step failure remains a program failure/refusal.
- A derivative carries capture incompleteness and source identity. A compact
  view cannot hide required failures or certify its own sufficiency.

### Lifecycle and recovery

- Changed build input, hidden configuration, lockfile, local dependency, or
  source mutation during build invalidates freshness. Fixtures bind the tested
  artifact, and a failed build cannot leave a publishable stale success.
- Interrupted/concurrent installation keeps one complete lock. Update and
  rollback preserve pinned runs and recheck authority. Uninstall cleanup
  failure cannot reactivate the package.
- Missing/stale revocation information follows explicit policy; revocation
  prevents new dispatch and preserves active-work uncertainty where relevant.
- Child runs share reservations, enforce depth/concurrency, and cannot reset
  the parent's budget. Cancellation propagates with explicit unknown outcomes.
- Recovery does not automatically repeat an ambiguous effect. Verification
  and integration remain distinct from execution completion.

### Surfaces and observability

- Terminal, headless, CLI, and model-facing operations call the same services
  with the same authorization, results, and failure semantics.
- A record can explain requested workflow, available candidates, selection,
  supplied context, invoked artifacts, fallback, verification, and known costs.
- Mandatory instructions survive context rebuilds. Optional skill bodies and
  hooks expire correctly. Background results show stale sources and do not
  silently become accepted changes.
- Payload retention and redaction respect task/tenant/disclosure scope; local
  replay can identify missing retained evidence without executing anything.

## Evaluation and default admission

Evaluate complete workflows, not only guest fixtures or compressed byte
counts. Keep conformance, authority enforcement, semantic function quality,
task correctness, and product efficiency as separate evidence dimensions.

For each candidate, record the exact application binary/configuration,
program/dependency lock, plugin/descriptor/schema digests, question sets,
model/artifact identities, consuming policy, workload, source snapshots,
environment, and available provider usage. Separate generator changes from
plugin changes so an apparent improvement has an interpretable cause.

Use four controls where applicable:

1. The current agent without the candidate.
2. A deterministic native selection/transformation policy.
3. The candidate with typed selection or context decisions.
4. An ablation removing the relevant decision or component while preserving
   the rest of the candidate.

Measure correctness and verification first, then complete-task input/output
usage, decision overhead, calls, retries, elapsed time, storage, known spend,
and foreground interference. Report unknown charges instead of treating them
as free. Component microbenchmarks explain overhead; they do not prove task
savings. Report results per workload as well as aggregate distributions and
uncertainty. Distinguish model/provider variance from component behavior.

Record a funnel of eligibility, discovery, selection, invocation, successful
output, application to context, and verified task result. Keep uninvoked,
no-op, refused, cancelled, and failed attempts in the task denominator.
An uninvoked candidate cannot demonstrate invocation quality, but it should
not automatically disqualify a workload-specific extension whose proper
behavior on that task is to stay inactive. Measure coverage separately.

Freeze development and held-out workload definitions before promotion. Include
long histories, large catalogs, misleading descriptors, unsupported languages,
changed bases, incomplete captures, local-only policy, stale revocation,
optional-role failure, and tasks where no extension should run. Use repeated
and appropriately interleaved trials. Define acceptable regressions, material
benefit, uncertainty, and workload scope before inspecting the final results;
do not adopt a universal threshold or require every noisy metric to improve.

Default admission applies to an activation policy plus exact component
versions, not merely to a package name. Compare suite on/off, add-one, and
leave-one-out configurations. Re-evaluate when artifacts, descriptors,
questions, policies, or workloads change. Preserve user opt-out across
updates. A useful opt-in extension may remain outside the default suite.

## Design sources in this repository

This specification was developed against the complete
[Coder design directory](../coder/design/) and the retained
[source archive](../coder/thoughts-on-a-typesafe-coding-agent/), including the
PDF, HTML, and six images. The main sources serve different purposes:

- The source proposal identifies opportunities; the analysis and roadmap
  define evidence/context architecture and staged delivery.
- The Decision Router consumer plan and function inventory define decision
  ownership, policies, and current limitations.
- The rebuild plan and dated project snapshot explain implementation history
  and existing work ownership. Historical status rows do not override code.
- The service specification and relay plan define existing transport,
  identity, and deployment boundaries; they do not implement a plugin catalog.
- The [program guide](../programs.md), [glossary](../glossary.md),
  [NIP-PRG](../../nips/openagents/NIP-PRG.md), and
  [NIP-CAP](../../nips/openagents/NIP-CAP.md) supply the adopted program,
  capability, and protocol vocabulary.

The [opportunity analysis](opportunities.md) also links the primary TypeSafe
guidance used for narrow semantic functions and retrieve-then-judge selection.
Keep the original proposal and retained exports unchanged; record subsequent
design decisions in this specification and versioned implementation contracts.
