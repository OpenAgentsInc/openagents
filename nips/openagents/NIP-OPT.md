# NIP-OPT — AI contracts and optimization studies

`draft` `optional` — v1, 2026-09-21. The [shared contracts](contracts.md)
are normative. This specification separates an AI task's meaning from the
implementation selected through search and measurement. It applies to general
agents in any supported domain.

DSPy and GEPA are possible authoring and optimization tools, not required
wire runtimes. This NIP specifies the records needed to exchange their outputs
and assess them alongside hand-authored baselines and other optimizers. It
does not standardize an optimization algorithm, require Python in a host,
or claim that compilation improves a workload.

## Transport, identity, and authority

No new event kind is allocated. Private definitions, studies, candidates,
trial records, and study results use local artifact storage or the shared
encrypted `3188` envelope. Its authentication, recipient ACL, COUNT/search,
size, and retention rules apply. Public reusable components use
[EXT](NIP-EXT.md); evaluation publications use [EVAL](NIP-EVAL.md). Never put
private examples or trace-derived prompts in the public extension catalog.

All bodies below have required `v`, `requires`, and optional inert `meta`.
Unless stated otherwise, fields listed for a body are required. ArtifactRef,
DefinitionRef, SchemaRef, bounds, effects, IDs, and refusal codes use the shared
contracts. Schemas and all functional dependencies are immutable and pinned.
An unavailable artifact refuses; a mutable provider alias is not a weight pin.

An envelope proves who declared a record. Consumers separately trust the
study owner's experiment authority, evaluator's measurements, compiler's
provenance, and operator's deployment authority. One identity MAY fill several
roles, but MUST disclose that relationship. It cannot claim independent review
of itself. Neither a score nor an optimizer's signature grants execution.

## Semantic AI signatures

An AI signature defines the intended behavior, not cryptographic event signing
and not a selectable operation descriptor. A definition has
`v: "openagents.ai-signature.v1"` and:

| Field | Contract |
| --- | --- |
| `id` | Qualified component ID. |
| `input`, `output` | SchemaRefs defining semantic field names and types. |
| `task` | ArtifactRef to the human-readable task meaning, output interpretation, abstention semantics, and known exclusions. |
| `invariants` | DefinitionRef to a registered contract checker/policy, including protected information-flow and control constraints. |
| `effects` | Maximum declared effect envelope; no authority grant. |
| `evaluation` | ArtifactRefs for evaluation requirements/rubrics, possibly empty when no quality claim is made. |

Natural-language meaning and examples cannot be replaced by type validation.
Keep formatting instructions, provider tricks, demonstrations, and inference
strategy in the implementation. A study freezes the signature. Changing label
meaning, answer ontology, required evidence, or the task itself creates a new
signature and a new study; it is not an improvement on the unchanged task.
An optimizer may rephrase a model-facing rendering only inside the permitted
search space. The original task remains available to independent evaluation.

Examples include evidence selection, cited answers, document extraction, and
record-update proposals. Noul/Choice/Score are useful implementation contracts
for some signatures, not the required output vocabulary of every AI task.

## Immutable implementations

A definition has `v: "openagents.ai-implementation.v1"`, `id`, `signature`
(DefinitionRef), `entry_kind` (`decision-function`, `program`, or `operation`),
`entry` (DefinitionRef), `target` (ArtifactRef described below), `build`
(ArtifactRef or null), `effects`, and `bounds`.

The entry's input/output SchemaRefs MUST equal the signature's. An adapter
that changes representation must be explicit in the entry's locked closure.
Implementation effects MUST be a subset of the signature's effect envelope,
and the entry's transitive effects MUST fit the implementation declaration.
Both remain subject to narrower host grants and aggregate bounds.
`entry_kind` selects an EXT decision function, PRG program, or EXT
operation binding; it does not embed arbitrary DSPy objects or source code.
All prompts, demonstrations, question sets, state builders, consuming policies,
module topology, generation settings, adapters, and executable bytes that can
affect the entry MUST be reachable through pinned dependencies. No functional
configuration may hide in `meta`, an environment default, or a mutable URL.

