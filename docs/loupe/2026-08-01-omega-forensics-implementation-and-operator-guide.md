# Omega forensic analysis: implementation and operator guide

Status: **implementation and operating guide for the completed OFR-001 through
OFR-018 development program.** The components described here exist and have
focused tests. OpenAgents Cloud forensic placement remains default-off unless
the exact managed profile is admitted. This document does not authorize a scan,
publish a vulnerability claim, contact a maintainer, or enable production
service.

Date: 2026-08-01

Use this document as the entry point for the system that grew from the Loupe
study and Coldcard experiment. Use the
[Coldcard practice-run runbook](../coldcard/2026-08-01-omega-coldcard-forensic-practice-runbook.md)
for the first benchmark. The longer rationale and delivery history remain in
the [forensic-analysis roadmap](2026-08-01-omega-forensic-analysis-roadmap.md).

## 1. What we built

We built a bounded forensic-analysis system that joins four previously separate
concerns:

1. Loupe-style agent discovery with typed findings and an independent verifier;
2. dependency-complete, immutable source materialization;
3. one disposable Linux worker from OpenAgents Cloud's admitted Google Cloud
   supply; and
4. an Omega workbench for preflight, prompt candidates, run control, evidence
   review, comparison, and the Coldcard reproduction chain.

The result is not a generic scanner and not an autonomous disclosure bot. It is
an evidence workbench. Every important transition is typed, every run binds its
inputs, missing evidence stays missing, and cleanup is part of terminal truth.

```text
Omega repository context
  -> target + benchmark arm + prompt digest
  -> OpenAgents source materializer
  -> coverage manifest and immutable private bundle
  -> admitted OpenAgents Cloud GCE worker
  -> Loupe-style discovery adapter
  -> independent verifier
  -> typed events, findings, hypotheses, receipts, and metrics
  -> Omega review, run matrix, and Coldcard evidence views
  -> verified deletion and zero-residue cleanup
```

### 1.1 Implementation inventory

