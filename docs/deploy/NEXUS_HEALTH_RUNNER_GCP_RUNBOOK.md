# Nexus Health Runner GCP Runbook

Date context: April 26, 2026.

This runbook defines the first hosted GCP lane for `nexus-health-agent`. The
goal is to remove production dependence on an operator laptop's short-lived
`gcloud` OAuth session. The runtime identity is a Cloud Run Job service
account, and hosted secrets come from GCP Secret Manager.

## Runtime Model

- Build artifact: the existing `nexus-relay` container image now includes
  `/usr/local/bin/nexus-health-agent`.
- Hosted runtime: Cloud Run Job named `nexus-health-runner`.
- Runtime identity: service account
  `nexus-health-runner@openagentsgemini.iam.gserviceaccount.com` by default.
- Public read target: `https://nexus.openagents.com`.
- Forge write target: configured through `NEXUS_HEALTH_RUNNER_FORGE_BASE_URL`
  for live health-event writes.
- Recovery actions: default `monitor`; leased actions can be selected through
  `NEXUS_HEALTH_RUNNER_JOB_ARGS`.
- Secret source: GCP Secret Manager. Do not put human refresh tokens, bearer
  tokens, or wallet material in Laravel env, Cloud Run literal env, docs,
  issue comments, or logs.

The first safe production proof can be a public-read-only Cloud Run Job:

```bash
NEXUS_HEALTH_RUNNER_JOB_ARGS='--dry-run,--json' \
NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false \
scripts/deploy/nexus/18-deploy-health-runner-job.sh
scripts/deploy/nexus/19-smoke-health-runner-job.sh
```

That proves the hosted service account can execute the monitor probe from GCP
without using a local operator OAuth token at runtime. The live Forge-writing
mode should attach Forge secrets and omit `--dry-run` only after the Forge
service URL and scoped service credentials are provisioned.

## Scripts

All scripts live under `scripts/deploy/nexus/`.

1. Build/push the container image that contains `nexus-health-agent`.

```bash
scripts/deploy/nexus/01-build-and-push-image.sh
```

2. Provision the hosted identity and Secret Manager bindings.

```bash
scripts/deploy/nexus/17-provision-health-runner-identity.sh
```

3. Deploy the Cloud Run Job.

```bash
scripts/deploy/nexus/18-deploy-health-runner-job.sh
```

4. Execute the job once and scan startup logs for secret-shaped material.

```bash
scripts/deploy/nexus/19-smoke-health-runner-job.sh
```

Every script supports a no-mutation plan mode:

```bash
NEXUS_HEALTH_RUNNER_DRY_RUN=true \
NEXUS_HEALTH_RUNNER_SECRET_SMOKE_ENABLED=true \
scripts/deploy/nexus/17-provision-health-runner-identity.sh

NEXUS_HEALTH_RUNNER_DRY_RUN=true \
NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false \
NEXUS_HEALTH_RUNNER_JOB_ARGS='--dry-run,--json' \
scripts/deploy/nexus/18-deploy-health-runner-job.sh

NEXUS_HEALTH_RUNNER_DRY_RUN=true \
scripts/deploy/nexus/19-smoke-health-runner-job.sh
```

## Config

Defaults come from `scripts/deploy/nexus/common.sh`.

Important variables:

- `NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_NAME`: defaults to
  `nexus-health-runner`.
- `NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL`: derived from the project and
  service-account name by default.
- `NEXUS_HEALTH_RUNNER_JOB`: defaults to `nexus-health-runner`.
- `NEXUS_HEALTH_RUNNER_IMAGE`: defaults to the current Nexus image.
- `NEXUS_HEALTH_RUNNER_NEXUS_BASE_URL`: defaults to the public Nexus URL.
- `NEXUS_HEALTH_RUNNER_FORGE_BASE_URL`: required for live Forge writes.
- `NEXUS_HEALTH_RUNNER_JOB_ARGS`: defaults to `--json`; use
  `--dry-run,--json` for a public-read-only proof job.
- `NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS`: defaults to `true`; set to
  `false` for the first read-only smoke job.
- `NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET`: defaults to `false`; set to
  `true` only for leased `treasury_refresh` jobs that need the scoped Nexus
  admin bearer token.
