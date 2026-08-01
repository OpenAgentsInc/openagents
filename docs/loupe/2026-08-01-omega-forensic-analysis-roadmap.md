# Omega forensic analysis roadmap

Status: **owner-directed roadmap.** This document sequences product and
research work. It does not itself authorize scanning a third-party target,
publishing a vulnerability claim, contacting a maintainer, spending beyond an
admitted budget, or running an exploit outside an owned lab.

Roadmap revision: 3

Date: 2026-08-01

Primary evidence:

- [`Episode 264 - Running Loupe`](../transcripts/264.md)
- [`Results: Loupe on the pre-fix Coldcard firmware`](2026-08-01-coldcard-prefix-experiment-results.md)
- [`Codex analysis: what should catch the Coldcard class of failure`](2026-08-01-codex-analysis.md)
- [`Coordination, not scanners`](2026-08-01-coordination-not-scanners.md)
- [`DSPy in Effect: Git history audit`](../dspy/2026-07-20-dspy-in-effect-git-history-audit.md)
- [`Blueprint Kernel Boundary`](../../apps/openagents.com/workers/api/src/blueprint/README.md)
- [`Managed agent sandboxes: accepted plan`](../sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md)
- [`OpenAgents managed-sandbox contract`](../cloud/contracts/openagents.managed_sandbox.v1.md)
- [`OpenAgents GCE capacity contract`](../cloud/contracts/openagents.gce_capacity_class.v1.md)
- [`OpenAgents Cloud architecture`](../cloud/ARCHITECTURE.md)
- [`Managed IDE placement`](../ide/2026-07-19-sbx-06-managed-ide-placement.md)
- [`Managed-sandbox GCE runtime`](../cloud/bootstrap/SBX-02-managed-sandbox-runtime.md)
- [`SBX-09 independent admission disposition`](../cloud/2026-07-21-sbx09-independent-admission-disposition.md)
- [`Ascii Box and Optibox teardown`](../teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md)
- [`Managed-sandbox ProductSpec`](../../specs/openagents/managed-agent-sandboxes.product-spec.md)

The first target is the historical, pre-fix Coldcard firmware at
`bcc2c382a324690a2fcf972c0bac3b79bf923f7b`. The first product is an
interactive forensic-analysis lab inside Omega. The longer direction is the
security-invariant and evidence workbench described in the Codex analysis.

---

## 1. Outcome

An operator should be able to open a pinned repository in Omega, select or edit
a forensic prompt program, launch a dependency-complete Loupe-style run on an
OpenAgents Cloud-managed Linux worker, and inspect every finding, causal path,
uncertainty, proof artifact, and comparison without leaving the IDE.

The first benchmark is deliberately concrete:

> Starting from a clean clone of the pre-fix Coldcard commit, the system must
> identify the complete wallet-seed entropy failure in the first prioritized
> scan tranche, explain the cross-repository mechanism with exact source
> references, and distinguish that claim from proof about the final firmware
> artifact.

The required mechanism is:

1. `MICROPY_HW_ENABLE_RNG` is defined as zero in the board configuration.
2. libNgU tests whether the macro exists rather than whether it is enabled.
3. MicroPython compiles the deterministic Yasmarang fallback.
4. the board does not export the global `rng_get()` required by libNgU;
5. `ngu.random.bytes()` supplies wallet-seed material through the fallback;
6. the resulting candidate space can be checked against a cheap public oracle.

Coldcard is the development benchmark, not evidence that the system can find
unknown vulnerabilities. Generalization must be measured on blinded structural
variants, unrelated historical bugs, and clean controls.

---

## 2. What Episode 264 changed

The pre-run prediction said Loupe's per-file design was structurally unable to
find the Coldcard defect. The experiment refuted that prediction.

Loupe assigns one focal file to a session but mounts the whole worktree. When
the libNgU and MicroPython submodules were present, a session assigned to the
Coldcard board header followed the evidence across all three codebases and
reported the full failure. When the submodules were empty, the same file,
model, prompt, and effort missed it.

The first roadmap item is therefore not a new scanner. It is a trustworthy
analysis input:

- materialize required submodules and pinned dependencies;
- record exactly which source and build inputs were present;
- fail closed or mark the run `incomplete` when required inputs are absent;
- never let a productive-looking partial scan imply complete coverage.

Attack-surface ranking, prompt optimization, executable PoCs, and artifact
provenance remain necessary. They come after input completeness because no
prompt can reason over evidence that was never mounted.

---

## 3. Product boundary

Omega is the operator surface. It does not need to run untrusted scanners in
the desktop process.

| Surface | Responsibility |
| --- | --- |
| Omega | Target selection, pinned-revision disclosure, prompt editing, run configuration, progress, finding review, source navigation, comparisons, approvals, and receipts. |
| OpenAgents control plane | Versioned contracts, managed-sandbox admission and lifecycle, benchmark datasets, prompt artifacts, evaluation, optimizer records, release gates, worker dispatch, and durable evidence refs. |
| OpenAgents Cloud Linux worker | One admitted, disposable GCE VM per forensic run for checkout preparation, dependency materialization, Loupe execution, builds, tests, artifact inspection, and later fuzzing. |
| Upstream Loupe | Scanner lifecycle, typed finding tools, verdict-before-patch discipline, deduplication, and generally useful checkout or prompt hooks. |

The macOS Omega application is the console. Bubblewrap scanning remains on
Linux. For now, every forensic run executes on OpenAgents Cloud's admitted
Linux supply. Omega must not run the scanner locally or silently route to a
developer machine, a generic remote Linux host, a contributor Pylon, a fake
provisioner, a Box-owned sandbox, or another cloud. Higher-risk build or
dynamic work uses the same OpenAgents-managed isolation boundary rather than
weakening Loupe's sandbox to make it run on macOS.

The default run is read-only and manual-reporting. The target checkout is
immutable. PoC patches, builds, and generated harnesses live in disposable
scratch storage. A forensic run produces evidence and proposals, not source
mutation or public claims.

### 3.1 Required OpenAgents Cloud placement

The first worker target is deliberately closed:

| Field | Required value |
| --- | --- |
| target class | `openagents_managed` |
| provider | `google_cloud` |
| adapter | `adapter.oa-codex-control.gce.v1` |
| isolation | `gce_vm` |
| profile | `profile.sbx.gce.e2-small.v1` or a later digest-pinned revision admitted specifically for forensics |
| data posture | `openagents_managed_region` |
| network | `network-policy-ref://openagents/managed-sandbox/broker-only-v1` |

