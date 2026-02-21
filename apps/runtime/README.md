# Runtime (Rust-Only)

`apps/runtime` is the Rust runtime service for authority writes, projector updates, and Khala sync delivery.

## Status

- Rust-only active path.
- Elixir/Phoenix scaffolding is retired from active runtime operations.
- Build/test/run/deploy flows are Cargo-native.

## Local Commands

Run service:

```bash
cargo run --manifest-path apps/runtime/Cargo.toml --bin openagents-runtime-service
```

Run tests:

```bash
cargo test --manifest-path apps/runtime/Cargo.toml
```

Run migrations (requires `DB_URL` or `DATABASE_URL`):

```bash
DB_URL=postgres://... cargo run --manifest-path apps/runtime/Cargo.toml --bin runtime-migrate
```

Run smoke checks against a deployed runtime:

```bash
SMOKE_BASE_URL=http://127.0.0.1:4100 cargo run --manifest-path apps/runtime/Cargo.toml --bin runtime-smoke
```

## Runtime Binaries

- `openagents-runtime-service`: HTTP runtime service.
- `runtime-migrate`: SQL migration runner (creates/updates `runtime.*` sync/projection tables).
- `runtime-smoke`: health + authority-path smoke probe.

## Key Environment Variables

- `RUNTIME_BIND_ADDR` (default `127.0.0.1:4100`)
- `RUNTIME_SERVICE_NAME` (default `runtime`)
- `RUNTIME_BUILD_SHA` (default `dev`)
- `RUNTIME_AUTHORITY_WRITE_MODE` (`rust_active|shadow_only|read_only`)
- `RUNTIME_SYNC_TOKEN_SIGNING_KEY`
- `RUNTIME_SYNC_TOKEN_ISSUER` (default `https://openagents.com`)
- `RUNTIME_SYNC_TOKEN_AUDIENCE` (default `openagents-sync`)
- `RUNTIME_SYNC_REVOKED_JTIS` (comma-separated revoked token IDs)

Migration/env:

- `DB_URL` or `DATABASE_URL` for `runtime-migrate`.
- `SMOKE_BASE_URL` for `runtime-smoke`.

## Container + Deploy

Build image (repo root context required):

```bash
docker build -f apps/runtime/Dockerfile -t runtime:dev .
```

Cloud Build:

```bash
gcloud builds submit \
  --config apps/runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG=\"$(git rev-parse --short HEAD)\" \
  .
```

Cloud Run deploy + migrate (required sequence):

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/<project>/runtime/runtime:<tag> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

Mandatory post-deploy migration rerun command:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
apps/runtime/deploy/cloudrun/run-migrate-job.sh
```

## Docs

- `apps/runtime/docs/RUST_RUNTIME_SERVICE_FOUNDATION.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `apps/runtime/docs/KHALA_ORDERING_DELIVERY_CONTRACT.md`
- `apps/runtime/docs/KHALA_WS_THREAT_MODEL.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `apps/runtime/docs/DEPLOY_GCP.md`
- `apps/runtime/docs/DB_ROLE_ISOLATION.md`
