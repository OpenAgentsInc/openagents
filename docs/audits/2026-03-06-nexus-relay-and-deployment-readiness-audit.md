<!-- Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, and `docs/OWNERSHIP.md`. Deployment state, endpoint behavior, and repo structure may have changed after this audit. -->

# Audit: Nexus Relay And Deployment Readiness

Date: 2026-03-06

## Scope

This audit answers four questions:

1. What is the repo currently configured to use for the OpenAgents-hosted Nexus relay and authority surfaces?
2. What live endpoint behavior exists today for `nexus.openagents.com` and `relay.openagents.dev`?
3. What deployment evidence exists in archived backroom materials?
4. Is the current retained repo ready to deploy as the canonical Nexus stack?

Sources reviewed:

- current repo:
  - `apps/autopilot-desktop/src/app_state.rs`
  - `apps/autopilot-desktop/src/sync_bootstrap.rs`
  - `apps/autopilot-desktop/src/starter_demand_client.rs`
  - `apps/nexus-control/src/lib.rs`
  - `apps/nexus-relay/src/lib.rs`
  - `docs/MVP.md`
  - `docs/autopilot-earn/README.md`
- archived backroom:
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/core/DEPLOYMENT_RUST_SERVICES.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/openagents.com/docs/STAGING_DEPLOY_RUNBOOK.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/sync/SPACETIME_GCLOUD_DEPLOYMENT_CONSIDERATIONS.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/audits/2026-02-25-local-runtime-nexus-swarm-audit.md`
  - backroom `apps/openagents.com/deploy/*`
  - backroom `apps/runtime/deploy/*`

Live checks run on 2026-03-06:

- `dig +short nexus.openagents.com`
- `dig +short relay.openagents.dev`
- `curl -I https://nexus.openagents.com`
- `curl https://nexus.openagents.com/api/stats`
- `curl -X OPTIONS https://nexus.openagents.com/api/sync/token`
- websocket upgrade probes against `https://nexus.openagents.com/` and `https://nexus.openagents.com/ws`
- `gcloud run services list --platform=managed --region=us-central1 --project=openagentsgemini`
- `gcloud beta run domain-mappings list --platform=managed --region=us-central1 --project=openagentsgemini`

## Executive Answer

`relay.openagents.dev` is not the right default anymore.

As of 2026-03-06:

- `relay.openagents.dev` does not resolve in DNS.
- `nexus.openagents.com` is live.
- `nexus.openagents.com` serves:
  - a browser-facing Nexus Relay page at `/`
  - a working websocket endpoint at `/` and `/ws`
  - `GET /api/stats`
  - CORS-enabled `OPTIONS /api/sync/token`

So the user instinct is correct: the OpenAgents-hosted Nexus surface is currently living under `nexus.openagents.com`, not `relay.openagents.dev`.

However, the retained repo is not yet a clean, canonical deployable replacement for that live Nexus stack.

On the specific question of whether `nexus.openagents.com` is "current" or "old":

- it is definitely a **current live OpenAgents service** as of 2026-03-06,
- but it appears to be backed by **older/backroom-era deployment lineage**, not by the retained repo's current `apps/nexus-relay` implementation deployed as-is.

The short version:

- **live domain truth:** `nexus.openagents.com`
- **current repo default truth:** still `relay.openagents.dev`
- **deployment readiness of current retained repo:** **not ready for clean canonical Nexus deployment**

## Key Findings

### 1. The current repo still hardcodes the wrong relay hostname

Current retained defaults are still set to `wss://relay.openagents.dev`.

Primary examples:

- `apps/autopilot-desktop/src/app_state.rs`
  - `DEFAULT_NEXUS_PRIMARY_RELAY_URL = "wss://relay.openagents.dev"`
- `apps/nexus-control/src/lib.rs`
  - `DEFAULT_HOSTED_NEXUS_RELAY_URL = "wss://relay.openagents.dev"`
- `apps/nexus-control` tests also use `wss://relay.openagents.dev`

This means the desktop default and the hosted starter-demand authority default are both pointed at a dead hostname.

### 2. `nexus.openagents.com` is live and appears to be the real hosted Nexus surface

Observed live behavior on 2026-03-06:

- `dig +short nexus.openagents.com`
  - returned Cloudflare-backed addresses
- `curl https://nexus.openagents.com/api/stats`
  - returned real JSON stats
- `curl -X OPTIONS https://nexus.openagents.com/api/sync/token`
  - returned `200`
- websocket upgrade probes to `https://nexus.openagents.com/` and `https://nexus.openagents.com/ws`
  - returned `101 Switching Protocols`
  - immediately emitted a NIP-42 `["AUTH", "<challenge>"]` challenge

That is strong evidence that `nexus.openagents.com` is already functioning as both:

- the hosted relay endpoint
- the hosted control/API surface

This means `nexus.openagents.com` is not merely an old hostname left in docs. It is currently serving the real public Nexus surface.

### 3. `relay.openagents.dev` is dead from the outside

Observed live behavior on 2026-03-06:

- `dig +short relay.openagents.dev`
  - returned no records
- `curl https://relay.openagents.dev`
  - failed with host resolution error

That means current defaults in the retained repo are operationally broken for a new user unless overridden.

### 4. The live Nexus stack does not match the retained in-repo `apps/nexus-relay`

The retained repo contains `apps/nexus-relay`, but it is much thinner than the live `nexus.openagents.com` behavior.

What `apps/nexus-relay` currently is:

- an in-memory websocket relay
- no persistent storage
- no auth/session bridge
- no NIP-42 AUTH flow
- no browser landing page
- only `GET /healthz` plus websocket route(s)
- no deploy docs, Dockerfile, cloudbuild, or domain mapping assets in the retained repo

What the live `nexus.openagents.com` behavior shows:

- browser-facing relay page
- websocket upgrade on `/` and `/ws`
- NIP-42 AUTH challenge on connect
- `/api/stats`
- `/api/sync/token`

Conclusion:

The live Nexus surface is not simply the retained `apps/nexus-relay` binary deployed as-is.

Either:

1. the live service is still coming from older backroom `apps/openagents.com` infrastructure, or
2. there is deployment/runtime glue outside the retained repo that is not represented here.

The most likely reading is:

- `nexus.openagents.com` is **current-live**,
- but it is **not current-repo-derived**,
- and it more closely matches the older/backroom service family than the retained MVP repo's Nexus apps.

### 5. `apps/nexus-control` has the right authority concepts, but not the canonical public surface shape

The retained `apps/nexus-control` does expose meaningful hosted-Nexus authority routes:

- `/api/session/desktop`
- `/api/sync/token`
- `/api/starter-demand/*`
- `/stats`
- `/v1/kernel/*`

This is useful and real.

But it is still not a drop-in mirror of the live Nexus public surface:

- current repo exposes `/stats`, while live service responds at `/api/stats`
- no repo-local proof that `apps/nexus-control` is what powers `nexus.openagents.com`
- no retained deployment/runbook assets for this app

So `apps/nexus-control` is a viable local/backend building block, but not yet a canonical deploy lane.

### 6. Backroom contains real GCP deploy history for the older control service, not for the retained Nexus apps

Archived backroom contains substantial real deployment materials:

- `docs/core/DEPLOYMENT_RUST_SERVICES.md`
- `apps/openagents.com/docs/STAGING_DEPLOY_RUNBOOK.md`
- `apps/openagents.com/deploy/cloudbuild.yaml`
- `apps/openagents.com/deploy/deploy-production.sh`
- `apps/openagents.com/deploy/deploy-staging.sh`
- `apps/openagents.com/deploy/canary-rollout.sh`
- canary drill reports

This is strong evidence that there was a real Cloud Run deploy lane around the older `apps/openagents.com` control stack in GCP project `openagentsgemini`, region `us-central1`.

Backroom also consistently references:

- `wss://nexus.openagents.com`
- `https://nexus.openagents.com/api/stats`
- canonical `POST /api/sync/token`

That aligns with the live endpoint behavior observed today.

### 7. Live GCP resource state could not be proven from `gcloud` in this session

Attempts to list current GCP resources failed because local `gcloud` auth was expired:

- `gcloud run services list ...`
- `gcloud beta run domain-mappings list ...`

Both failed with reauthentication-required errors.

So this audit can prove live public endpoint behavior, and it can prove archived deploy assets exist, but it cannot prove the exact current Cloud Run service/domain-mapping inventory from GCP APIs without refreshed credentials.

## Evidence Summary

### Current repo posture

What exists:

- `apps/nexus-control`
  - real local/hosted authority surface
  - sync token mint path
  - starter-demand authority
  - kernel authority slice
- `apps/nexus-relay`
  - basic websocket relay implementation
- desktop config + starter-demand client
  - still expecting a hosted Nexus relay/control split

What is missing:

- no retained deploy/runbook lane for `apps/nexus-control`
- no retained deploy/runbook lane for `apps/nexus-relay`
- no Dockerfiles or Cloud Build manifests for those retained Nexus apps
- no retained domain mapping or TLS runbook for `nexus.openagents.com`
- no retained proof that current repo reproduces the live NIP-42-authenticated relay behavior

### Backroom posture

What exists:

- real GCP/Cloud Run deployment documentation and scripts for older control service surfaces
- canary/staging/prod rollout materials
- repeated use of `nexus.openagents.com` as the canonical relay/stats host
- repeated use of `/api/sync/token` as canonical control endpoint

What does not exist in the retained repo:

- those deploy scripts and runbooks
- the older `apps/openagents.com` service code that likely matches the live public Nexus surface more closely

## Readiness Assessment

### Ready right now

1. **Ready to conclude that `relay.openagents.dev` should not be the desktop default**
   - Yes.
2. **Ready to conclude that `nexus.openagents.com` is the live OpenAgents-hosted Nexus endpoint family**
   - Yes.
3. **Ready to update docs/config defaults to `nexus.openagents.com`**
   - Yes.

### Not ready right now

1. **Ready to say the retained repo is the canonical deploy source for the live Nexus stack**
   - No.
2. **Ready to deploy `apps/nexus-relay` as the production Nexus relay without additional work**
   - No.
3. **Ready to retire backroom deploy assets and treat retained repo deploys as complete**
   - No.

## Why The Retained Repo Is Not Yet Ready To Deploy As Canonical Nexus

The main blockers are:

1. **Endpoint mismatch**
   - repo defaults point to dead `relay.openagents.dev`
   - live service is at `nexus.openagents.com`
2. **Behavior mismatch**
   - live relay performs NIP-42 auth challenge
   - retained `apps/nexus-relay` does not
3. **Surface mismatch**
   - live service exposes `/api/stats`
   - retained `apps/nexus-control` exposes `/stats`
4. **Deploy gap**
   - retained repo has no canonical Cloud Run/build/domain-mapping runbook for `apps/nexus-control` or `apps/nexus-relay`
5. **Ops verification gap**
   - live GCP service inventory and domain mappings were not verifiable from `gcloud` in this session

## Recommended Next Actions

### Immediate

1. Change retained defaults from `wss://relay.openagents.dev` to `wss://nexus.openagents.com`.
2. Audit docs for any remaining `relay.openagents.dev` references and remove them.
3. Decide whether the product contract is:
   - one canonical host: `nexus.openagents.com` for both relay + control, or
   - split hosts for control and relay

### Before calling the retained repo deploy-ready

1. Add canonical deployment assets for:
   - `apps/nexus-control`
   - `apps/nexus-relay`
2. Converge retained route shape with live route shape.
3. Implement NIP-42 auth in retained `apps/nexus-relay`, or explicitly document that another service owns relay auth.
4. Decide whether retained `apps/nexus-relay` is:
   - the real future production relay, or
   - only a local dev harness
5. Refresh `gcloud` auth and verify:
   - Cloud Run services
   - domain mappings
   - TLS/domain ownership for `nexus.openagents.com`

### If the goal is “ship MVP now without infra archaeology”

The lowest-risk move is:

1. treat `nexus.openagents.com` as the real hosted Nexus domain now,
2. update desktop + `nexus-control` defaults to match it,
3. document that the live hosted Nexus currently comes from legacy/backroom-era deployment assets,
4. separately decide whether to port that deploy lane into the retained repo or replace it with a new retained Nexus deployment lane.

## Bottom Line

There is a live Nexus service.

It is at `nexus.openagents.com`.

That domain is current and serving traffic now. It does not look like a dead or merely historical hostname.

But it also does not look like the retained repo's current Nexus apps are the thing serving it.

So the right interpretation is:

- **`nexus.openagents.com` is the current live service endpoint**
- **the deployment behind it appears to come from older/backroom-era infrastructure or unreconciled external glue**

The retained repo is still pointed at `relay.openagents.dev`, which is dead.

The retained repo is **not yet** a clean deployable source of truth for the live Nexus stack, because the code defaults, public route shape, relay auth behavior, and deployment assets do not yet line up with what is actually live.
