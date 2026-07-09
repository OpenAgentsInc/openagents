# `openagents.artanis_bootstrap_assignment.v1`

Status: SHC Codex bootstrap scaffold

This contract lets Vortex or an approved operator launch one bounded Artanis
bootstrap workroom on managed SHC infrastructure. It is not a public Artanis
chat transcript and it is not a Pylon wallet path. It turns the relevant
historical Artanis policy/code into a private Codex workroom that produces the
next Pylon launch plan, continual-learning plan, signature-mining plan, and
work-order drafts.

## Boundary

Artanis source material lives across the root workspace docs, Vortex public
projection docs, and the deprecated Autopilot4 Rust source. This contract pulls
in the useful pieces:

- Artanis identity, objective, and instruction source refs.
- capability labels for trainer, support, evaluator, and integrity work;
- Program/Blueprint policy signatures;
- GitHub repository allowlist and staged-write posture;
- health gates, dispatch blockers, recovery commands, launch checks, and
  promotion evidence gates;
- public projection/redaction rules.

The workroom remains private. Public Artanis output must be created later as a
redacted projection after retained evidence exists.

## Assignment Fields

| Field | Purpose |
| --- | --- |
| `bootstrap_run_id` | Stable Cloud/Vortex run id and local job id. |
| `workroom_id` | Private no-wallet workroom id. |
| `target_node_id` | First target is `oa-shc-katy-01`. |
| `operator_ref` / `organization_ref` | Vortex-owned operator scope. |
| `provider_account_ref` | Sanitized ChatGPT/Codex account ref. |
| `auth_grant_ref` | Short-lived Vortex grant for this Codex run. |
| `repository_refs` | Repos the workroom should inspect or write drafts for. |
| `source_refs` | Artanis source docs/code to import conceptually. |
| `objective_id` / `objective_summary` | Bounded Artanis bootstrap objective. |
| `pylon_launch_id` | Target Pylon launch planning lane. |
| `settlement_intent` | Optional no-wallet settlement metadata for Pylon paid-work traceability. |
| `pylon_capability_labels` | Capability labels to seed routing and issue drafts. |
| `blueprint_signature_ids` | Program/signature ids to load into Codex context. |
| `budget` | Timeout, attempts, and optional max cost. |
| `retention_mode` / `artifact_sink_ref` | Artifact retention policy. |
| `required_artifacts` | Local artifact filenames Codex must create. |
| `wallet_authority` | Must be `false`. |

`settlement_intent`, when present, contains only public-safe identifiers:

- `artanis_run_id`, which must equal `bootstrap_run_id`;
- `artanis_assignment_id`;
- `settlement_intent_id`;
- optional `public_receipt_id`.

It does not grant wallet authority, payment authority, or checkout authority.
Its purpose is to let the Artanis workroom and any Pylon NIP-90 job it creates
reuse the same id chain. Pylon accepts those ids as:

- `oa:artanis_run_id`
- `oa:artanis_assignment_id`
- `oa:settlement_intent_id`

## Endpoint

```text
POST /v1/artanis/bootstrap/start
```

The route validates the assignment, translates it into the existing async Codex
run envelope, persists `artanis-bootstrap-assignment.json` in the job directory,
and emits these initial events:

- `artanis.bootstrap.validated`
- `artanis.capability_context.loaded`
- `artanis.artifact_policy.attached`

The Codex worker then runs through the same account-backed SHC workroom path as
normal `/v1/codex-runs/start` jobs.

## Required Artifacts

The fixture uses:

- `result.md`
- `artanis-source-map.json`
- `pylon-launch-plan.json`
- `continual-learning-plan.json`
- `signature-mining-plan.json`
- `work-order-drafts.json`
- `artifact-manifest.json`
- `proof-bundle.json`

These are enough for the next Vortex/Admin surface to show a mission briefing
and for subsequent agents to open issue-sized implementation work.

## Safety Rules

- No raw Codex credentials, API keys, provider tokens, wallet material, private
  repo contents, raw shell logs, or local paths in artifacts meant for public
  projection.
- `wallet_authority` must be `false`.
- `settlement_intent` is identifier metadata only. It must not contain raw
  invoices, preimages, wallet config, access tokens, or private payment state.
- Credential use is only through `provider_account_ref` plus `auth_grant_ref`.
- `danger_full_access` is acceptable only because the SHC VM/workroom is the
  external sandbox and receives no wallet authority or broad cloud credentials.
- Public Artanis claims remain blocked until benchmark, rollback, and receipt
  evidence exists.

## Fixture

```text
fixtures/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json
```

## Verification

```bash
cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates
cargo test -p oa-codex-control artanis_bootstrap
```
