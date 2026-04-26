# Nexus Health Agent Runner

`nexus-health-agent` is the cloud-worker entrypoint for the Autopilot/Forge
health loop. The default path is monitor-only: it observes Nexus, classifies
the result, emits redacted evidence, and writes Forge health work-orders/events.
It can now also record and execute bounded recovery actions when the action is
explicitly requested, the required Forge lease is present, and post-action
verification runs before the report claims recovery success.

## Entry Point

Build or run the worker from the `openagents` repo:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
```

The command emits one JSON report. The report includes:

- `snapshot`: deterministic Nexus health snapshot from the existing classifier.
- `evidence_artifacts`: redacted inline `nexus.health.snapshot` evidence plus a
  stable SHA-256 digest.
- `forge_work_order_request`: the planned or submitted Forge
  `POST /v1/health/work-orders` body.
- `forge_event_request`: the planned or submitted Forge
  `POST /v1/health/events` body.
- `action_plan`: normalized action kind, safety class, resource, lease, and
  approval requirements.
- `action_results`: bounded execution result plus post-action verification
  status.
- `scheduler`: hosted scheduler/cycle metadata, including the expected
  detection window and stale-after threshold for Forge/openagents.com
  projections.
- `external_reachability`: the public-edge probe result from the worker's
  vantage, including route failures and Cloudflare error codes.
- `forge_writes`: dry-run, fake, or live write results.
- `redaction`: booleans proving secret-shaped keys and strings are absent from
  the emitted report.

## Modes

Use fake Nexus plus dry-run for local parser and report validation:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
```

Use live Nexus but no Forge writes when checking public service health from a
local or cloud shell:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --pretty
```

Use fake Forge integration in tests or local harnesses when validating the
write path without a real Forge service:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- \
  --fake-nexus \
  --fake-forge \
  --pretty
```

Use live Forge only from a protected cloud-worker environment with the service
bearer and actor context injected as secrets:

```shell
NEXUS_HEALTH_AGENT_FORGE_BASE_URL="https://forge.internal.example" \
NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN="<redacted>" \
NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT="<redacted>" \
cargo run -p nexus-control --bin nexus-health-agent -- --pretty
```

Do not put those secret values in tracked files, issue comments, or logs.

The hosted GCP deployment lane is documented in:

- `docs/deploy/NEXUS_HEALTH_RUNNER_GCP_RUNBOOK.md`

Autopilot now also exposes the same classifier-derived status locally:

- Native view: open the Tauri Autopilot shell and choose `View > Health`.
- CLI: `cargo run -p autopilot --bin autopilotctl-tauri -- --json health nexus status`.
- CLI fallback/proof without a running Tauri control plane:
  `cargo run -p autopilot --bin autopilotctl-tauri -- health nexus status --fake --json`.

That Autopilot projection is intentionally operator-facing and redacted. It
shows subsystem state, active training run, queued follow-ups, stop/cancel
state, latest action, failed predicates, verification gates, and a timeline. It
must not expose raw bearer tokens, wallet secrets, stack traces, `sync stale`,
or vague user-facing status labels without the exact failed predicate.

That lane runs the binary as both a Cloud Run Job and a warm Cloud Run Service
with an attached service account and Secret Manager injection. The Job remains
the manual smoke and leased-action path. The Service is the recurring monitor
path because it keeps one warm instance and avoids Cloud Run Job provisioning
latency/backlog. A first production read probe can run with `--dry-run,--json`
and no Forge secrets attached. Live Forge writes need the Forge bearer and
actor-context secrets. Live `treasury_refresh` additionally needs the scoped
Nexus admin bearer secret. Hosted continuous monitoring is installed by
`scripts/deploy/nexus/21-deploy-health-runner-service.sh` plus
`scripts/deploy/nexus/20-deploy-health-runner-scheduler.sh`, which uses Cloud
Scheduler with OIDC to invoke the Service `/run` endpoint from GCP rather than
from an operator laptop.

## Environment

- `NEXUS_HEALTH_AGENT_NEXUS_BASE_URL`: defaults to
  `https://nexus.openagents.com`.