The target has `v: "openagents.ai-target.v1"`, `runtime` (DefinitionRef),
`models`, and `environment` (ArtifactRef). Each model entry is
`{role, component, requested, identity, assurance}`. Role is `student`, `judge`,
or `reflection`; component is the exact model-consuming DefinitionRef;
requested is a nonempty model/door identifier; identity is an ArtifactRef
or null; assurance is `artifact`, `provider_revision`, or `alias_only`.
Artifact assurance requires retained model identity evidence; provider-revision
assurance requires a declared provider revision. Alias-only MUST use null
identity and cannot claim immutable weights. Components are unique per role.
Environment identifies required host/runtime and execution configuration.
Actual identities and discrepancies belong in trial receipts, including
provider-reported identity unavailable at compile time.

Build provenance, when present, has `v: "openagents.ai-build.v1"`, `builder`
(DefinitionRef), `inputs` (ArtifactRefs), `configuration` (ArtifactRef),
`outputs` (ArtifactRefs), and `limitations` (ArtifactRef). It records compiler,
exporter, adapter, dependency/toolchain identities, and explicit unknowns.
Outputs identify entry artifacts, not the containing implementation definition:
avoid a self-referential digest. A null build means no build provenance claim.

Implementations are EXT components, installed inertly. PRG `invoke` can name
one under a supported host dispatcher. Admission resolves it to its exact entry
and signature, validates the full closure and protected contract, and applies
CAP/POL requirements. The wrapper adds no grants or new step language. The
entry may use bounded composition or an admitted native operation;
unsupported inference strategies refuse before dispatch.

An exported prompt file alone is not a compiled implementation. A serialized
Python object is not a portable executable, and installation MUST NOT unpickle,
import, build, or run it. Offline tooling can emit supported definitions and
inert assets; another execution environment requires an explicitly admitted
capability with its own evidence and limitations. Pure/snapshot-read Wasm
profiles remain unable to call a model. Fine-tuned artifacts require separately
authorized training and admitted model-serving contracts.

## Study plan and permitted search

A study has `v: "openagents.optimization-study.v1"` and:

| Field | Contract |
| --- | --- |
| `study` | Random common ID, stable for this frozen plan. |
| `owner` | Pubkey or locally trusted provenance ID. |
| `signature`, `baseline` | Exact signature and AI implementation DefinitionRefs. Baseline realizes that signature. |
| `space` | ArtifactRef to the search-space definition below. |
| `data` | ArtifactRef to the partition/access plan below. |
| `suite` | ArtifactRef to an EVAL suite with frozen labels, metrics, and environment. |
| `objective`, `acceptance` | Registered DefinitionRefs for candidate selection and final acceptance respectively; they can differ and neither is editable by candidates. |
| `optimizer` | `{operation, algorithm, configuration, target}`: DefinitionRef, nonempty algorithm/version string, configuration ArtifactRef, and ai-target ArtifactRef. |
| `bounds` | Aggregate ceilings for the complete study. |
| `disclosure`, `retention` | ArtifactRefs to supported host policies covering every participant and derived artifact. |
| `confirmation` | `{max_candidates, max_attempts, policy}` with positive integers and policy DefinitionRef, frozen before search. |

The optimizer configuration pins actual library/source version, proposal and
selection procedures, stopping conditions, seeds when supported, and declared
sources of nondeterminism. A grid search MUST NOT describe itself as GEPA.
A different optimizer, objective, or search space starts a new linked study;
it does not silently alter an admitted plan.

A search space has `v: "openagents.optimization-space.v1"`, `slots`, and
`validator` (DefinitionRef). Each slot is `{id, component, surface, values}`:
slug, seed component DefinitionRef, surface enum, and SchemaRef for allowed
replacement values. Surfaces are `instructions`, `demonstrations`, `questions`,
`inference`, `model`, `parameters`, `composition`, and `source`. Slots are
distinct and interpreted by the pinned validator; paths or strings are not
executable patch expressions. Candidate construction emits complete new
definitions and locks, not mutable overlays applied after admission.

Search MAY change several slots jointly. Composition/source search MAY alter
the bounded internal decomposition or propose executable code, but MUST pass
the pinned validator and normal build/admission boundary before measurement.
The signature's essential control constraints remain fixed: authority,
mandatory disclosure rules, protected checkers, approval consumption, and
effect confirmation cannot become search variables. Operational policies such
as relevance ranking or escalation recommendations can vary only inside that
boundary. A grader or label correction is a new experiment, not a candidate.

No optimizer is required to search every surface. The search-space validator
and host enforce an explicitly supported subset; unknown semantics refuse.
Record failures to build or materialize a proposal as candidate outcomes.