Omega requests admission and commands through the native Desktop managed-
sandbox boundary. `ManagedSandboxService`, the Worker broker, Cloud SQL
lifecycle authority, `oa-codex-control`, and the private control bridge remain
authoritative. The renderer receives only public-safe refs and projections. It
never receives a GCP client, raw project or instance identifiers, topology,
provider credentials, control tokens, or a generic shell capability.

Do not implement this by calling the older generic `cloud-gcp` placement lane
directly. That capacity class includes a default fake provisioner and historical
fallback behavior useful for tests and other products. Its GCE lease and
cleanup primitives may be reused only below the managed-sandbox broker, whose
forensic admission requires `providerKind=live_gce`, observed readiness, the
exact target/profile, and typed refusal instead of substitution.

Every placement binds the authenticated owner, tenant, program, work unit,
sandbox, attachment generation, resource generation, target, immutable image
digest, profile digest, lease, budget, capabilities, and idempotency bytes
before the first provider effect. A run can reach `worker_ready` only after the
provisioner observes the exact image and profile, private guest boot, readiness
marker, network posture, guest identity posture, and Linux tool preflight. A
control-plane label, configured job ID, VM existence, fake receipt, or provider
state string is insufficient.

Use one newly provisioned GCE VM per forensic run in the initial program. Do
not add reuse, prewarming, checkpoint, fork, restore, or Firecracker placement
to the critical path. Firecracker becomes eligible only after its exact Linux,
KVM, image, isolation, cost, and cleanup profile passes separate admission; it
is never an automatic fallback. If the exact GCE broker, image, capacity,
budget, or cleanup oracle is unavailable, the run is refused or becomes
`recovery_required`.

The guest keeps broker-only networking. Source and pinned submodules arrive as
an immutable OpenAgents source bundle through a scoped SCM/source-materializer
capability and private artifact path. Until that path is admitted, the run does
not receive temporary Internet egress. Model access is likewise brokered with
short-lived, run- and generation-bound capabilities; a provider key, auth home,
or user-supplied token is never placed in the sandbox environment.

The admitted image exposes one bounded forensic workload driver below the
native broker. It starts the pinned Loupe build or Loupe-style harness, emits
native ordered progress and finding events, supports exact-run interrupt, and
returns content-addressed artifacts. It does not turn the guest into an ambient
long-lived Loupe worker, expose Loupe's control plane publicly, or smuggle a
generic shell through the Box command surface.

The managed-sandbox production flags are currently default-off. The SBX-09
independent disposition admitted the AssuranceSpec revision and independently
observed the native GCE lifecycle, network posture, and zero residue, while its
Box SDK re-run remained incomplete and production enablement remained a
separate owner decision. This roadmap does not turn those flags on or make a
public availability claim. Owner-gated staging is the first integration
environment; release requires the exact currently deployed revisions to pass
the forensic worker gate.

### 3.2 What the Box and Optibox teardown changes

Adopt the useful resource semantics, not the vendor control plane:

- one durable resource identity with explicit generations and idempotent
  commands;
- separate provisioning, guest, filesystem, ingress, process, lease, and
  cleanup truth;
- reconnectable ordered events, explicit interrupt, and structural runtime
  settlement rather than silence-based completion;
- bounded file, command, artifact, and retention surfaces;
- cleanup ownership recorded before provisioning and zero-residue evidence
  after deletion.

Reject Box or Optibox as runtime authority. The Box v1 facade remains an
optional, lossy conformance projection over native OpenAgents contracts. Omega
does not use it to launch forensic work, and the Box SDK remains a development-
only compatibility client. Raw secrets in environment variables, in-memory
production lifecycle state, public ingress, hidden placement changes, and
provider state labels as process truth remain out of scope.

---

## 4. The first Omega experience

Add a **Forensics** workbench to an existing Omega repository context.

### 4.1 Target

The operator sees and confirms:

- repository identity and clone URL;
- exact commit;
- clean, dirty, or externally prepared source state;
- submodule and dependency policy;
- selected scan profile;
- admitted OpenAgents Cloud target, image and profile digests, GCE VM
  isolation, region, custody, lease, and broker-only network policy;
- model, effort, concurrency, time, and token or cost caps.

The run cannot start as `complete` until preflight emits a coverage manifest.
An operator may continue an incomplete research run, but the incomplete state
must remain visible in every result and export.

### 4.2 Prompt lab

The operator can inspect, clone, and edit a versioned forensic prompt artifact.
Editing is structured rather than one undifferentiated string:

- role and threat model;
- vulnerability classes;
- target-specific security invariants;
- evidence and source-reference requirements;
- cross-file and dependency exploration policy;
- uncertainty policy;
- tool policy;
- finding schema;
- hypothesis schema;
- PoC requirements;
- severity policy;
- context and budget policy.

Every artifact has a canonical representation and digest. A result always names
the exact prompt digest, model, parameters, target digest, dependency manifest,
and tool surface that produced it.

### 4.3 Two output lanes

More detailed responses must not weaken Loupe's finding discipline.

**Finding lane.** A typed, falsifiable claim with exact source references, an
impact statement, explicit assumptions, and a PoC or invariant check. Only tool
submission creates a finding.

**Forensic hypothesis lane.** A typed lead that lacks enough evidence to be a
finding. It records the suspected mechanism, missing inputs, the next check,
and the consequence if true. It is visibly `unverified` and cannot be reported
or promoted as a vulnerability.

The hypothesis lane solves a real prompt tension. Loupe correctly suppresses
untestable hardening prose, but cross-build failures may first appear as a
question such as "does wallet seed generation actually reach the certified
hardware source?" That question should survive for investigation without being
misrepresented as a confirmed finding.

### 4.4 Run matrix and comparison

One run is anecdote. Omega should make controlled comparison the normal path.
An operator can vary:

- prompt artifact;
- model family and effort;
- dependency materialization;
- scope and attack-surface ranking;
- random seed where the provider exposes one;
- tool availability;
- static-only versus build-enabled mode.

Each arm receives a fresh managed sandbox and immutable source bundle. Arms do
not share a writable disk, provider session, auth home, environment, or hidden
worker state. Worker image and profile digests are comparison dimensions, not
ambient facts.

The comparison view shows common findings, findings unique to one arm,
differences in causal detail, evidence tier, false positives, latency, token or
cost use, and missing-input declarations. Divergence is a lead, not noise to
average away.

### 4.5 Editor-native review

Each result opens the cited file and symbol, the causal-path graph, the proposed
test diff, verifier verdicts, and receipts. The operator can move between the
finding and every referenced dependency without reconstructing paths from a
database row. Raw hidden model reasoning is not required; the review surface is
the structured evidence map the model was asked to submit.

