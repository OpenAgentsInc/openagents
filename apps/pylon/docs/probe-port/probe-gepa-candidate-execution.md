# Probe GEPA Candidate Execution

Date: 2026-06-08

Status: implemented for Probe issue #185.

Probe now has a runtime-local adapter for running a retained benchmark
assignment with either the baseline assignment text or a supplied Psionic GEPA
text-bundle candidate manifest. The adapter lives in
`packages/runtime/src/benchmark/candidate-execution.ts` and is exported from the
runtime package entry point.

## Current Shape

`runProbeRetainedBenchmarkCandidate` consumes:

- a Probe benchmark assignment;
- a retained Terminal-Bench fixture;
- an optional `psionic.probe_gepa_candidate_manifest.v1` candidate manifest;
- optional selected Blueprint signature refs;
- optional projected tool refs.

The adapter decodes the assignment, validates the candidate manifest, projects
the candidate hash and Probe import refs into an assignment clone, and delegates
normalized artifact production to `makeProbeBenchmarkCloseoutBundle`.

This is not yet the live sandbox runner. It is the typed execution seam that
Benchmark Cloud and Psionic can call before the later sandbox execution layer is
plugged in. Baseline and candidate runs already produce the same normalized
closeout bundle file set, so score import, retained-failure handling, and GEPA
frontier comparison can be developed against stable output.

## Candidate Safety

Probe validates candidate text before it can affect a closeout. Candidate
components cannot contain credential-shaped text, private repository refs,
release-gate bypass instructions, public-claim upgrade authority, or requests
for new runtime authority.

The candidate safety boundary must keep:

- `no_new_runtime_authority: true`;
- non-empty inherited runtime authority refs;
- `public_claim_upgrade_authority: false`.

Probe treats the candidate as an optimization artifact. It can change prompt and
playbook text used for benchmark rollouts, but it cannot grant itself Blueprint
authority, public claim authority, or runtime promotion authority.

## Blueprint And Tool-Menu Boundary

Candidate-selected signatures are checked against the assignment-selected
Blueprint Program Signature refs and the retained fixture requirements. A
candidate cannot select a signature outside the assignment, and it cannot drop a
fixture-required signature.

Projected tool refs are checked against the retained fixture tool-menu
constraints. The projection must include required tools, must only contain
allowed tools, and must not include denied tools. The closeout keeps the
assignment `toolMenuRef` as the authority and records the projected menu as a
snapshot.

## Closeout Emission

Candidate closeouts include:

- candidate hash in `probe-closeout.json`;
- selected Blueprint signatures;
- assignment tool-menu ref plus projected tool-menu snapshot;
- Probe candidate import refs in `candidate-ref.json`;
- candidate component refs derived from component hashes;
- verifier result refs in `artifact-refs.json`;
- failure classification and retained-failure refs;
- policy findings for candidate import boundary checks.

The closeout writer now accepts optional `candidateComponentRefs` and
`verifierResultRefs` and records them inside the existing bundle file set
without adding a new schema-breaking file.

## Tests

`packages/runtime/tests/benchmark-candidate-execution.test.ts` covers:

- baseline and supplied GEPA candidate runs against the same retained fixture;
- candidate hash and candidate refs in closeout output;
- candidate component refs and verifier result refs;
- rejection of candidate text that requests new Blueprint/runtime authority;
- rejection of projected tool-menu refs outside policy;
- rejection of candidate-selected signatures outside assignment authority.

The full runtime suite currently passes with `bun test`.
