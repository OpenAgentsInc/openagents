# Spacetime GCloud Deployment Considerations

Date: 2026-02-26
Status: planning + readiness audit (not a deploy runbook)
Owner lanes: Infra, Runtime, Control, Desktop

## Purpose

Capture current Spacetime deployment posture and what is still required to run SpacetimeDB as a real GCP-hosted sync substrate for OpenAgents.

For managed-hosting guidance (Maincloud), see `docs/sync/SPACETIME_MAINCLOUD_MANAGED_DEPLOYMENT.md`.

This document answers:

1. Do we currently have Spacetime/SpacetimeDB deployed from this repo to GCP?
2. Are we close to deployment?
3. What still must be built/validated before production deployment is real?

## Current State Snapshot (As of 2026-02-26)

### 1) What is clearly deployed and managed in GCP from this repo

Canonical GCP deploy docs/scripts exist for:

1. `openagents-control-service` (Cloud Run)
2. `runtime` + `runtime-migrate` (Cloud Run service + Cloud Run job)

References:

1. `docs/core/DEPLOYMENT_RUST_SERVICES.md`
2. `apps/runtime/deploy/cloudrun/README.md`

### 2) What exists for Spacetime today

Spacetime assets in this repo are primarily:

1. environment/health checks (`scripts/spacetime/provision-check.sh`)
2. rollout orchestration scripts and gates (`scripts/spacetime/run-staging-canary-rollout.sh`, `scripts/spacetime/run-production-phased-rollout.sh`)
3. docs/plans describing target SpacetimeDB direction (`docs/plans/spacetimedb-full-integration.md`)

There is currently no canonical in-repo GCP deploy script/manifests for a dedicated SpacetimeDB service (Cloud Run/GKE/Compute) comparable to runtime/control deploy scripts.

### 3) Implementation reality relevant to deployment

Current retained code paths are Spacetime-native:

1. Desktop sync opens Spacetime subscribe sessions at `/v1/database/{database}/subscribe`.
2. Runtime state bootstrap requires configured Spacetime publisher bootstrap (`SpacetimePublisher::from_env()`).
3. Runtime publish path writes through shared `autopilot-spacetime` network client APIs with outbox/retry semantics.

Implication:

Deploying SpacetimeDB infra is now an operations/deployment problem (durability, auth boundaries, observability, rollout), not a legacy transport migration problem.

### 4) Live GCP state verification attempt in this session

I attempted read-only `gcloud` listing commands for `openagentsgemini` and `us-central1`, but token refresh failed in non-interactive mode.

Commands attempted:

1. `gcloud run services list --platform=managed --region=us-central1 --project=openagentsgemini`
2. `gcloud run jobs list --region=us-central1 --project=openagentsgemini`
3. `gcloud artifacts repositories list --location=us-central1 --project=openagentsgemini`

Result:

1. Re-auth required (`gcloud auth login`), so this session cannot assert live resource state from GCP APIs.

## Answer: Do we have Spacetime currently deployed or soon to be deployed in GCP?

### Current deployed state (from repo evidence)

From repository-managed infrastructure and code evidence, SpacetimeDB is not yet established here as a first-class deployed GCP service with canonical deploy scripts/runbooks equivalent to runtime/control.

### Near-term likelihood

"Soon" is plausible from planning and rollout scaffolding, but not deploy-complete. The repo contains substantial prep (env matrix, canary scripts, SLO gates) but still has material integration and infra gaps.

## Deployment Considerations (GCP)

### A) Hosting topology decision (must be explicit)

Pick one canonical model and document it:

1. GKE stateful deployment for SpacetimeDB
2. Compute Engine managed VM deployment
3. Another managed SpacetimeDB hosting approach

For whichever model is chosen, define:

1. single canonical service DNS endpoint per env (`dev/staging/prod`)
2. TLS termination strategy
3. region and failover stance

### B) Durability and state lifecycle

SpacetimeDB deployment must include durable state controls:

1. persistent storage/WAL durability policy
2. snapshot cadence and retention
3. restore drill and replay verification
4. RPO/RTO targets

Without this, rollout scripts only validate connectivity and not operational survivability.

### C) Auth and trust boundary integration

Current control-issued sync token model must map cleanly to SpacetimeDB auth:

1. issuer/audience/signing-key rotation policy per env
2. token mint/refresh handling across control and desktop
3. incident behavior for expired/revoked tokens

Minimum alignment references:

1. `docs/sync/SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`
2. `docs/sync/SPACETIME_ENVIRONMENT_MATRIX.md`

### D) Network and edge policy

Deployment needs explicit network/security posture:

1. ingress controls (public endpoint vs private + gateway)
2. CORS/origin policy where relevant
3. service account/secret manager integration
4. DDoS/rate-limit posture

### E) Observability and SLOs

Before production, ensure:

1. Spacetime service metrics exported to existing observability stack
2. alert routing and runbook links wired
3. staged/production SLO snapshot endpoint (`OA_SPACETIME_SLO_SNAPSHOT_URL`) backed by real data

### F) Release process integration

Add canonical deploy/release lane for SpacetimeDB similar to runtime/control:

1. build/publish artifact step
2. deploy step
3. post-deploy health and migration checks
4. rollback command path

Today, only provisioning checks and rollout probes exist; deploy orchestration is missing.

### G) App readiness dependencies (blocking)

Infra deployment should not be declared complete until app integration is validated continuously:

1. Desktop subscribe/reconnect behavior remains green against Spacetime endpoint semantics.
2. Runtime publish path remains green with real reducer writes and durable outbox/retry gates.
3. Protocol surface remains converged on stream/query-set model and stale-cursor/replay semantics.
4. Control token endpoint/docs/tests remain internally consistent on canonical `POST /api/sync/token`.

## Required Work Before "Spacetime on GCP" Can Be Called Production-Ready

### 1) Infra buildout (missing)

1. choose and implement canonical hosting model
2. add deploy scripts/runbook under canonical deployment docs
3. provision secrets and DNS/TLS per env
4. add backup/restore runbooks and drills

### 2) Runtime/control/desktop cutover (in progress)

1. keep runtime real-write Spacetime path green through release cycles
2. keep desktop Spacetime-native subscription path green through release cycles
3. keep canonical sync token route behavior/docs locked (`POST /api/sync/token`)

### 3) Gate evidence (must be green with real infra)

1. `scripts/spacetime/provision-check.sh <env>` against live endpoint
2. replay/resume parity gates
3. chaos drill gates
4. staged canary and production phased rollout artifacts

## Minimal Verification Commands For Operator (after re-auth)

Use these to establish live state quickly:

```bash
gcloud auth login

gcloud run services list \
  --platform=managed \
  --region=us-central1 \
  --project=openagentsgemini

gcloud run jobs list \
  --region=us-central1 \
  --project=openagentsgemini

gcloud artifacts repositories list \
  --location=us-central1 \
  --project=openagentsgemini
```

If a dedicated SpacetimeDB service exists outside Cloud Run (GKE/Compute), add the corresponding canonical verification commands to this document once the hosting model is finalized.

## Recommended Next Step

Create and land one canonical Spacetime GCP deployment runbook + scripts (equivalent to runtime/control deploy lanes), then wire app/runtime integration to that target and promote only through existing canary/SLO gates.