---

## 5. First benchmark pack: Coldcard

The benchmark is a checked-in manifest plus private or public-safe evidence
refs. It pins source, dependencies, prompts, tools, and scoring before each
evaluation series.

### 5.1 Revisions

| Role | Revision |
| --- | --- |
| Vulnerable target | `bcc2c382a324690a2fcf972c0bac3b79bf923f7b` |
| Fix | `ca72463709f4e3f8964952039d5caf955f566a87` |
| libNgU | `537519a829259622ea6b0334fbafd6cae852852f` |
| MicroPython | `4107246f8a080807b62c3b4838e71e812ea68b6f` |
| ckcc-protocol | `3d1dfa858beb58b8dac37d8c66d7aed2909812f2` |
| mpy-qr | `11347d83f4eb325b10676a4eb8e17deccfe0df44` |

### 5.2 Required arms

1. **Incomplete clone.** Required submodule paths are empty. Preflight must
   produce `incomplete`, identify the missing paths, and deny a comprehensive
   coverage claim before any model call. If the operator continues, results
   remain explicitly incomplete.
2. **Complete vulnerable clone.** Pinned submodules are materialized. The scan
   must identify the full causal chain, not only generic RNG hardening.
3. **Fixed clone.** The same prompt must not report the historical vulnerability
   as present after the fix. It may report a different supported finding.
4. **Structural variants.** Rename the macro and symbol, change directory
   layout, reverse guard shape, and move the fallback into a different pinned
   dependency. These prevent exact-token matching from masquerading as a
   detector.
5. **Clean controls.** Include correct hardware-source selection, an
   intentionally unavailable dependency, and a noncryptographic PRNG used only
   for non-secret behavior.

### 5.3 Scoring

Hard gates:

- admits only the required live OpenAgents Cloud GCE worker target;
- detects incomplete inputs before inference;
- cites the exact source evidence it used;
- names all required causal links for a Coldcard `HIT`;
- labels final-link outcome `not proven` until artifact evidence exists;
- does not reproduce the finding against the fixed control;
- never claims to have inspected an absent dependency;
- stays within the admitted run budget;
- reaches a terminal success state only after native receipts prove worker
  deletion and zero residual compute, firewall, disk, process, scratch, and
  capability grants.

Efficiency metrics become promotion-eligible only when the candidate satisfies
the hard gates. Operational and reliability metrics include every run. Failed,
incomplete, cancelled, budget-exhausted, and missed runs remain in the dataset
with their actual terminal state; they are not removed to make latency or token
numbers look better.

### 5.4 Metrics: faster identification without weaker evidence

"Immediately" must mean the first **qualified identification**, not the first
time model prose happens to mention randomness. A finding qualifies when it:

1. was emitted through the typed finding tool;
2. is later mapped by the frozen evaluator to the benchmark vulnerability or
   independently adjudicated vulnerability identity;
3. contains the required causal links and resolvable source references; and
4. carries the correct input-completeness and evidence-tier claims.

Verification and human acceptance are later milestones. For a blinded target,
the evaluator classifies the finding after the run but uses the original
immutable finding-event timestamp. It must not rewrite the time of discovery.

#### 5.4.1 Canonical milestones

| Marker | Meaning | Collected from |
| --- | --- | --- |
| `T0 request_accepted` | Native forensic command and exact run bytes are durably admitted. | Worker broker command record. |
| `T1 worker_ready` | The exact live GCE guest, image/profile, isolation, network posture, and forensic driver are observed ready. | Managed-sandbox readiness event and receipt. |
| `T2 coverage_ready` | The immutable source bundle is mounted and its coverage manifest is terminal. | Source-materializer and preflight receipt. |
| `T3 analysis_started` | The first discovery turn is structurally accepted. | Native runtime-turn event. |
| `T4 first_hypothesis` | The first typed forensic hypothesis is recorded. | Forensic event stream. |
| `T5 first_qualified_identification` | The earliest typed finding later accepted by the frozen vulnerability oracle. | Finding event plus evaluator adjudication. |
| `T6 first_verified_finding` | Deterministic evidence or an admitted independent verifier confirms the finding. | Evidence or verifier receipt. |
| `T7 first_reviewed_finding` | A human accepts, corrects, or rejects the finding in Omega. | Review decision event. |
| `T8 cleanup_observed` | The sandbox is deleted and the zero-residue oracle passes. | Native cleanup receipt. |

Durations spanning control-plane phases use one server-side clock. Durations
inside the worker use monotonic elapsed values reported in receipts. Do not
subtract unsynchronized guest, browser, and control-plane wall clocks.

#### 5.4.2 Speed and lifecycle metrics

| Metric | Definition | Collection | Omega display | Improve toward |
| --- | --- | --- | --- | --- |
| Provision latency | `T1 - T0`. | Managed-sandbox command, readiness event, and provision receipt. | Run waterfall: queue plus provision segment. | Lower without weaker readiness. |
| Input-readiness latency | `T2 - T1`. | Source-bundle and coverage receipts. | Preflight segment with submodule and dependency drill-down. | Lower while completeness stays green. |
| Time to first hypothesis | `T4 - T3`. Diagnostic only; not a hit. | First typed hypothesis event. | Dashed marker on the analysis timeline. | Lower only when later yield does not regress. |
| Analysis time to identification | `T5 - T3`. The primary scanner-speed metric. | Runtime start, immutable finding event, and frozen evaluator result. | Primary run card and finding detail. | Lower p50 and tail latency. |
| End-to-end time to identification | `T5 - T0`. Includes supply and preflight. | Same events plus broker admission. | Primary comparison column and full waterfall. | Lower without hiding infrastructure time. |
| Time to verification | `T6 - T0` and `T6 - T5`. | Evidence and verifier receipts. | Verification segment beside discovery time. | Lower while verifier independence holds. |
| Time to reviewed finding | `T7 - T0` and active reviewer minutes. | Explicit Omega review-session and decision events, with idle time separate. | Review queue and finding detail. | Lower operator burden. |
| Cleanup latency | `T8 - cleanup_requested`. | Stop, delete, and cleanup receipts. | Final lifecycle segment; red until zero residue. | Lower, with 100% observed cleanup. |
| First-priority-tranche hit | Whether `T5` occurs before the priority tranche closes. | Scheduler tranche event plus qualified finding. | Badge and run-matrix boolean. | Higher hit rate. |
| Work fraction to identification | Focal-file sessions and ranked tranches started/completed through `T5`, divided by the declared scan plan. | Scheduler plan and session events. | Scan-progress marker at the qualified hit. | Lower fraction with equal or better recall. |

