# OpenAgents Runtime Operations

Operational procedures for day-to-day runtime management and incident response.

## 1. Daily checks

```bash
kubectl -n <NAMESPACE> get pods -l app=runtime
kubectl -n <NAMESPACE> get hpa runtime
kubectl -n <NAMESPACE> get job runtime-smoke
```

Expected outcome:
- Pods are healthy.
- HPA is stable.
- Latest smoke run is `Complete`.

## 2. Observability surfaces

- Dashboard asset:
  - `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
- Alert rules:
  - `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- Alert thresholds/runbook:
  - `apps/runtime/docs/OPERATIONS_ALERTING.md`

## 3. Incident triage checklist

1. Identify active alerts and affected timeframe.
2. Check runtime pod health/restarts.
3. Inspect migration/smoke jobs for recent deploy regressions.
4. Check stream/session, lease, tool, and policy telemetry dimensions.
5. Capture run IDs and trace/request IDs for forensic analysis.

## 4. Common incidents

### A. Stream completion ratio drops

Commands:

```bash
kubectl -n <NAMESPACE> logs -l app=runtime --since=15m | rg "stream"
```

Actions:
- Check `stream.session` outcomes (`tail_timeout` vs `client_closed`).
- Validate run terminal event flow (`run.finished`).
- Inspect cancel storm behavior.

### B. Lease steals spike

Commands:

```bash
kubectl -n <NAMESPACE> logs -l app=runtime --since=15m | rg "lease"
```

Actions:
- Check pod restart churn.
- Validate janitor cycle and stale lease handling.
- Confirm BEAM distribution/network policy integrity.

### C. Tool failures spike

Commands:

```bash
kubectl -n <NAMESPACE> logs -l app=runtime --since=15m | rg "tool"
```

Actions:
- Split by tool/provider and error class.
- Check upstream model/tool provider health.
- Confirm timeout budgets and cancellation behavior.

### D. Policy denial anomaly

Actions:
- Inspect policy reason code distribution.
- Confirm delegated budget envelopes and recent config changes.
- Validate no looping workloads are exhausting budgets.

### E. Circuit breaker open

Actions:
- Confirm upstream provider incident.
- Validate fallback behavior is receipt-visible.
- Roll back recent provider config changes if needed.

### F. Sync auth failures / token mint failures

Actions:
- Check alerts `OpenAgentsRuntimeSyncSocketAuthFailureRatioHigh` and `OpenAgentsKhalaTokenMintFailureRatioHigh`.
- Inspect auth reject reason classes (`unknown_kid`, `claim_mismatch`, `token_expired`, `missing_token`).
- Validate control-plane token mint config parity with runtime verifier config.
- Follow `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md#incident-a-ws-auth-and-token-failures`.

### G. Reconnect timeout storm or stale cursor spike

Actions:
- Check alerts `OpenAgentsRuntimeSyncSocketTimeoutRateHigh` and `OpenAgentsRuntimeSyncStaleCursorSpike`.
- Validate heartbeat/reconnect behavior and retention floor configuration.
- Confirm client resnapshot behavior for stale cursor responses.
- Follow:
  - `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md#incident-b-ws-reconnect-and-timeout-storm`
  - `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md#incident-c-stale_cursor-spike`

## 5. Secrets rotation

Secrets used by runtime:
- `DATABASE_URL`
- `SECRET_KEY_BASE`
- `RUNTIME_SIGNATURE_SECRET`

Rotation flow:
1. Update secret values in Kubernetes secret source.
2. Restart runtime pods and rerun smoke gate.
3. Verify internal auth and stream path still pass smoke checks.

## 6. Migration + smoke gate rerun

```bash
NAMESPACE=<NAMESPACE> \
IMAGE=us-central1-docker.pkg.dev/<PROJECT_ID>/runtime/runtime:<TAG> \
apps/runtime/deploy/jobs/run-postdeploy-gate.sh
```

Use this after deploys and after infra/security changes.

Cloud Run production flow:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

This chained script enforces deploy + migration invariants:
- runtime deploy cannot complete without migration step validation,
- migrate job image is synced to runtime service image,
- migration runs with `OpenAgentsRuntime.Release.migrate_and_verify!()`.

Manual/alert drift check:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
apps/runtime/deploy/cloudrun/check-migration-drift.sh
```

## 7. Rollback policy

- Primary rollback: `kubectl rollout undo statefulset/runtime`.
- Schema rollback: manual + explicit migration version; treat as last resort.

## 8. Validation commands

From `apps/runtime/`:

- `mix ci`
- `mix test test/openagents_runtime/deploy/jobs_assets_test.exs`
- `mix test test/openagents_runtime/deploy/network_policy_assets_test.exs`
- `mix test test/openagents_runtime/deploy/smoke_test.exs`
- `apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh`

Restart/reconnect chaos drill evidence:

- Runbook: `apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`
- Latest report: `apps/runtime/docs/reports/2026-02-21-runtime-khala-restart-reconnect-chaos-report.md`

## 9. Gate G0 local baseline verification

Before runtime-adjacent integration work, verify local DB + runtime baseline:

```bash
pg_isready -h localhost -p 5432
PGPASSWORD=postgres psql -h localhost -U postgres -d postgres -Atc "select datname from pg_database where datname in ('openagents_runtime_dev','openagents_runtime_test') order by datname;"
cd apps/runtime
mix ecto.create
mix ecto.migrate
mix test
mix runtime.contract.check
```

Expected outcome:

- Postgres is accepting connections.
- Both runtime DBs exist (`openagents_runtime_dev`, `openagents_runtime_test`).
- Migrations run cleanly.
- Runtime tests pass.
- Contract check passes with `runtime contract check passed`.
