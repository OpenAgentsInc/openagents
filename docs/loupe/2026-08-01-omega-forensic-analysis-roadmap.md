# Omega forensic analysis roadmap

Status: **owner-directed roadmap.** This document sequences product and
research work. It does not itself authorize scanning a third-party target,
publishing a vulnerability claim, contacting a maintainer, spending beyond an
admitted budget, or running an exploit outside an owned lab.

Roadmap revision: 1

Date: 2026-08-01

Primary evidence:

- [`Episode 264 - Running Loupe`](../transcripts/264.md)
- [`Results: Loupe on the pre-fix Coldcard firmware`](2026-08-01-coldcard-prefix-experiment-results.md)
- [`Codex analysis: what should catch the Coldcard class of failure`](2026-08-01-codex-analysis.md)
- [`Coordination, not scanners`](2026-08-01-coordination-not-scanners.md)
- [`DSPy in Effect: Git history audit`](../dspy/2026-07-20-dspy-in-effect-git-history-audit.md)
- [`Blueprint Kernel Boundary`](../../apps/openagents.com/workers/api/src/blueprint/README.md)

The first target is the historical, pre-fix Coldcard firmware at
`bcc2c382a324690a2fcf972c0bac3b79bf923f7b`. The first product is an
interactive forensic-analysis lab inside Omega. The longer direction is the
security-invariant and evidence workbench described in the Codex analysis.

---

## 1. Outcome

An operator should be able to open a pinned repository in Omega, select or edit
a forensic prompt program, launch a dependency-complete Loupe-style run on an
isolated Linux worker, and inspect every finding, causal path, uncertainty,
proof artifact, and comparison without leaving the IDE.

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
| OpenAgents control plane | Versioned contracts, benchmark datasets, prompt artifacts, evaluation, optimizer records, release gates, worker dispatch, and durable evidence refs. |
| Isolated Linux worker | Checkout preparation, dependency materialization, Loupe execution, builds, tests, artifact inspection, and later fuzzing. |
| Upstream Loupe | Scanner lifecycle, typed finding tools, verdict-before-patch discipline, deduplication, and generally useful checkout or prompt hooks. |

The macOS Omega application is the console. Bubblewrap scanning remains on
Linux. Higher-risk build or dynamic work moves to one disposable microVM per
target rather than weakening Loupe's isolation to make it run locally.

The default run is read-only and manual-reporting. The target checkout is
immutable. PoC patches, builds, and generated harnesses live in disposable
scratch storage. A forensic run produces evidence and proposals, not source
mutation or public claims.

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
- worker isolation and network policy;
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

- detects incomplete inputs before inference;
- cites the exact source evidence it used;
- names all required causal links for a Coldcard `HIT`;
- labels final-link outcome `not proven` until artifact evidence exists;
- does not reproduce the finding against the fixed control;
- never claims to have inspected an absent dependency;
- stays within the admitted run budget.

Scorecard dimensions:

- causal-chain coverage;
- evidence-reference validity;
- uncertainty calibration;
- PoC or invariant-check quality;
- actionable detail;
- false-positive and duplicate burden;
- time to first actionable finding;
- time to reviewed finding;
- tokens, wall-clock, and cost per confirmed finding;
- result stability across repeated runs;
- performance on renamed and blinded variants.

"Immediately" becomes a measured property. Before setting a public latency
number, run at least three baseline repetitions on a pinned worker class. The
initial release gate is that the full causal finding appears during the first
priority tranche, before lower-risk bulk scanning completes. The resulting
p50 and p95 become the next revision's explicit latency target.

### 5.4 Data splits

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

Candidate generation may optimize for more detailed output, but it cannot trade
away evidence validity, incomplete-input detection, fixed-control precision, or
budget compliance. Those are gates, not weighted preferences.

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
| `ForensicCoverageManifest.v1` | Present, absent, excluded, oversized, generated, and dependency paths; coverage status and reasons. |
| `ForensicScanProfile.v1` | Scope ranking, vulnerability classes, model matrix, prompt artifact, tools, sandbox, network, and budgets. |
| `ForensicPromptArtifact.v1` | Structured Prompt IR, input/output schemas, examples, parameters, canonical digest, lineage, and compatibility. |
| `ForensicRun.v1` | Target, profile, worker, state, timing, usage truth, findings, hypotheses, errors, and receipt refs. |
| `ForensicFinding.v1` | Claim, causal steps, source refs, assumptions, severity, evidence tier, PoC, verifier state, and disclosure state. |
| `ForensicHypothesis.v1` | Suspected mechanism, supporting refs, missing evidence, next check, consequence if true, and expiration state. |
| `ForensicEvidenceReceipt.v1` | Exact command or tool, immutable inputs, observed result, artifact digests, environment, and timestamp. |
| `ForensicScorecard.v1` | Dataset revision, hard-gate results, metrics, failures, cost, and comparison refs. |
| `ForensicPromptPromotion.v1` | Candidate, evaluator, release gate, operator decision, rollback anchor, and active-pointer transition. |

Run state is explicit:

```text
draft -> preflight -> ready -> running -> completed
                  \-> incomplete -> running_incomplete -> completed_incomplete
                  \-> denied
running -> failed | cancelled
completed* -> review -> candidate | retained | dismissed
candidate -> release_gate -> admitted | rejected
```

An `incomplete` run can never transition into a complete result merely because
it produced findings.