Runs with no `T5` are right-censored at their declared time or token budget and
count as misses. They never receive a zero duration and are never omitted from
hit-rate denominators. Compute latency and token distributions with a censor-
aware survival estimator; if censoring makes a percentile unidentified, show
`not_estimable` instead of inventing it. Alongside p50 and p95, display the
sample count, miss/censor count, range, and confidence interval. Three
repetitions are useful as a smoke baseline but cannot support a meaningful p95;
until the pre-registered sample size is reached, show every observation and
label tail statistics provisional.

#### 5.4.3 Detection and evidence-quality metrics

| Metric | Definition | Collection | Omega display | Improve toward |
| --- | --- | --- | --- | --- |
| Qualified hit rate | Runs with at least one `T5` divided by all eligible runs, including misses. | Frozen evaluator over typed findings. | Run-matrix hit percentage with numerator and denominator. | Higher on development and untouched holdout. |
| Finding precision | Unique qualified vulnerability claims divided by all unique adjudicated vulnerability claims. | Frozen evaluator plus deduplicated typed findings. | Submitted/unique/qualified funnel. | Higher without suppressing difficult true findings. |
| Recall at fixed time | Fraction of known benchmark vulnerabilities qualified by a fixed elapsed-time budget. | Finding timestamps and benchmark oracle. | Recall-versus-time curve and named `Hit@time` columns. | Curve moves up and left. |
| Recall at fixed tokens | Fraction qualified before a fixed aggregate token budget. | Finding sequence, provider usage receipts, benchmark oracle. | Recall-versus-token curve and `Hit@tokens`. | Curve moves up and left. |
| Causal-chain coverage | Required causal links supported by valid evidence divided by all frozen links. | Deterministic Coldcard/fixture evaluator. | Per-link checklist and aggregate percentage. | 100% for a benchmark hit. |
| Evidence-reference validity | Resolvable, target-bound source and artifact refs divided by submitted refs. | Tree, symbol, build, and artifact resolvers. | Evidence health badge with broken-ref drill-down. | 100%. |
| Verification rate | Qualified findings with admitted confirming receipts divided by qualified findings. | PoC, invariant, artifact, and verifier receipts. | Finding funnel: submitted → qualified → verified → reviewed. | Higher without circular verification. |
| Clean-control false-positive rate | Clean controls incorrectly receiving a qualified vulnerability claim. | Frozen clean-control evaluator. | Red matrix column and release-gate failure. | Zero. |
| Fixed-control regression rate | Runs that still report the historical issue on the fixed commit. | Fixed-revision evaluator. | Dedicated hard-gate column. | Zero. |
| Duplicate burden | Duplicate submitted findings divided by all submitted findings. | Semantic identity plus deterministic fingerprint. | Collapsed duplicate groups and ratio. | Lower without hiding distinct variants. |
| Calibration | Agreement between `finding`, `hypothesis`, `not_proven`, and later adjudication. | Typed claim state plus evaluator/reviewer decisions. | Calibration table by confidence/evidence tier. | Better calibrated; unsupported certainty decreases. |
| Generalization gap | Development hit rate minus blinded holdout hit rate. | Dataset-split identity and scorecards. | Development/holdout paired bars. | Smaller without training on holdout. |
| Run stability | Agreement of qualified vulnerability identities and causal links across matched repetitions. | Repeated-run groups. | Stability percentage and divergence view. | Higher, while real alternate findings remain visible. |

Finding count is not a success metric. A prompt that emits more duplicates or
weak claims must not appear better. Aggregate by adjudicated vulnerability
identity and preserve every dismissed or duplicate submission for burden
measurement.

For hit, recall, latency, and token-to-identification metrics, an eligible run
is a scheduled repetition of a complete vulnerable or structural-variant arm.
Incomplete-input, fixed, and clean arms contribute their own hard-gate and
false-positive metrics rather than being mislabeled as vulnerability misses.

#### 5.4.4 Token, cost, and work-efficiency metrics

| Metric | Definition | Collection | Omega display | Improve toward |
| --- | --- | --- | --- | --- |
| Tokens to identification | Sum of input, cached-input, and output tokens consumed through `T5` across discovery, verifier, and judge turns. Parallel agents all count. When a provider exposes usage only at turn settlement, count the full usage of every turn started before `T5` and mark the value `upper_bound`. | Provider usage receipts joined to native turn and finding sequences. | Primary efficiency column with role and exactness breakdown. | Lower while hit and quality gates hold. |
| Total run tokens | All provider tokens through structural settlement, grouped by discovery, verification, judging, and failed/retried turns. | Provider usage receipts. | Live budget meter and stacked bar. | Lower for equivalent or better evidence. |
| Tokens per unique qualified finding | Total run tokens divided by unique qualified vulnerability identities. Misses show the full spent budget and zero yield, not infinity hidden from charts. | Usage receipts plus adjudicated dedup groups. | Efficiency table and trend. | Lower with nonzero qualified yield. |
| Post-identification tokens | Tokens after `T5`, split into verification, additional discovery, and unproductive work. Turn-level-only usage is marked estimated rather than split as exact. | Turn sequence relative to finding event. | Finding timeline and optimization drill-down. | Remove unproductive tail, retain useful verification/coverage. |
| Cache utilization | Cached input tokens divided by cache-eligible input tokens, with provider exactness. | Provider usage receipts. | Cache segment in token bar. | Higher when it reduces cost without stale context. |
| Cost to identification | Provider cost plus measured GCE incremental cost through `T5`. | Provider billing/usage refs and managed-sandbox cost receipts. | USD-micro cost card and matrix column. | Lower subject to quality gates. |
| Total run cost | All provider, GCE, artifact, and evaluator cost through `T8`. | Native usage, infrastructure, and artifact receipts. | Budget meter and cost waterfall. | Lower with cleanup complete. |
| Tool work to identification | Tool calls, files opened, dependency boundaries crossed, bytes read, builds, and verifier actions through `T5`. | Bounded forensic-driver tool events. | Evidence-path summary and tool waterfall. | Fewer irrelevant actions, not fewer required dependencies. |
| Concurrency amplification | Sum of active agent milliseconds divided by analysis wall time, shown beside peak concurrency. | Turn start/settlement events. | Parallelism card. | Use deliberately; lower wall time without uncontrolled token growth. |
| Worker utilization | Measured running CPU time and memory high-water mark relative to lease time/profile. | GCE and forensic-driver receipts. | Infrastructure detail. | Right-size the admitted profile. |