| Capability | Implementation | What it owns |
| --- | --- | --- |
| Canonical contracts | [`packages/forensic-contract/`](../../packages/forensic-contract/) | Strict Effect Schemas, canonical JSON and SHA-256 identity, lifecycle laws, metrics, scorecards, Coldcard reproduction, artifact witness, generator, historical scan, evidence graph, and claim history. |
| Loupe-style discovery | [`packages/forensic-loupe-adapter/`](../../packages/forensic-loupe-adapter/) | Structured prompt compilation, typed finding and hypothesis lanes, immutable checkout checks, bounded events, and `manual_no_reporting`. |
| Independent verifier | [`packages/forensic-loupe-adapter/src/verifier.ts`](../../packages/forensic-loupe-adapter/src/verifier.ts) | Distinct verifier identity, verdict-before-PoC ordering, vulnerable/fixed execution receipts, and discovery-only release gating. |
| Managed worker | [`apps/openagents.com/workers/api/src/forensic-managed-sandbox.ts`](../../apps/openagents.com/workers/api/src/forensic-managed-sandbox.ts) | Admission, dispatch, observation, interrupt, settlement, artifact collection, deletion, and cleanup for one exact GCE worker generation. |
| Guest workload boundary | [`scripts/cloud/forensic-worker-driver.mjs`](../../scripts/cloud/forensic-worker-driver.mjs) | Allowlisted Linux workload execution under Bubblewrap, bounded I/O, process-group cancellation, and residue checks. |
| Source materializer | [`apps/openagents.com/workers/api/src/forensic-source-materializer.ts`](../../apps/openagents.com/workers/api/src/forensic-source-materializer.ts) | Commit and tree resolution, gitlink and submodule coverage, private bundle identity, scoped delivery, and removal. |
| Prompt optimization | [`apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.ts`](../../apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.ts) | Immutable candidates, bounded offline compilation, holdout isolation, independent evaluation, Blueprint release gates, activation, and rollback records. |
| Coldcard benchmark | [`fixtures/forensics/coldcard/`](../../fixtures/forensics/coldcard/) | Five development arms, the six-link source rubric, four reproduction suites, frozen controls, dataset splits, and synthetic fixtures. |
| Node fingerprint tools | [`tools/coldcard/`](../../tools/coldcard/) | Independent, exact-integer, append-only Bitcoin Core fingerprint scanning and base-rate stratification. |
| Omega domain and renderer | [`omega_forensics`](https://github.com/OpenAgentsInc/omega/tree/main/crates/omega_forensics) and [`forensics_workbench.rs`](https://github.com/OpenAgentsInc/omega/blob/main/crates/agent_ui/src/forensics_workbench.rs) | Renderer-safe projections, preflight, prompt candidates, run lifecycle, review, matrices, and the Coldcard evidence workspace. |

OFR-001 through OFR-007 established the OpenAgents contract, worker, source,
benchmark, metric, adapter, and verifier layers. OFR-008 through OFR-012 added
the Omega target, lifecycle, review, prompt, and comparison surfaces. OFR-013
added Blueprint-governed prompt optimization. OFR-014 through OFR-017 added
the Coldcard artifact, generator, historical-scan, and evidence-graph suites.
OFR-018 linked that evidence into Omega.

## 2. The non-negotiable operating model

### 2.1 Omega is the console, not the sandbox

Omega selects the target, shows authority and budget, emits typed commands, and
renders bounded projections. It does not receive Google Cloud credentials,
provider clients, raw topology, worker control tokens, private source bytes, or
a generic shell.

The host resolves the authenticated OpenAgents session and calls only the
native `POST /api/forensics/workers` route. The route accepts admission,
dispatch, observation, artifact collection, cancellation, and deletion
commands. It does not route through the generic GCP lane or the Box facade.

### 2.2 The worker comes only from admitted OpenAgents Cloud supply

The initial placement is deliberately closed:

- target class: `openagents_managed`;
- provider: `google_cloud`;
- adapter: `adapter.oa-codex-control.gce.v1`;
- isolation: one disposable `gce_vm` per run;
- worker driver: `driver.openagents.forensic-worker.v1`;
- network: broker-only; and
- profile: an image- and digest-pinned forensic GCE profile.

Local macOS execution, a contributor Pylon, a fake provisioner, Box ownership,
generic remote Linux, another cloud, or an unobserved fallback is a refusal—not
a degraded placement. The renderer must show `Awaiting OpenAgents managed
profile` until that exact admission exists.

### 2.3 Source completeness comes before inference

Episode 264 established that the complete Coldcard dependency graph changes the
answer. The source materializer therefore resolves the commit, Git tree,
declared submodules, observed gitlinks, generated inputs, and dependency policy
before prompt dispatch.

Coverage is `complete`, `incomplete`, pending, or denied. An incomplete research
run can continue only after explicit acknowledgment. It retains that state in
every result and never enters the qualified-identification denominator. Missing
or oversized required input cannot appear as a clean or comprehensive result.

### 2.4 Findings, hypotheses, and prose are different lanes

Diagnostic prose is retained only as diagnostics. A finding exists only after
the strict `submit_forensic_finding` tool lane accepts it. An uncertain lead
uses `submit_forensic_hypothesis` and must name missing evidence and the next
check. Neither prose length nor confidence wording raises an evidence tier.

The independent verifier must not reuse the discovery identity. It locks one
initial verdict before executing or writing a fix. Confirmation requires the
admitted vulnerable target to fail and the fixed target to pass with exact
receipts. A PoC that merely applies is not an executed proof.

### 2.5 Cleanup is terminal truth

Silence is never completion. Cancellation observes interrupt and structural
settlement before deletion. Completion requires durable capability revocation,
process and scratch cleanup, provider deletion, and zero compute, disk,
firewall, ingress, process, and workspace residue. Missing cleanup truth yields
`recovery_required` or cleanup failure, not success.

## 3. How to use the system

There are two supported modes today: deterministic development and an admitted
live managed run.

### 3.1 Deterministic development mode

Use this mode to change contracts, prompts, metrics, fixtures, or Omega
presentation without spending cloud budget.

1. Validate the OpenAgents contracts and benchmark corpus.
2. Exercise the Loupe discovery and verifier adapters with their fake backends.
3. Exercise the managed-worker, source-materializer, prompt-compiler, and guest
   driver tests.
4. Open the synthetic Coldcard projection in Omega or run its focused GPUI
   tests.
5. Treat every fixture result as development evidence only. It is not proof
   that a production firmware build, cloud worker, or historical scan ran.

Run the core OpenAgents checks from the repository root:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
pnpm exec vp test --run \
  apps/openagents.com/workers/api/src/forensic-managed-sandbox.test.ts \
  apps/openagents.com/workers/api/src/forensic-source-materializer.test.ts \
  apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.test.ts
node --test scripts/cloud/forensic-worker-driver.test.mjs
```

Run the Omega checks from the Omega repository:

```sh
cargo test -p omega_forensics
cargo test -p agent_ui forensics_workbench --features test-support
cargo test -p omega_workbench_state -p omega_workbench_conformance
./script/clippy -p omega_forensics -p agent_ui
```

### 3.2 Admitted Omega run

Use this sequence only when the native forensic route is deployed, the owner is
authenticated, and the exact managed profile is enabled.

1. Open an Omega task bound to a Git repository and select **Forensics** from
   the workbench rail.
2. Confirm repository, remote, exact commit, source state, and dependency
   policy.
3. Select the benchmark or scan arm. Changing the arm invalidates previous
   coverage and prepared intent.
4. Wait for the admitted OpenAgents Cloud profile. Confirm GCE isolation,
   image and profile digests, region and custody refs, lease, network policy,
   model, effort, concurrency, time, token, cost, artifact, and network caps.
5. Inspect the terminal coverage manifest. Acknowledge an incomplete research
   run only when that limitation is intentional.
6. Inspect the active prompt artifact and digest. Clone it before making a
   candidate. Save and activate a candidate as a new immutable artifact; never
   mutate an artifact already bound to a run.
7. Select **Prepare run**. Preparation binds the launch intent but does not
   create a worker.
8. Select **Launch worker**. Omega admits exactly one worker and follows ordered
   events from the returned cursor.
9. Use **Refresh events** to resume observation. An empty page means only that
   no new event was returned.
10. Review findings and hypotheses separately. Open source citations only at
    the pinned commit. Inspect evidence tier, causal path, PoC receipts,
    verifier verdict, budget, lifecycle, and cleanup truth.
11. Use **Cancel and clean up** for a running turn, or **Delete and verify
    cleanup** for an idle ready worker. Do not abandon a worker in
    `recovery_required`.
12. Compare candidates only through a matched run matrix with distinct writable
    state and the same frozen benchmark population.

## 4. Prompt iteration and Blueprint promotion

The prompt is a structured `ForensicPromptIr`, not one undifferentiated string.
It covers role, threat model, vulnerability classes, invariants, evidence,
dependency exploration, uncertainty, tool policy, finding and hypothesis
schemas, PoC policy, severity, context, and budget policy.

The productive iteration loop is:

```text
active artifact
  -> clone immutable candidate
  -> edit structured IR
  -> inspect semantic diff
  -> validate tool/schema compatibility
  -> save candidate digest
  -> run matched development matrix
  -> evaluate on untouched holdout with another identity
  -> apply hard gates and Pareto comparison
  -> explicit promotion or append-only rollback
```

Do not optimize against Coldcard alone. Coldcard and its visible structural
variants are optimizer-visible development data. The vulnerable and clean
holdouts are separately owned evaluator inputs and must remain absent from
candidate generation.

Promotion is lexicographic, not a weighted score. A candidate is blocked if it
reduces hit rate, weakens causal evidence, hits a clean control, exceeds budget,
shares state, lacks independent evaluation, or fails cleanup—even if it is
faster or cheaper.

The current native Omega surface can inspect, clone, save, list, and activate
prompt candidates. Structured draft mutation exists in the workbench state
API, but the rendered surface does not yet expose the full field-level editor.
Use tests or a client of that API for field edits until the editor is completed;
do not describe the present buttons as a complete prompt-authoring experience.

## 5. Measuring improvement

Record canonical milestones rather than one undifferentiated duration:

- `T0`: admission accepted;
- `T1`: worker ready;
- `T2`: source and coverage ready;
- `T3`: model work started;
- `T4`: first valid typed candidate;
- `T5`: first qualified identification;
- `T6`: independent verification; and
- `T7`: cleanup observed.

The primary speed metrics are time and tokens to `T5`. Keep them beside—not in
place of—hit rate, all required causal links, valid source references, evidence
tier, verifier independence, false positives, clean-control results, reviewer
seconds, cancellation latency, cleanup latency, and zero-residue rate.

Eligible misses retain their full spend and a nonzero right-censor boundary.
Unavailable provider usage has no numeric value; it never becomes zero. Small
samples remain provisional, and tail latency remains not estimable until the
pre-registered sample count is met without censoring.

Use the matrix to answer these questions in order:

1. Did the candidate identify the issue on complete vulnerable and structural
   variants?
2. Did it stay quiet on fixed and clean controls?
3. Did it provide the required causal path and valid evidence?
4. Was verification independent?
5. Did every run stay within budget and clean up?
6. Only then: was identification faster or cheaper?

## 6. Reading the Omega workbench

The workbench contains these logical surfaces even though the first renderer
places them in one scrollable dock:

| Surface | Read it as |
| --- | --- |
| Target and preflight | Exact repository, commit, source completeness, placement, and bounds. |
| Prompt artifacts | Active immutable input, draft lineage, semantic changes, and candidates. |
| Managed worker | OpenAgents Cloud placement truth and current lifecycle. |
| Coverage and lifecycle | Whether the result is eligible and whether the worker settled and cleaned up. |
| Findings | Typed claims with causal links, citations, evidence receipts, PoC refs, and append-only human decisions. |
| Hypotheses | Unverified leads with explicit missing evidence and next checks. |
| Run matrix | Matched candidate populations, censored misses, confidence bounds, cost, quality gates, and Pareto status. |
| Coldcard evidence workspace | Nine-rung claim ladder, source-to-generator trace, entropy assumptions, private scan, provenance health, reconciliation, and correction history. |

The renderer receives projections, not the underlying authority. A green label
does not replace its receipt, and a selected benchmark arm does not prove that
the corresponding source bundle or worker exists.

## 7. Current limitations and next UI work

The implementation program is complete, but the first interface still has
important usability and proof gaps:

- existing tasks persisted before OFR-018 can retain a six-surface availability
  list and omit Forensics until repository identity is rebuilt; migrate or
  recompute this state;
- the prompt surface needs the full structured field editor and side-by-side
  active/candidate comparison;
- the single long dock needs sections for Setup, Prompt, Run, Evidence,
  Findings, and Reconciliation, plus sticky run, cleanup, and private-boundary
  status;
- exact hashes, evidence refs, assumptions, and reconciliation values currently
  use one-line truncation in the narrow dock; add wrapping, copy, and detail
  affordances;
- add deterministic visual-test scenes for awaiting-profile, complete and
  incomplete preflight, running, cancellation, recovery, review, matrix, and
  private Coldcard evidence states; and
- a live production or staging claim still requires the exact deployed route,
  enabled managed profile, independent admission, budget authority, and
  cleanup evidence. The checked-in fixtures do not supply those facts.

## 8. Safety and disclosure boundary

- Run public third-party analysis privately with `manual_no_reporting`.
- Never let a worker file an issue, email a maintainer, publish a post, or move
  from evidence to disclosure without separate authority and human review.
- Use only synthetic or explicitly owner-authorized fixtures for seed or wallet
  reproduction. Persist no mnemonic, xprv, seed phrase, node cookie, or RPC
  credential.
- A source flaw does not prove a shipped artifact. A generator match does not
  prove a wallet. A transaction fingerprint does not prove a vendor, person,
  intent, theft, or identity.
- Keep original model output and every correction append-only. Review changes
  the disposition; it never rewrites what the model emitted.
- Refuse an unknown-key search or live-value recovery. Those are outside the
  development program and outside this guide.

## 9. Related documents

- [Project Loupe reference study](README.md)
- [Coldcard Loupe experiment results](2026-08-01-coldcard-prefix-experiment-results.md)
- [Forensic prompt optimization governance](2026-08-01-forensic-prompt-optimization-governance.md)
- [Coldcard evidence derivation and claim history](2026-08-01-coldcard-evidence-derivation.md)
- [Coldcard historical fingerprint scan](2026-08-01-coldcard-historical-fingerprint-scan.md)
- [OpenAgents forensic contracts](../../packages/forensic-contract/README.md)
- [OpenAgents Loupe adapter](../../packages/forensic-loupe-adapter/README.md)
- [Coldcard development benchmark](../../fixtures/forensics/coldcard/README.md)
- [Omega Forensics preflight](https://github.com/OpenAgentsInc/omega/blob/main/docs/src/development/omega-forensics-preflight.md)
- [Omega Forensics cloud control](https://github.com/OpenAgentsInc/omega/blob/main/docs/src/development/omega-forensics-cloud-control.md)
- [Omega Forensics review](https://github.com/OpenAgentsInc/omega/blob/main/docs/src/development/omega-forensics-review.md)
- [Omega Forensics run matrices](https://github.com/OpenAgentsInc/omega/blob/main/docs/src/development/omega-forensics-run-matrices.md)
- [Omega Coldcard evidence views](https://github.com/OpenAgentsInc/omega/blob/main/docs/src/development/omega-coldcard-evidence-views.md)
