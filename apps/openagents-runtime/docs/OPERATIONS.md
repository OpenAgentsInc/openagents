# OpenAgents Runtime Operations

Operational procedures for day-to-day runtime management and incident response.

## 1. Daily checks

```bash
kubectl -n <NAMESPACE> get pods -l app=openagents-runtime
kubectl -n <NAMESPACE> get hpa openagents-runtime
kubectl -n <NAMESPACE> get job openagents-runtime-smoke
```

Expected outcome:
- Pods are healthy.
- HPA is stable.
- Latest smoke run is `Complete`.

## 2. Observability surfaces

- Dashboard asset:
  - `apps/openagents-runtime/deploy/monitoring/grafana/openagents-runtime-ops-dashboard.json`
- Alert rules:
  - `apps/openagents-runtime/deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml`
- Alert thresholds/runbook:
  - `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md`

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
kubectl -n <NAMESPACE> logs -l app=openagents-runtime --since=15m | rg "stream"
```

Actions:
- Check `stream.session` outcomes (`tail_timeout` vs `client_closed`).
- Validate run terminal event flow (`run.finished`).
- Inspect cancel storm behavior.

### B. Lease steals spike

Commands:

```bash
kubectl -n <NAMESPACE> logs -l app=openagents-runtime --since=15m | rg "lease"
```

Actions:
- Check pod restart churn.
- Validate janitor cycle and stale lease handling.
- Confirm BEAM distribution/network policy integrity.

### C. Tool failures spike

Commands:

```bash
kubectl -n <NAMESPACE> logs -l app=openagents-runtime --since=15m | rg "tool"
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
IMAGE=us-central1-docker.pkg.dev/<PROJECT_ID>/openagents-runtime/runtime:<TAG> \
apps/openagents-runtime/deploy/jobs/run-postdeploy-gate.sh
```

Use this after deploys and after infra/security changes.

## 7. Rollback policy

- Primary rollback: `kubectl rollout undo statefulset/openagents-runtime`.
- Schema rollback: manual + explicit migration version; treat as last resort.

## 8. Validation commands

From `apps/openagents-runtime/`:

- `mix ci`
- `mix test test/openagents_runtime/deploy/jobs_assets_test.exs`
- `mix test test/openagents_runtime/deploy/network_policy_assets_test.exs`
- `mix test test/openagents_runtime/deploy/smoke_test.exs`