- `NEXUS_HEALTH_AGENT_TIMEOUT_MS`: defaults to `15000`.
- `NEXUS_HEALTH_AGENT_FORGE_BASE_URL`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_PROJECT_ID`: defaults to `openagents`.
- `NEXUS_HEALTH_AGENT_ACTOR_ID`: defaults to `nexus-health-agent`.
- `NEXUS_HEALTH_AGENT_ACTION_KIND`: defaults to `monitor`.
- `NEXUS_HEALTH_AGENT_FORGE_LEASE_ID`: required for mutating actions.
- `NEXUS_HEALTH_AGENT_APPROVAL_ID`: required for unsafe approval-gated actions.
- `NEXUS_HEALTH_AGENT_ACTION_REASON`: optional human-readable action reason.
- `NEXUS_HEALTH_AGENT_NEXUS_ADMIN_BEARER_TOKEN`: required only for live
  `treasury_refresh`.
- `NEXUS_HEALTH_AGENT_SCHEDULER_NAME`: defaults to `manual`; hosted jobs set
  this to the Cloud Scheduler job name.
- `NEXUS_HEALTH_AGENT_SCHEDULER_INTERVAL_SECONDS`: defaults to `60`.
- `NEXUS_HEALTH_AGENT_CYCLES`: defaults to `1`; launch periods can use `2`
  with a 30-second cycle interval when Cloud Scheduler is firing once per
  minute.
- `NEXUS_HEALTH_AGENT_CYCLE_INTERVAL_SECONDS`: defaults to `0`; set to `30`
  for two probes inside each scheduled minute.
- `NEXUS_HEALTH_AGENT_EXTERNAL_VANTAGE_ID`: defaults to `local`; hosted jobs
  use a GCP/Cloud Run vantage id so public-edge failures are distinguishable
  from VM-local checks.
- `NEXUS_HEALTH_AGENT_SERVER_ARGS`: Cloud Run Service wrapper args, defaulting
  to `--dry-run,--json`; the server forces a single monitor cycle per request.

CLI flags with the same meaning override the non-secret defaults:

```text
--nexus-base-url <url>
--forge-base-url <url>
--timeout-ms <ms>
--project-id <id>
--actor-id <id>
--action-kind <kind>
--forge-lease-id <id>
--approval-id <id>
--action-reason <text>
--scheduler-name <name>
--scheduler-interval-seconds <seconds>
--cycles <count>
--cycle-interval-seconds <seconds>
--external-vantage-id <id>
--fake-nexus
--fake-forge
--dry-run
--pretty
--json
```

## Forge Contract

The worker writes the APIs introduced for the health-agent program:

- `POST /v1/health/work-orders`
- `POST /v1/health/events`

Monitor work uses `nexus.health.monitor`. Recovery work uses the closest
health kind for the requested action, such as `nexus.health.recover` or
`nexus.treasury.verify`. The event payload contains redacted evidence
references, classifier facts, the normalized action plan, and action results.
It also contains the scheduler metadata and external reachability report so the
admin UI and agent API can show whether the hosted loop is fresh and whether
the failure was observed from the public edge.
If Nexus public endpoints fail, the worker still produces a Forge event plan or
write result instead of crashing; the event class/resource is derived from the
failed predicate or requested action resource, such as `nexus-cloudflared`,
`nexus-treasury-wallet`, or `nexus-training-dispatcher`.

## Hosted Scheduler

GCP Cloud Scheduler is minute-granularity. The recurring path should target the
warm Cloud Run Service, not the Cloud Run Job, because Cloud Run Job startup can
take longer than the desired detection window under load. Deploy the service and
then point Scheduler at the service URL:

```bash
scripts/deploy/nexus/21-deploy-health-runner-service.sh

SERVICE_URL="$(gcloud run services describe nexus-health-runner-service \
  --project openagentsgemini \
  --region us-central1 \
  --format 'value(status.url)')"