---

## 8. Delivery phases

### Phase 0 — freeze the benchmark and run contracts

Deliver:

- Coldcard manifest with the five required arms;
- structured finding and hypothesis schemas;
- coverage manifest and incomplete-state rules;
- frozen scoring rubric and dataset split rules;
- a replay importer for the Episode 264 experiment results;
- explicit sandbox, network, retention, and budget defaults.

Exit gate:

- the existing Arm A result imports as `completed_incomplete`, not a complete
  miss;
- Arm B imports as an unverified source-level hit;
- the fixed commit and holdout identities are separate from development data.

### Phase 1 — run Loupe-style analysis from Omega

Deliver:

- Forensics workbench bound to an Omega repository context;
- pinned target and dependency preflight;
- remote Linux worker launch and progress stream;
- editable prompt artifact with immutable save-as-candidate behavior;
- typed finding and hypothesis intake;
- editor navigation, PoC diff, log, and receipt views;
- manual cancellation and hard budget stops;
- no-reporting default.

Fix or route around Loupe's broken verify stage before presenting any finding
as independently verified. A discovery-only run must say so everywhere.

Exit gate:

- from Omega, an operator launches the complete Coldcard vulnerable arm and
  receives the full source-level causal finding in the first priority tranche;
- the incomplete arm is blocked or visibly degraded before inference;
- cancelling the run stops workers and preserves a terminal receipt;
- no target source, credentials, or findings leave the admitted boundary.

### Phase 2 — prompt lab and controlled run matrices

Deliver:

- prompt diff and lineage views;
- run matrices across prompts, models, scopes, and dependency policies;
- repeated-run support and stochastic result summaries;
- divergence view;
- deterministic scorecard computation;
- retained-failure capture;
- baseline-versus-candidate comparison.

Exit gate:

- at least one candidate improves actionable causal detail on development data
  without regressing any hard gate or clean control;
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

- apply and run regression PoCs in disposable microVMs;
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
| OFR-001 | Define forensic target, coverage, prompt, run, finding, hypothesis, receipt, and scorecard contracts. | `openagents` |
| OFR-002 | Check out a pinned target with explicit submodule/dependency materialization and emit the coverage manifest. | worker / upstream Loupe where general |
| OFR-003 | Import the Coldcard benchmark arms and frozen rubric. | `openagents` |
| OFR-004 | Add a configurable Loupe prompt/profile seam without weakening typed submission. | upstream Loupe or adapter |
| OFR-005 | Repair and prove the Loupe verifier path, or keep the first release explicitly discovery-only. | upstream Loupe or adapter |
| OFR-006 | Add Omega Forensics target and preflight UI. | `omega` |
| OFR-007 | Launch, cancel, and monitor an isolated run from Omega with hard budgets. | `omega` + `openagents` |
| OFR-008 | Render findings, hypotheses, evidence maps, PoC diffs, and source navigation. | `omega` |
| OFR-009 | Add prompt artifact editor, diff, digest, and candidate save. | `omega` + `openagents` |
| OFR-010 | Add run matrices, divergence, retained failures, and deterministic scorecards. | `omega` + `openagents` |
| OFR-011 | Add bounded offline prompt compilation and Blueprint release-gate records. | `openagents` |
| OFR-012 | Build the Coldcard C/C++ artifact witness and fault-build fixtures. | worker / `openagents` |

Do not start OFR-011 by restoring the deleted DSE package unchanged. Harvest its
tested design: typed signatures, Prompt IR, canonical hashes, budgets, receipts,
dataset splits, immutable candidates, and rollback. Implement against the
current Effect, Node, package-manager, and runtime boundaries.

---

## 10. Non-negotiable boundaries

1. **Repository content is hostile input.** Source comments and documentation
   never modify system policy, tool permissions, targets, or disclosure state.
2. **Complete input is a claim.** Missing dependencies, generated sources,
   toolchains, hardware, or build metadata remain visible and produce
   `incomplete` or `not_proven`.
3. **No live-key work.** Never derive, search for, correlate, or spend a real
   user's key. Coldcard impact demonstrations use known synthetic seeds and
   owned local fixtures only.
4. **No live exploitation.** Dynamic work runs only on synthetic, local,
   regtest, signet, emulator, or explicitly owned hardware and infrastructure.
5. **No automatic reporting.** Findings remain private and manual-reporting by
   default. A model verdict does not authorize maintainer contact or publication.
6. **No self-promotion.** Prompt and module candidates remain evidence-only
   until an independent release gate and operator decision admit them.
7. **Budgets are enforced by runtime.** Prompt text cannot raise time, token,
   concurrency, network, or infrastructure limits.
8. **Detailed is not verified.** Longer explanations, multiple-model agreement,
   and confident severity do not advance evidence tier.
9. **Public claims require executed evidence and authority.** Rediscovering a
   known historical bug is not a zero-day track record.
10. **No scanner monoculture.** Share useful fixes upstream, retain configuration
    diversity, and compare divergence before replacing a working tool.

---

## 11. Decisions this roadmap makes

- Start in Omega with an interactive, configurable Loupe-style lab.
- Make dependency completeness and honest incomplete states the first gate.
- Preserve Loupe's typed finding and verdict-ordering discipline.
- Add a separate typed hypothesis lane for detailed forensic inquiry.
- Optimize versioned prompt artifacts against frozen datasets and scorecards.
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