Usage values retain `exact | estimated | unavailable`. Unknown usage is not
coerced to zero. Raw token counts are comparable only within the same provider
tokenizer and model revision; cross-model comparisons emphasize qualified hit
rate, elapsed time, measured cost, and workload-normalized results. Retries,
abandoned turns, verifier calls, and parallel losing arms remain charged to the
candidate that caused them. Provider-specific reasoning, cache-read, and cache-
write tokens stay separately labeled when exposed rather than being silently
mixed into a supposedly portable total.

#### 5.4.5 Reliability and human-load metrics

| Metric | Definition and collection | Omega display | Improve toward |
| --- | --- | --- | --- |
| Admission and run success | Requested runs reaching `worker_ready`, structural settlement, and `T8`, with refusal reasons separated. Native lifecycle receipts. | Fleet health funnel. | Higher without accepting weaker targets. |
| Cancellation latency | Cancel intent to runtime interruption, then to `T8`. Command, interrupt, and cleanup events. | Cancellation timeline. | Lower and reliably bounded. |
| Cleanup success | Runs with observed zero residue divided by all provisioned runs. Cleanup oracle. | Global red/green SLO and per-run receipt link. | 100%; anything else is `recovery_required`. |
| Budget compliance | Runs staying within token, time, cost, network, and artifact caps. Budget receipts. | Budget bars and hard-gate status. | 100%. |
| Reviewer minutes per qualified finding | Explicit review-session intervals divided by qualified findings; no keystroke or ambient focus surveillance. Omega review events. | Reviewer workload dashboard. | Lower without rubber-stamping. |
| Correction and rejection burden | Qualified findings materially corrected or rejected by reviewers. Review decision reasons. | Funnel and reason histogram. | Lower while difficult true findings remain discoverable. |
| Actionable acceptance rate | Qualified findings accepted for retained regression or remediation work. Review decisions. | Finding funnel. | Higher; never used alone as truth. |

#### 5.4.6 Collection and projection

All metric inputs are append-only native facts, not client analytics guesses:

- the managed-sandbox broker records admission, generation, readiness, lease,
  budget, stop, deletion, cleanup, measured runtime, and infrastructure cost;
- the forensic driver records tranche, agent role, turn, tool, file/dependency,
  hypothesis, finding, verifier, artifact, retry, and settlement events;
- provider adapters record exact usage when exposed and explicitly mark
  estimated or unavailable usage;
- deterministic evaluators append vulnerability identity, causal-link,
  reference, control, PoC, and holdout adjudications without mutating the
  original event; and
- Omega appends human review decisions and bounded active-review intervals.

Each event binds run, benchmark, dataset split, arm, repetition, target/source
bundle, prompt digest, model and parameters, worker image/profile, sandbox and
generation, evaluator revision, sequence, and receipt refs. Native events and
private evidence remain in the existing Cloud SQL/Khala Sync and private GCS
boundaries. A metrics projector derives `ForensicScorecard.v1`; projections
are rebuildable and never become lifecycle or finding authority.

The projector separates complete and incomplete inputs, vulnerable and clean
controls, development and holdout, and model/worker revisions. Public-safe
aggregates contain counts, durations, costs, and digests only. Repository
paths, prompts, findings, raw model output, topology, and credentials stay in
the admitted private evidence boundary. Even aggregate publication remains
default-private until an explicit projection and release policy admits it.

#### 5.4.7 Omega views

| View | What it shows |
| --- | --- |
| Live run header | Current phase, elapsed time, token/cost/budget consumption, current tranche, first-hypothesis and qualified-hit markers, worker health, and cleanup state. |
| Run waterfall | Queue, provision, source materialization, preflight, analysis, verification, review, stop/delete, and cleanup durations with findings and failures placed on the same ordered timeline. |
| Finding detail | Time and tokens to this finding, causal-link coverage, evidence validity, verification status, duplicate group, and human decision. |
| Matrix comparison | One row per prompt/model/profile arm; hit rate, misses, p50/p95 identification time, tokens and cost to identification, causal coverage, false positives, cleanup, and sample size. |
| Trend and Pareto view | Version-over-version curves for recall@time and recall@tokens plus the non-dominated frontier of hit rate, identification latency, tokens, cost, false positives, and reviewer load. |
| Metric provenance drawer | Exact formula and version, eligibility and censor rules, raw event/receipt refs, exactness, dataset split, sample size, confidence interval, and evaluator revision. |

Incomplete runs remain visible in the same interface but never pool with
complete runs. A user can drill from any aggregate to the exact contributing
runs, including misses and failures.

#### 5.4.8 Improvement and promotion policy

Do not collapse the program into one reward number. Optimize in this order:

1. satisfy every input, isolation, clean-control, evidence, budget, and cleanup
   hard gate;
2. maximize qualified holdout hit rate and causal-chain coverage at fixed time
   and token budgets;
3. among candidates that do not regress those properties, reduce p50 and tail
   time to identification, tokens to identification, and cost to
   identification; and
4. then reduce verification latency, reviewer load, duplicate burden, and
   infrastructure waste.

Show the Pareto frontier rather than hiding tradeoffs in a weighted average. A
candidate that is faster because it misses more runs, emits unsupported claims,
skips dependencies, disables verification, or spends more parallel tokens does
not improve. Promotion comparisons use matched arms, pinned worker/model
revisions, pre-registered budgets and sample sizes, blinded holdout data, clean
controls, and uncertainty intervals. Baseline and candidate retain every miss,
cancellation, retry, and cleanup failure.

The first release gate remains full causal identification during the first
priority tranche. Initial repetitions establish the baseline distributions;
the next roadmap revision should freeze explicit `Hit@time`, `Hit@tokens`, p50,
tail-latency, token, cost, false-positive, and reviewer-load targets rather than
choosing targets after seeing candidate results.

### 5.5 Data splits

The known Coldcard incident may be used for development. It may not be the
holdout.

- **Train:** examples used directly by prompt or optimizer candidates.
- **Development:** Coldcard and visible structural variants used during prompt
  iteration.
- **Holdout:** separately owned fixtures and historical failures not exposed to
  candidate generation.
- **Clean holdout:** correct implementations and ambiguous cases where the only
  honest result is `not proven`.

Missing or overlapping holdout data fails the promotion gate. Train data never
silently becomes evaluation data.

---

## 6. Prompt optimization with DSPy and Blueprint

OpenAgents has useful prior art, but the current state must be named accurately:

- the former Effect-native DSE package implemented typed signatures, Prompt IR,
  bounded search, evaluation, receipts, budgets, and promotion controls;
- it did not implement real GEPA or MIPROv2 and is not present in current
  source;