## Data access and partition isolation

A data plan has `v: "openagents.optimization-data.v1"`, `suite` (ArtifactRef),
`search`, `selection`, `confirmation`, `excluded` (case-ID arrays), `access`
(ArtifactRef to an authorized data-use policy), and `exposure` (ArtifactRef).
IDs MUST be distinct across and within arrays, and their union MUST equal the
suite's case IDs. Search and selection together equal EVAL's `development`;
confirmation equals `held_out`; excluded equals `excluded`.

Search data can feed proposals, demonstrations, reflection, and fitting as
authorized. Selection data ranks candidates and informs adaptive search, so it
is not unseen final evidence. Confirmation stays inaccessible to proposer,
reflection model, compiler, candidate author, and candidate configuration until
the chosen candidate and acceptance policy are committed. The evaluator alone
reads it under a recorded, bounded confirmation allowance. Trial inputs reach
the candidate as normal task inputs, without hidden labels or grader state.

The exposure artifact records known prior access, duplicate/near-duplicate
groups, source overlap, and unknown provenance. Partition by leakage unit
(for example repository, user, document, or time window), not merely by a new
case ID. Unknown training exposure is a limitation, never proof of being unseen.
Confirmation results, including aggregate feedback, spend the allowance. Once
used for further tuning they become development evidence; new confirmation
requires genuinely unexposed cases and a new plan. Relabeling does not reset it.

Data rights apply separately to evaluation, prompt/example optimization,
reflection, weight training, retention, and export. Retaining a trace does not
authorize prompt tuning, even when model weights stay unchanged. Missing
permission refuses that reuse.
Transferred derivatives preserve source restrictions. The host isolates grader
credentials, expected answers, and confirmation storage from candidate code.

The trusted study validator resolves the protected suite/data closure. It gives
each participant only its admitted view through CTX; a reference to a protected
artifact does not authorize fetching it. The proposer need not receive hidden
labels or confirmation contents to participate in a validated study.

## Candidates and actual trial execution

A proposal has `v: "openagents.optimization-proposal.v1"`, `study`
(ArtifactRef), `parents` (candidate ArtifactRefs), `changes` (ArtifactRef to
`{slot, value}` replacements), and `proposer` (DefinitionRef). It records an
untrusted suggestion before validation or build; invalid proposals remain
attributable outcomes and are never executable merely because they are recorded.

A candidate has `v: "openagents.optimization-candidate.v1"`, `study`
(ArtifactRef), `parents` (candidate ArtifactRefs), `implementation`
(DefinitionRef), `lock` (ArtifactRef to the full implementation lock), `changes`
(ArtifactRef to a list of `{slot, value}` replacements), and `proposer`
(DefinitionRef), plus `proposal` (the originating proposal ArtifactRef).
Study, parents, changes, and proposer MUST agree with that proposal.
Empty parents denotes a proposal from the study baseline.
Parents must belong to the same study; cycles refuse. Baseline and candidate
must realize the same signature. Preserve rejected candidates and failures
under the retention policy; absence of a winner is a valid study outcome.
If validation or build cannot produce a complete candidate, record construction
failure in the study result below with no candidate or inference trial. Do not
fabricate an executable identity or count the failure as successful evaluation.

A trial has `v: "openagents.optimization-trial.v1"` and:

| Field | Contract |
| --- | --- |
| `study`, `candidate` | Study ArtifactRef and candidate ArtifactRef or null for baseline. |
| `trial` | Unique random ID; retries preserve this logical trial ID and use RUN attempt identities. Independent repeats get new trial IDs. |
| `phase` | `search`, `selection`, or `confirmation`. |
| `case` | Suite case ID belonging to that phase. |
| `implementation`, `lock` | Actual requested DefinitionRef and full lock ArtifactRef. |
| `materialization` | ArtifactRef below, or null if refused before loading. |
| `runs`, `receipts` | Ordered run, trajectory, and execution receipt ArtifactRefs. |
| `outcome` | Common execution outcome, distinct from evaluation quality. |
| `evaluation` | EVAL report ArtifactRef or null if no evaluation was possible. |
| `usage` | ArtifactRef to the accounting record below. |