- `NEXUS_HEALTH_RUNNER_SECRET_FORGE_BEARER_TOKEN`: Secret Manager name for the
  Forge service bearer token.
- `NEXUS_HEALTH_RUNNER_SECRET_FORGE_ACTOR_JWT`: Secret Manager name for the
  Forge actor JWT.
- `NEXUS_HEALTH_RUNNER_SECRET_NEXUS_ADMIN_BEARER_TOKEN`: Secret Manager name
  for the scoped Nexus admin bearer token used by live `treasury_refresh`.

## IAM

Reader/monitor role, enabled now:

- Cloud Run Job attached service account:
  `nexus-health-runner@<project>.iam.gserviceaccount.com`.
- Project role `roles/logging.logWriter`.
- Project role `roles/monitoring.metricWriter`.
- Secret-level `roles/secretmanager.secretAccessor` only on the named health
  runner secrets.

Recoverer role, not granted by this runbook:

- Future recovery actions must be leased through Forge first.
- GCP mutation permissions should be split into a separate recoverer service
  account or narrowly bound conditional role.
- Service restart, Cloudflare tunnel repair, and VM mutation rights should not
  be added to the monitor service account by default.

Treasury role:

- Treasury wallet material does not belong in this Cloud Run Job.
- `treasury_refresh` may attach only the scoped Nexus admin bearer secret and
  only with a Forge controller lease id in the job args.
- Larger financial controls, payout policy changes, and funding-invoice
  creation remain approval-gated and should route through Forge/Probe or the
  private `treasury` service, not this monitor identity.

Forbidden patterns:

- no service-account key files
- no human `gcloud` refresh tokens in runtime env
- no wallet mnemonic or payment preimage in Cloud Run env
- no raw bearer tokens in docs, issue comments, or startup logs

## Proof Checklist

Run the local guards first:

```bash
scripts/deploy/nexus/test-health-runner-deploy-shell-guards.sh
```

Expected proof:

- dry-run identity plan prints service account, IAM, and Secret Manager commands
  with secret values redacted.
- IAM verification commands exist for project roles and secret access.
- deploy dry-run prints a Cloud Run Job using `--service-account`, not a key.
- deploy dry-run can optionally attach the scoped Nexus admin secret for a
  leased treasury-refresh action without printing the secret value.
- smoke dry-run prints the job execution and log read commands.
- Dockerfile includes `/usr/local/bin/nexus-health-agent`.

For a hosted read-only proof:

1. Build and push an image that contains the current `nexus-health-agent`.
2. Provision identity and secrets with `17-provision-health-runner-identity.sh`.
3. Deploy a read-only job:

```bash
NEXUS_HEALTH_RUNNER_JOB_ARGS='--dry-run,--json' \
NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false \
scripts/deploy/nexus/18-deploy-health-runner-job.sh
```

4. Execute the job:

```bash
scripts/deploy/nexus/19-smoke-health-runner-job.sh
```

5. Confirm the execution completed and the smoke script reports that startup
   log secret scan passed.

For a live Forge-writing proof:

1. Add current scoped Forge secret versions in Secret Manager.
2. Set `NEXUS_HEALTH_RUNNER_FORGE_BASE_URL`.
3. Use default `NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=true`.
4. Deploy without `--dry-run` in `NEXUS_HEALTH_RUNNER_JOB_ARGS`.
5. Execute and confirm Forge received a redacted `nexus.health.monitor`
   work-order/event.

For a leased treasury-refresh proof:

1. Ensure Forge has granted/recorded the controller lease id for
   `nexus-treasury-wallet`.
2. Attach the scoped Nexus admin secret and deploy the action:

```bash
NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET=true \
NEXUS_HEALTH_RUNNER_JOB_ARGS='--action-kind,treasury_refresh,--forge-lease-id,<forge_lease_id>,--json' \
scripts/deploy/nexus/18-deploy-health-runner-job.sh
```

3. Execute with `19-smoke-health-runner-job.sh`.
4. Confirm the emitted report has `mode=forge_leased_recovery`,
   `work_order_kind=nexus.treasury.verify`, a
   `nexus.health.recovery_action` evidence artifact, and a post-action
   verification status. Do not call the incident closed unless verification
   passed.

Do not close health-runner deployment work as complete if the only proof is a
local laptop command. The durable proof is a Cloud Run execution using the
attached service account.