- current Blueprint has schemas for Program Runs, Optimizer Runs, candidate
  Module Versions, scorecards, and release gates;
- Blueprint records candidate lineage and enforces evidence-only output, but it
  does not execute an optimizer;
- current GEPA-related code emits governed failure feedback, not optimized
  prompts.

The forensic system should use this split.

### 6.1 Offline candidate compiler

Run prompt search as an offline, bounded job. Start with deterministic methods:

- human-curated candidates;
- ablation of individual prompt sections;
- instruction grids;
- few-shot selection;
- context-policy and budget grids;
- retained-failure replay.

Add upstream Python DSPy or a real GEPA implementation only after the dataset,
metrics, and candidate contracts work. Do not label rule-based refinement as
GEPA. Python optimization may run in an isolated build job; Omega runtime does
not depend on Python or an optimizer service.

The candidate compiler may coordinate work in the control plane, but every
candidate evaluation that reads or executes a target repository uses the same
OpenAgents Cloud worker requirement. Optimization is not a reason to introduce
a local evaluator, a second sandbox provider, or shared mutable workers.

Candidate generation may optimize for more detailed output, but it cannot trade
away evidence validity, incomplete-input detection, fixed-control precision, or
budget compliance. Those are gates, not weighted preferences.

The candidate generator consumes the versioned metric registry and train or
development scorecards only. Its objective is lexicographic and Pareto-based:
pass the hard gates, improve qualified hit rate and causal coverage at fixed
budgets, then reduce identification time, tokens, and cost. The independent
promotion evaluator applies the same frozen registry to holdout scorecards. No
optimizer may redefine `T5`, exclude misses, change censoring, inspect holdout
examples, or select a metric revision after seeing candidate results.

### 6.2 Blueprint governance spine

Represent each scan as an evidence-only Program Run. Represent an optimization
series as an Optimizer Run whose candidate modules point to immutable prompt
artifacts and scorecards. Reuse or extend the existing Blueprint contracts only
through an admitted product boundary; do not make Omega depend directly on the
OpenAgents web worker.

Required properties:

- candidate ID covers the complete prompt artifact, schema, tools, parameters,
  dataset revision, and optimizer configuration;
- every candidate names train, development, and holdout revisions;
- retained failures point back to exact run and finding refs;
- candidates cannot self-promote;
- promotion requires an explicit release gate and operator decision;
- active prompt pointers are reversible and retain rollback anchors;
- runtime serves a checked-in or otherwise immutable admitted artifact;
- every result remains attributable to the artifact that produced it.

### 6.3 Independent evaluation

The candidate generator and release evaluator must have distinct execution
identities and inputs. A second model family is useful but not sufficient;
mechanical checks remain the independent evidence:

- source reference resolves to the pinned tree;
- dependency manifest matches the mounted worktree;
- asserted macro values match preprocessing;
- asserted symbols match object and link-map inspection;
- PoC is observed failing on the vulnerable revision;
- the same check passes on the fixed revision;
- holdout membership is disjoint from optimizer-visible data.

Pinned model judges may score clarity or usefulness. They never decide whether
a factual source or artifact claim is true when a deterministic check exists.

---

## 7. Versioned contracts

Names are provisional; the information is not.

| Contract | Required content |
| --- | --- |
| `ForensicTargetSnapshot.v1` | Repository, commit, source digest, dirty-state truth, dependency policy, toolchain refs, and authorization refs. |
| `ForensicSourceBundle.v1` | Exact repository and commit, git tree, declared submodule commits, dependency manifest, private artifact ref, content digest, builder identity, retention, and materialization receipt. |
| `ForensicCoverageManifest.v1` | Present, absent, excluded, oversized, generated, and dependency paths; coverage status and reasons. |
| `ForensicScanProfile.v1` | Scope ranking, vulnerability classes, model matrix, prompt artifact, tools, sandbox, network, and budgets. |
| `ForensicWorkerPlacement.v1` | Owner, tenant, work unit, sandbox and generation refs; exact OpenAgents-managed target, provider, adapter, isolation, region, image/profile digests, network policy, lease, budget, capabilities, admission, readiness, stop, deletion, and cleanup receipt refs. |
| `ForensicPromptArtifact.v1` | Structured Prompt IR, input/output schemas, examples, parameters, canonical digest, lineage, and compatibility. |
| `ForensicRun.v1` | Target, profile, worker placement, source bundle, state, timing, usage truth, findings, hypotheses, errors, and native receipt refs. |
| `ForensicFinding.v1` | Claim, causal steps, source refs, assumptions, severity, evidence tier, PoC, verifier state, and disclosure state. |
| `ForensicHypothesis.v1` | Suspected mechanism, supporting refs, missing evidence, next check, consequence if true, and expiration state. |
| `ForensicEvidenceReceipt.v1` | Exact command or tool, immutable inputs, observed result, artifact digests, environment, and timestamp. |
| `ForensicRunEvent.v1` | Append-only run sequence for lifecycle milestones, tranche, agent role, turn, tool, usage, hypothesis, finding, verification, review, failure, and cleanup refs. |
| `ForensicMetricDefinition.v1` | Versioned formula, unit, eligible population, censor and miss treatment, exactness rules, dimensions, aggregation, and display metadata. |
| `ForensicScorecard.v1` | Dataset revision, metric-definition revision, hard-gate results, per-run values, distributions, censor counts, confidence intervals, failures, cost, and event/receipt provenance. |
| `ForensicPromptPromotion.v1` | Candidate, evaluator, release gate, operator decision, rollback anchor, and active-pointer transition. |

Run state is explicit:

```text
draft -> preflight -> ready_inputs | incomplete | denied
ready_inputs | incomplete -> admission_requested -> provisioning -> worker_ready
worker_ready -> running -> settling | cancel_requested | failed
settling | cancel_requested | failed -> cleanup_requested -> cleaned
cleaned -> completed | completed_incomplete | cancelled | failed
cleanup_requested -> recovery_required
completed* -> review -> candidate | retained | dismissed
candidate -> release_gate -> admitted | rejected
```

An `incomplete` run can never transition into a complete result merely because
it produced findings. `completed*` is available only after the native sandbox
has structurally settled, stopped, deleted, and produced a zero-residue cleanup
receipt. Lease or budget expiry may abort work as a declared guardrail, but it
cannot manufacture success. Output silence never means idle or complete.

---

## 8. Delivery phases

### Phase 0 — freeze the benchmark and run contracts

Deliver:

- Coldcard manifest with the five required arms;
- structured finding and hypothesis schemas;
- immutable source-bundle and coverage-manifest schemas;
- coverage manifest and incomplete-state rules;
- frozen milestone, metric-definition, eligibility, censoring, token-exactness,
  and aggregation rules;
- `ForensicWorkerPlacement.v1` bound to the native managed-sandbox identity,
  generation, admission, lifecycle, budget, and cleanup contracts;
- exact GCE forensic image/profile inputs, a broker-only network policy, and
  negative controls that refuse local, fake, BYO, Box-owned, and substitute
  provider targets;
- frozen scoring rubric and dataset split rules;
- a replay importer for the Episode 264 experiment results;
- explicit sandbox, network, retention, and budget defaults.

Exit gate:

- the existing Arm A result imports as `completed_incomplete`, not a complete
  miss;
- Arm B imports as an unverified source-level hit;
- the fixed commit and holdout identities are separate from development data;
- a fake or unavailable GCE target cannot satisfy `worker_ready`, and a missing
  cleanup receipt cannot satisfy `completed*`;
- imported Episode 264 data marks unavailable historical timing or token
  fields as unavailable rather than zero.

### Phase 1 — run Loupe-style analysis from Omega

Deliver:

- Forensics workbench bound to an Omega repository context;
- immutable source-bundle creation with pinned target and dependency preflight;
- owner-gated native Desktop admission and command routing through the
  OpenAgents managed-sandbox broker;
- one disposable GCE VM launch per run, an image-pinned forensic workload
  driver, Linux and Bubblewrap readiness proof, and ordered progress stream;
- append-only milestone, tranche, turn, tool, usage, finding, verification,
  review, and cleanup instrumentation plus a rebuildable metric projector;
- editable prompt artifact with immutable save-as-candidate behavior;
- typed finding and hypothesis intake;
- editor navigation, PoC diff, log, receipt, live budget, run-waterfall, and
  metric-provenance views;
- manual cancellation and hard budget stops;
- no-reporting default.

Fix or route around Loupe's broken verify stage before presenting any finding
as independently verified. A discovery-only run must say so everywhere.

Exit gate:

- from Omega, an operator launches the complete Coldcard vulnerable arm and
  receives the full source-level causal finding in the first priority tranche;
- the incomplete arm is blocked or visibly degraded before inference;
- the guest has no external IP, guest service account, provider credential,
  raw GCP identity, or ambient Internet egress;
- local, fake, Pylon, Box-owned, foreign-cloud, and generic remote-Linux
  placements fail before target effects;
- cancelling the run structurally interrupts work, stops and deletes the
  sandbox, and preserves runtime and cleanup receipts;
- a broker outage, unmeasurable spend, or incomplete cleanup produces refusal
  or `recovery_required`, never a successful result;
- Coldcard runs show end-to-end and analysis time to identification, tokens and
  cost to identification, causal coverage, exactness, and censor state from
  native events;
- no target source, credentials, or findings leave the admitted boundary.

### Phase 2 — prompt lab and controlled run matrices

Deliver:

- prompt diff and lineage views;
- run matrices across prompts, models, scopes, and dependency policies;
- fresh managed-sandbox placement per matrix arm with placement digests in the
  comparison key;
- repeated-run support and stochastic result summaries;
- divergence view;
- deterministic scorecard computation;
- recall-versus-time and recall-versus-token curves, matched baseline tables,
  confidence intervals, and Pareto views;
- retained-failure capture;
- baseline-versus-candidate comparison.

Exit gate:

- at least one candidate improves a pre-registered development-set Pareto
  metric without regressing qualified hit rate, causal detail, any hard gate,
  or clean control;
- every score is reproducible from retained refs;
- result comparison never merges incomplete and complete runs as peers.

### Phase 3 — DSPy/GEPA candidate optimization

Deliver:

- bounded offline compiler consuming immutable datasets;
- human, ablation, grid, few-shot, and retained-failure candidates first;
- real DSPy or GEPA adapter only when accurately named and independently tested;
- Blueprint-compatible Optimizer Run, candidate, scorecard, and release-gate
  records;
- checked-in admitted prompt artifact and rollback pointer.

Exit gate:

- a candidate wins on an untouched holdout and clean holdout;
- the evaluator proves dataset separation;
- the candidate does not self-promote;
- rollback restores the prior prompt without changing run history.

### Phase 4 — artifact witness MVP

This begins the longer Codex-analysis vision.

Deliver for one C/C++ embedded build pipeline:

- pinned toolchain and build-configuration capture;
- preprocessed-source and compiler-command inventory;
- object, archive, symbol, link-map, firmware, and debug-metadata ingestion;
- allowed-symbol-provider and forbidden-fallback assertions;
- source-to-secret-sink reachability assertions;
- retained-width and truncation checks;
- fault builds that remove the approved provider and must fail closed;
- `satisfied | violated | not_proven` results with reproducible receipts.

Exit gate:

- on the pre-fix Coldcard build, the witness observes the wrong provider rather
  than inferring it;
- on the fixed build, it observes the approved provider and the absence of
  fallback symbols;
- removing the approved provider makes the build or invariant check fail;
- no statistical output test is accepted as proof of entropy provenance.

### Phase 5 — executable evidence and controlled dynamic analysis

Deliver:

- apply and run regression PoCs in disposable managed sandboxes;
- provision those sandboxes only through the same admitted OpenAgents Cloud
  GCE target, with a separately admitted dynamic-analysis profile when needed;
- observe failure on the vulnerable revision and success after repair;
- property and differential tests for cryptographic behavior;
- agent-authored fuzz harnesses for prioritized parser and state-machine
  boundaries;
- crash minimization, deduplication, and regression-pack generation;
- synthetic, regtest, signet, emulator, or owned-hardware impact labs.

Exit gate:

- a finding advances evidence tier only from an observed receipt;
- lab impact tests use synthetic secrets and nonvaluable outputs;
- network, credential, artifact export, and spend are separate typed
  capabilities;
- no dynamic result grants disclosure authority.

### Phase 6 — entropy, oracle, delta, and variant analysis

Deliver:

- secret-source, mixer, reseed, retained-width, and consumer analysis;
- cheap-oracle mapping for addresses, keys, signatures, and protocol outputs;
- priority ranking by consequence and attacker economics;
- commit-delta analysis over callers, callees, generated configuration, build
  inputs, resolved symbols, and downstream consumers;
- private structural variant queries across authorized branches, forks,
  vendored copies, and sibling projects;
- persistent regression packs after repair.

Exit gate:

- a confirmed fixture produces variants without exact symbol names;
- clean siblings remain clean;
- newly missing evidence becomes `not_proven`, never `safe`;
- fix persistence is checked against a later release.