Materialization has `v: "openagents.ai-materialization.v1"`, `trial`, `host`
(pubkey or trusted local provenance ID), `implementation` (DefinitionRef),
`lock_digest` (canonical shared lock Digest), `loaded` (ArtifactRefs),
`target` (ai-target ArtifactRef), `bindings` (ArtifactRef), and `verification`
(`passed`, `failed`, or `unverifiable`). Loaded lists the actual functional
artifact closure observed at execution, sorted by digest without duplicates.
Bindings pin adapters, host executable, policy, environment, and admitted
provider recipients. The evaluator MUST compare it to the resolved expected
closure before attributing a candidate result. Execution receipts bind each
actual model call and its served identity to that materialization.

Changing the candidate file while the runtime executes its seed is
`identity_mismatch`, not a measurement of the candidate. A deliberate baseline
retest is labeled baseline. Remote materialization is an attributable host
claim, not remote attestation. If required identity cannot be established,
quality evidence is inconclusive at that assurance level.

Record cache policy and observations, repetition order, environment resets,
random seeds, and provider drift in the suite environment and receipts. Do not
reuse a cached baseline result as a candidate execution or coalesce independent
repeats. A cache hit may satisfy a declared replay study, but must not be
reported as a fresh inference measurement. Trial retries and failures remain
in EVAL denominators under the frozen policy.

## Accounting, results, and promotion

Usage has `v: "openagents.optimization-usage.v1"`, `scope` (study or trial
ID), `entries`, and `complete` (boolean). Entries are
`{category, receipt, calls, wall_ms, spend_microunits, currency}`. Category is
`proposal`, `reflection`, `student`, `judge`, `build`, `tool`, `storage`, or
`cleanup`; receipt is an ArtifactRef; counts/duration/spend are nonnegative
integers or null when unknown; currency is a code or null when spend is null.
Entries account for disjoint charges; a shared receipt is not charged twice.
Parallel elapsed wall time is measured separately from the sum of call time.
Complete is true only when all work in the declared scope is accounted for
and required costs are known; missing charges or unknown cost make it false.

All jobs share study reservations. A metric-call limit alone does not bound
reflection, tools, provider spending, training, or cleanup. Reserve each before
dispatch through normal host accounting; retain unknown charges on interruption.
Unsupported hard ceilings cause `cannot_enforce`, including hosted proposers.

A result has `v: "openagents.optimization-result.v1"`, `study` (ArtifactRef),
`construction`, `candidates`, `trials`, `reports` (the last three are ArtifactRef
arrays), `selected` (candidate
ArtifactRef or null), `usage` (ArtifactRef), `status`, and `limitations`
(ArtifactRef). Status is `completed`, `budget_exhausted`, `cancelled`, `failed`,
or `inconclusive`. Selected MUST belong to candidates and may still fail
confirmation. Results identify complete retained indexes, including negative
outcomes, rather than listing only winning trials. Publication of reduced
indexes declares omissions and never implies full reproducibility.

Construction is an ordered array of `{proposal, candidate, outcome, receipts}`:
proposal ArtifactRef, candidate ArtifactRef or null, common execution outcome,
and receipt ArtifactRefs. Each proposed build/validation attempt has an entry,
including failures that never reach inference. A nonnull candidate MUST belong
to candidates and identify the same proposal. Construction and inference
attempt counts remain separate; both contribute to study cost and limitations.

Final acceptance uses EVAL's independent scoped admission, binding the exact
implementation and reports. Selection is not promotion. Passing a module
metric does not establish whole-agent improvement. Adoption creates a new
EXT release or local installed pin; operator policy controls activation,
canaries, rollback, expiry, and model-change requalification. Active runs keep
their admitted pins. A revoked implementation cannot be restored by rollback.
Neither a background finding nor an optimizer result rewrites a live program.

## Conformance and roles

Required fixtures include wrong candidate actually loaded, missing transitive
assets, signature drift, unsupported strategy, altered labels/grader, leaked
confirmation, duplicate groups across splits, exhausted confirmation allowance,
unrecorded reflection spending, model alias drift, cross-arm cache reuse,
interrupted reservations, forged materialization, incomplete result indexes,
private demonstration publication, and promotion without operator authority.

Host/validator conformance is advertised only for tested roles and supported
surfaces under `nip-opt-v1` in named extensions. Relay support for `3188` proves
envelope behavior only; it does not prove optimization, compilation, or quality.
No optimizer name belongs in numeric `supported_nips`.