NEXUS_HEALTH_RUNNER_SCHEDULER_AUTH_MODE=oidc \
NEXUS_HEALTH_RUNNER_SCHEDULER_URI="${SERVICE_URL}/run" \
NEXUS_HEALTH_RUNNER_SCHEDULER_OIDC_AUDIENCE="${SERVICE_URL}" \
scripts/deploy/nexus/20-deploy-health-runner-scheduler.sh
```

The Job path is still useful for a manual smoke or leased one-shot action:

```bash
NEXUS_HEALTH_RUNNER_JOB_ARGS='--dry-run,--json' \
scripts/deploy/nexus/18-deploy-health-runner-job.sh
scripts/deploy/nexus/19-smoke-health-runner-job.sh
```

Both paths record `scheduler.max_expected_detection_seconds` and
`external_reachability.vantage_id` in every health event.

## Recovery Boundary

Action kinds:

- `monitor`: default observe-only path.
- `reprobe`, `vm_local_compare`, `collect_diagnostics`: read-only actions.
- `treasury_refresh`, `restart_nexus_cloudflared`, `restart_nexus_relay`:
  mutating recovery actions that require a Forge controller lease.
- `vm_reset`, `startup_script_recovery`, `deploy_image`,
  `payout_policy_change`, `funding_invoice_create`: unsafe actions that require
  both a Forge controller lease and an approval id.

`treasury_refresh` can call `POST /v1/admin/treasury/refresh` when a scoped
Nexus admin bearer token is injected. Service restarts and VM/deploy mutations
are intentionally routed through a Forge-leased Probe/GCP executor; this worker
records the controlled action and verification contract instead of running
local laptop shell commands.

Report status is deliberately conservative. A dry run is
`recovery_dry_run_planned`; a queued Probe/GCP action is
`recovery_action_queued`; an approval-gated action without execution is
`recovery_approval_required`; a recovery is not reported as `completed` unless
the action result and post-action verification pass.

## Verification

Focused verification for this runner:

```shell
cargo test -p nexus-control health_agent -- --nocapture
cargo test -p nexus-control health_verification -- --nocapture
cargo run -p nexus-control --bin nexus-control -- health verify --fake --pretty
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
cargo run -p nexus-control --bin nexus-health-agent -- \
  --action-kind treasury_refresh \
  --forge-lease-id forge-lease-dry-run \
  --dry-run \
  --fake-nexus \
  --pretty
cargo check -p nexus-control --bins
git diff --check
```

Expected tests cover:

- fake Nexus plus dry-run report generation.
- fake Forge integration for work-order and event append calls.
- public endpoint failure handling.
- hosted scheduler and external vantage metadata.
- Cloudflare 1033-style public-edge failure classification.
- Pylon homework-worker eligibility telemetry: online presence, admitted
  homework workers, and presence-only blocker reasons must be visible in the
  health verification evidence.
- no sensitive-shaped keys or strings in emitted reports.
- lease requirement for mutating action plans.
- approval-id requirement for unsafe action plans.
- recovery dry-run action evidence and post-action verification contract.
- fake Forge event/write assertions for leased recovery actions.

For Probe or Forge evidence on Nexus fixes, attach the
`nexus-control health verify` JSON from `docs/NEXUS_HEALTH_VERIFICATION_PACK.md`.
It packages required health gates, advisory payout/training/deploy checks, and
redacted inline evidence without granting mutation authority.

Hosted deploy-lane verification:

```shell
scripts/deploy/nexus/test-health-runner-deploy-shell-guards.sh
```

Expected deploy guards cover Cloud Run Job/Service service-account attachment,
Secret Manager binding, OAuth Job scheduler plans, OIDC Service scheduler
plans, dry-run GCP command plans, and startup log secret-scan wiring.

For a hosted treasury-refresh recovery proof:

1. Ensure the health-runner service account has Secret Manager access to the
   scoped Nexus admin bearer secret.
2. Deploy with the admin secret attached and a leased recovery action:

```bash
NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET=true \
NEXUS_HEALTH_RUNNER_JOB_ARGS='--action-kind,treasury_refresh,--forge-lease-id,<forge_lease_id>,--json' \
scripts/deploy/nexus/18-deploy-health-runner-job.sh
```

3. Execute the job and confirm the report contains a
   `nexus.health.recovery_action` evidence artifact and a post-action
   verification status. Do not treat the incident as closed unless that
   verification status passed.
