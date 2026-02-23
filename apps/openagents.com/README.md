# openagents.com (Rust Control Service + Rust Web Shell)

Active web runtime path is Rust-only:

- Control/API service: `apps/openagents.com/service` (`openagents-control-service`)
- Web UI runtime: `apps/openagents.com/web-shell` (Rust/WASM)
- Shared UI/state crates under `crates/` (`openagents-ui-core`, `openagents-app-state`, etc.)

Legacy Laravel/Inertia/React sources remain in-repo only as historical/transition artifacts and are not part of the active runtime lane.
Legacy agent guidance is archived at `apps/openagents.com/docs/archived/legacy-laravel-deploy/AGENTS.laravel-boost.md`.

Rust-only container path:

- `apps/openagents.com/Dockerfile` builds the Rust control service and Rust web-shell dist.
- Legacy PHP/Laravel image recipe is archived at `apps/openagents.com/deploy/archived-laravel/Dockerfile`.

## Active verification

From repo root:

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
cargo check -p openagents-web-shell --target wasm32-unknown-unknown
```

## Active deploy path

1. Build Rust web-shell dist:

```bash
apps/openagents.com/web-shell/build-dist.sh
```

2. Deploy/control rollout via Rust service runbooks:

- `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`
- `apps/openagents.com/docs/20260222-oa-webparity-058-production-canary-rollback-drill.md`
- `apps/openagents.com/docs/20260222-oa-webparity-059-production-rust-route-flip.md`
- `apps/openagents.com/docs/20260222-oa-webparity-060-retire-laravel-serving-path.md`

Optional helper (no Laravel/Node runtime steps):

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:<tag> \
apps/openagents.com/service/deploy/deploy-production.sh
```

Staging deploy (canonical; 100% traffic):

```bash
TAG="$(git rev-parse --short HEAD)"
gcloud run deploy openagents-control-service-staging \
  --project openagentsgemini \
  --region us-central1 \
  --image "us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:${TAG}" \
  --quiet
```

Optional staging helper:

- `apps/openagents.com/service/deploy/deploy-staging.sh` (runs local verification gates and creates a no-traffic revision; see `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md` for traffic shift + smoke checks)

Image build (Cloud Build, Rust-only):

```bash
gcloud builds submit \
  --config apps/openagents.com/service/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  .
```
