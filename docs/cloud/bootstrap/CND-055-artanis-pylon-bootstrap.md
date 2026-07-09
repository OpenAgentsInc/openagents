# CND-055: Artanis Pylon Launch Bootstrap

Date: 2026-06-02

## Goal

Use the improved Codex workroom path to turn the existing Artanis source
material into a bounded SHC-backed bootstrap run for the next Pylon launch and
the continual-learning loop.

The useful Artanis code was not copied wholesale into Cloud. The deprecated
Autopilot4 source is treated as policy/reference material. Cloud now owns the
private execution envelope that can ask Codex to inspect that material and emit
launch artifacts under the same no-wallet, credential-scoped SHC workroom rules
as the normal Autopilot coding runs.

## Imported Artanis Concepts

Relevant source refs:

- `workspace:agents/training-program-maintenance-agent.md`
- `workspace:docs/2026-05-22-artanis-fake-projection-to-live-agent-gap-audit.md`
- `vortex:docs/public-agents-artanis.md`
- `autopilot4-deprecated:src/artanis.rs`
- `autopilot4-deprecated:src/benchmark_release_gates.rs`

Imported concepts:

- Artanis identity as public training-program overseer;
- private workroom first, public projection second;
- instruction source/version refs;
- active objective and pylon launch lane;
- Pylon capability labels for trainer, support, evaluator, and integrity work;
- Program policy ids for context selection, work selection, capability
  matching, dispatch risk, promotion readiness, and next action;
- health gates, dispatch blockers, recovery commands, and launch checks;
- retained benchmark, rollback, and receipt requirements before any public
  promotion claim.

## What Was Added

Cloud now defines `openagents.artanis_bootstrap_assignment.v1` with a fixture:

```text
fixtures/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json
```

`oa-codex-control` now accepts:

```text
POST /v1/artanis/bootstrap/start
```

The endpoint:

1. validates the Artanis bootstrap assignment;
2. translates it into a normal async Codex run;
3. queues the run on SHC with `danger_full_access` only inside the external
   no-wallet VM/workroom boundary;
4. persists `artanis-bootstrap-assignment.json` next to the job state;
5. emits initial public-safe Cloud events:
   - `artanis.bootstrap.validated`
   - `artanis.capability_context.loaded`
   - `artanis.artifact_policy.attached`
   - `artanis.settlement_intent.attached` when the assignment carries
     public-safe Artanis/Pylon settlement trace ids
6. requires Codex to create:
   - `result.md`
   - `artanis-source-map.json`
   - `pylon-launch-plan.json`
   - `continual-learning-plan.json`
   - `signature-mining-plan.json`
   - `work-order-drafts.json`
   - `artifact-manifest.json`
   - `proof-bundle.json`

## Continual-Learning Shape

The first bootstrap prompt asks Codex to produce a concrete loop:

```text
failed or partial Codex/Pylon trace
  -> classify failure family
  -> select Blueprint/Probe signature
  -> run retained replay/eval
  -> compare raw backend against signature-backed route
  -> record improvement receipt
  -> update signature pack or work-order drafts
  -> project only redacted public-safe Artanis progress
```

This connects directly to the benchmark improvement work from `CND-054`, where
retained Terminal-Bench fixtures moved from raw Codex mean reward `0.000` to an
expected Probe+signature mean reward `0.900` across seven retained failures.

## Credential Boundary

The assignment uses only:

- `provider_account_ref`
- `auth_grant_ref`
- optional `settlement_intent` public-safe ids

The Cloud runner resolves those through the existing Vortex grant path and the
VM-local account-scoped Codex home. It does not accept or store raw ChatGPT,
Codex, OpenAI API, GitHub, wallet, or cloud credentials in the assignment.

`settlement_intent` is identifier metadata only. It does not grant wallet
authority or payment authority. When present, the Artanis prompt tells the
workroom to reuse those ids in Pylon structured NIP-90 requests, where Pylon
projects them as `oa:artanis_run_id`, `oa:artanis_assignment_id`, and
`oa:settlement_intent_id` tags for later MDK settlement receipt traceability.

## Closeout Behavior

The SHC runner treats the declared artifact list as the bounded execution
contract. When every required artifact exists in the Codex workspace and stays
stable for a short grace period, `oa-workroomd` can close the run, capture the
artifacts, and emit an `artifact_set.completed` runner event even if Codex
would otherwise continue composing a final chat message.

This keeps Autopilot and Artanis bootstraps tied to durable outputs instead of
waiting on open-ended model prose. Missing required artifacts still block
closeout, and timeout/failure paths remain terminal when the artifact set is
not complete.

## Programmatic Smoke

The new fake-workroomd test proves the path without real credentials:

```bash
cargo test -p oa-codex-control artanis_bootstrap
```

The fake runner validates that the endpoint translation can complete, persist
the Artanis assignment, emit Artanis context events, capture artifacts, and
retain the usage-unavailable event for subscription-backed Codex accounting.

The contract test proves the fixture validates and rejects wallet authority or
secret-like text:

```bash
cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates
```

## Next Vortex Work

Vortex should add an admin/operator action that posts the fixture-shaped body to
the Cloud endpoint with a fresh `auth_grant_ref` from the connected admin
ChatGPT/Codex account. The resulting run can render in the same mission UI as
other Codex workrooms, but the top-level briefing should label it as:

```text
Artanis Pylon Launch Bootstrap
```

The first real run should be treated as private evidence. Public Artanis
projection updates should use only redacted summaries and retained artifact or
receipt refs after the workroom closes.
