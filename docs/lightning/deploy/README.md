# Aperture deploy (L402 gateway on GCP)

Build and push the Aperture image for Cloud Run. See **§7.1** in `docs/lightning/VOLTAGE_TO_L402_CONNECT.md` for full deploy steps and troubleshooting.

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

## Config

Base Aperture config with Voltage authenticator: `docs/lightning/scripts/aperture-voltage-config.yaml`. Update that file and add a new secret version before redeploying:

```bash
gcloud secrets versions add l402-aperture-config --data-file=docs/lightning/scripts/aperture-voltage-config.yaml
```

## Files

- **Dockerfile.aperture** – Multi-stage build from Lightning Labs aperture (Go 1.24); no Docker Hub dependency.
- **cloudbuild-aperture.yaml** – Cloud Build config to build and push to Artifact Registry repo `l402`.