### Phase 7 — coordinated defensive service

Deliver:

- signed scan ledger with coverage and configuration receipts;
- reviewed community scan profiles;
- private divergence detection;
- NIP-29 campaign rooms with stable human and agent identities;
- nonrevealing finding commitments;
- verified maintainer contacts, encrypted delivery, and embargo state;
- proof-carrying fix packages and release watch;
- campaign accounting and stop conditions.

Exit gate:

- OpenAgents has completed several authorized, human-reviewed disclosure drills
  before offering a broad ecosystem service;
- public coverage never exposes a useful attacker target list;
- discovery, verification, repair, contact, publication, and settlement remain
  separate authorities;
- success is measured by confirmed fixes, false-positive burden, time to repair,
  variant coverage, persistence, and cost, not finding count.

---

## 9. Issue-ready first sequence

The first implementation program should be cut in this order:

| ID | Work item | Primary home |
| --- | --- | --- |
| OFR-001 | Define forensic target, source bundle, coverage, worker placement, prompt, run/event, finding, hypothesis, receipt, metric-definition, and scorecard contracts. | `openagents` |
| OFR-002 | Admit an image-pinned forensic worker profile on the native OpenAgents managed-sandbox GCE path; prove readiness, refusal of every fallback, cost limits, cancellation, deletion, and zero residue. | OpenAgents Cloud |
| OFR-003 | Materialize a pinned target and all declared submodules into an immutable private source bundle, deliver it through a scoped capability, and emit the coverage manifest without guest Internet egress. | OpenAgents Cloud / upstream Loupe where general |
| OFR-004 | Import the Coldcard benchmark arms and frozen rubric. | `openagents` |
| OFR-005 | Implement native forensic milestone and usage events, the frozen metric registry, evaluator adjudications, rebuildable scorecards, and miss/censor negative controls. | `openagents` + OpenAgents Cloud |
| OFR-006 | Add a configurable Loupe prompt/profile seam without weakening typed submission. | upstream Loupe or adapter |
| OFR-007 | Repair and prove the Loupe verifier path, or keep the first release explicitly discovery-only. | upstream Loupe or adapter |
| OFR-008 | Add Omega Forensics target, OpenAgents-managed placement, and preflight UI. | `omega` |
| OFR-009 | Launch, interrupt, cancel, clean up, and monitor a native managed-sandbox run from Omega with hard budgets. | `omega` + `openagents` |
| OFR-010 | Render findings, hypotheses, evidence maps, PoC diffs, source navigation, placement truth, live budgets, run waterfalls, metric provenance, and cleanup status. | `omega` |
| OFR-011 | Add prompt artifact editor, diff, digest, and candidate save. | `omega` + `openagents` |
| OFR-012 | Add matched run matrices, recall curves, divergence, retained failures, confidence intervals, Pareto views, and deterministic scorecards. | `omega` + `openagents` |
| OFR-013 | Add bounded offline prompt compilation and Blueprint release-gate records. | `openagents` |
| OFR-014 | Build the Coldcard C/C++ artifact witness and fault-build fixtures. | OpenAgents Cloud / `openagents` |

Do not start OFR-013 by restoring the deleted DSE package unchanged. Harvest its
tested design: typed signatures, Prompt IR, canonical hashes, budgets, receipts,
dataset splits, immutable candidates, and rollback. Implement against the
current Effect, Node, package-manager, and runtime boundaries.

---

## 10. Non-negotiable boundaries

1. **Worker supply is closed for now.** Every repository-reading, inference,
   build, test, PoC, and fuzz run uses the exact admitted OpenAgents Cloud GCE
   target. Unavailable capacity refuses; it never falls back to local, fake,
   BYO, Box-owned, Pylon, generic remote Linux, or another provider.
2. **Repository content is hostile input.** Source comments and documentation
   never modify system policy, tool permissions, targets, or disclosure state.
3. **Complete input is a claim.** Missing dependencies, generated sources,
   toolchains, hardware, or build metadata remain visible and produce
   `incomplete` or `not_proven`.
4. **No live-key work.** Never derive, search for, correlate, or spend a real
   user's key. Coldcard impact demonstrations use known synthetic seeds and
   owned local fixtures only.
5. **No live exploitation.** Dynamic software work stays inside the admitted
   OpenAgents Cloud sandbox against synthetic, regtest, signet, or emulator
   targets. Owned-hardware work requires a later, separate capability and gate.
6. **No automatic reporting.** Findings remain private and manual-reporting by
   default. A model verdict does not authorize maintainer contact or publication.
7. **No self-promotion.** Prompt and module candidates remain evidence-only
   until an independent release gate and operator decision admit them.
8. **Budgets are enforced by runtime.** Prompt text cannot raise time, token,
   concurrency, network, or infrastructure limits.
9. **Detailed is not verified.** Longer explanations, multiple-model agreement,
   and confident severity do not advance evidence tier.
10. **Public claims require executed evidence and authority.** Rediscovering a
   known historical bug is not a zero-day track record.
11. **No scanner monoculture.** Share useful fixes upstream, retain configuration
    diversity, and compare divergence before replacing a working tool.

---

## 11. Decisions this roadmap makes

- Start in Omega with an interactive, configurable Loupe-style lab.
- Run every initial forensic workload on one disposable, admitted OpenAgents
  Cloud GCE VM through the native managed-sandbox broker, with no fallback.
- Treat Box semantics as design input and its API as optional compatibility,
  never as Omega's worker authority.
- Make dependency completeness and honest incomplete states the first gate.
- Preserve Loupe's typed finding and verdict-ordering discipline.
- Add a separate typed hypothesis lane for detailed forensic inquiry.
- Optimize versioned prompt artifacts against frozen datasets and scorecards.
- Measure qualified identification time, tokens, cost, recall at fixed budgets,
  causal coverage, false positives, cleanup, and reviewer load from native
  events, with misses and unavailable data kept visible.
- Promote from hard gates and a visible Pareto frontier, never a single score
  that can trade detection quality for speed or token savings.
- Use offline DSPy/GEPA for candidate generation and Blueprint for governed
  lineage and promotion, not as interchangeable names for one system.
- Require holdout independence and clean controls before prompt promotion.
- Build artifact provenance before claiming final-binary proof.
- Advance from source candidates to executed evidence, variants, coordination,
  and proof-carrying remediation in that order.

The near-term success condition is not "Omega has a security scanner." It is:

> Omega lets us run a reproducible forensic experiment, improve the analysis
> program without moving the scoring goalposts, and show exactly why the new
> program is better on evidence it was not allowed to train on.
