# OpenAgents Runtime Deploy (GCP/GKE)

This runbook defines the production deploy flow for `apps/runtime`.

For Cloud Run production deploys, use `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`.

## 1. Preconditions

- You have `gcloud` + `kubectl` authenticated to the target project/cluster.
- Runtime secrets exist in target namespace as `runtime-secrets` with keys:
  - `DATABASE_URL`
  - `SECRET_KEY_BASE`
  - `RUNTIME_SIGNATURE_SECRET`
- Namespace and cluster context are selected.

```bash
gcloud config set project <PROJECT_ID>
gcloud container clusters get-credentials <CLUSTER_NAME> --region <REGION>
kubectl config current-context
```

Expected outcome:
- Context points to the target cluster.

## 2. Build and push runtime image

From repo root:

```bash
gcloud builds submit \
  --config apps/runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/runtime
```

Expected outcome:
- Image pushed to Artifact Registry:
  - `us-central1-docker.pkg.dev/<PROJECT_ID>/runtime/runtime:<TAG>`

## 3. Deploy manifests

Apply the overlay for the environment:

```bash
kubectl apply -k apps/runtime/deploy/k8s/overlays/staging
# or prod/dev overlay as appropriate
```

Check rollout:

```bash
kubectl -n <NAMESPACE> rollout status statefulset/runtime --timeout=600s
kubectl -n <NAMESPACE> get pods -l app=runtime
```

Expected outcome:
- StatefulSet rollout completes.
- Runtime pods are `Running` and readiness probes are healthy.

## 4. Run migration + smoke gate (required)

Use the post-deploy gate runner:

```bash
NAMESPACE=<NAMESPACE> \
IMAGE=us-central1-docker.pkg.dev/<PROJECT_ID>/runtime/runtime:<TAG> \
apps/runtime/deploy/jobs/run-postdeploy-gate.sh
```

What it does:
1. Runs `runtime-migrate` job (`OpenAgentsRuntime.Release.migrate/0`).
2. Runs `runtime-smoke` job (`OpenAgentsRuntime.Deploy.Smoke.run!/1`).

Expected outcome:
- Both jobs complete successfully.
- Smoke logs show health + stream + tool path checks passing.

## 5. Post-deploy verification

```bash
kubectl -n <NAMESPACE> get job runtime-migrate
kubectl -n <NAMESPACE> get job runtime-smoke
kubectl -n <NAMESPACE> logs job/runtime-smoke
```

Expected outcome:
- Jobs show `Complete`.
- Smoke logs do not contain runtime failures.

## 6. Rollback

Rollback app pods to previous revision:

```bash
kubectl -n <NAMESPACE> rollout undo statefulset/runtime
kubectl -n <NAMESPACE> rollout status statefulset/runtime --timeout=600s
```

If schema rollback is required (manual, high-risk):

```bash
kubectl -n <NAMESPACE> run runtime-rollback \
  --image=us-central1-docker.pkg.dev/<PROJECT_ID>/runtime/runtime:<PREVIOUS_TAG> \
  --restart=Never \
  --env=DATABASE_URL=... \
  --env=SECRET_KEY_BASE=... \
  --env=RUNTIME_SIGNATURE_SECRET=... \
  --command -- bin/openagents_runtime eval 'OpenAgentsRuntime.Release.rollback(OpenAgentsRuntime.Repo, <MIGRATION_VERSION>)'
```

Only run schema rollback when backward compatibility is broken and restore-forward is not viable.

## 7. Network policy check

```bash
kubectl -n <NAMESPACE> get networkpolicy runtime-ingress -o yaml
```

Expected outcome:
- BEAM ports (`4369`, `9000`) only allow runtime peers.
- HTTP port (`4000`) only allows trusted control-plane clients.

## 8. Reference docs

- `apps/runtime/docs/OPERATIONS.md`
- `apps/runtime/docs/OPERATIONS_ALERTING.md`
- `apps/runtime/docs/NETWORK_POLICY.md`
- `apps/runtime/deploy/jobs/README.md`
