# openagents.com (Rust Control Service + Rust Web Shell)

Active web runtime path is Rust-only:

- Control/API service: `apps/openagents.com/service` (`openagents-control-service`)
- Web UI runtime: `apps/openagents.com/web-shell` (Rust/WASM)
- Shared UI/state crates under `crates/` (`openagents-ui-core`, `openagents-app-state`, etc.)

Legacy Laravel/Inertia/React sources remain in-repo only as historical/transition artifacts and are not part of the active runtime lane.

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
- `apps/openagents.com/docs/20260221-route-cutover-default-rust.md`

Optional helper (no Laravel/Node runtime steps):

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service \
IMAGE=us-central1-docker.pkg.dev/<project>/<repo>/openagents-control-service:<tag> \
apps/openagents.com/service/deploy/deploy-production.sh
```

Staging helper:

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service-staging \
IMAGE=us-central1-docker.pkg.dev/<project>/<repo>/openagents-control-service:<tag> \
apps/openagents.com/service/deploy/deploy-staging.sh
```

Image build (Cloud Build, Rust-only):

```bash
gcloud builds submit \
  --config apps/openagents.com/service/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  .
```
