# Aperture deploy (L402 gateway on GCP)

**Full runbook (architecture, secrets, how to use, how to edit, troubleshooting):**
`docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`

This directory contains only the **image build** and optional Cloud Build config. Config content, Secret Manager, and Cloud Run deploy are described in the runbook and in `docs/lightning/reference/VOLTAGE_TO_L402_CONNECT.md` (§7.1).

**Canonical gateway URL:** `https://l402.openagents.com` (custom domain → Cloud Run). Staging route: `https://l402.openagents.com/staging`. Use these for `OA_LIGHTNING_OPS_*` env vars; they are the defaults in `apps/lightning-ops`. **Operator checklist:** `docs/lightning/status/20260212-0753-status.md` §12.

## Build image (linux/amd64)

Cloud Run requires `linux/amd64`. From repo root:

```bash
gcloud auth configure-docker us-central1-docker.pkg.dev --quiet
cd docs/lightning/deploy
docker buildx build --platform linux/amd64 -f Dockerfile.aperture \
  -t us-central1-docker.pkg.dev/openagentsgemini/l402/aperture:latest --push .
```

## Cloud Build (optional)

If your account has Cloud Build permissions:

```bash
# From repo root
gcloud builds submit --config docs/lightning/deploy/cloudbuild-aperture.yaml \
  --substitutions=_TAG="$(git rev-parse --short HEAD)" \
  docs/lightning/deploy
```

## Config (summary)

- **Production** uses **Postgres**; the config template is `docs/lightning/scripts/aperture-voltage-config-postgres.yaml` with `password: "REPLACE_PASSWORD"`. You must inject the real DB password when creating a new Secret Manager version (see runbook §5 and §6.1). Do not commit the password.
- **SQLite** base config (for local/testing only): `docs/lightning/scripts/aperture-voltage-config.yaml`. Cloud Run does not use SQLite (filesystem not writable).

## Files in this directory

- **Dockerfile.aperture** – Multi-stage build from Lightning Labs Aperture source (Go 1.24); no Docker Hub dependency.
- **cloudbuild-aperture.yaml** – Cloud Build config to build and push to Artifact Registry repo `l402`.
