#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd "$script_dir/.." && pwd)"
repo_dir="$(cd "$app_dir/../.." && pwd)"

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" != "1" ]]; then
  (cd "$repo_dir" && pnpm --dir apps/oa-updates run build:server)
fi

service="${OA_UPDATES_SERVICE:-oa-updates}"
region="${OA_UPDATES_REGION:-us-central1}"
deploy_mode="${OA_UPDATES_DEPLOY_MODE:-incremental}"

if [[ "$deploy_mode" != "incremental" ]]; then
  echo "REFUSED: OA_UPDATES_DEPLOY_MODE must be incremental; full seed replacement requires the separately reviewed full-release path" >&2
  exit 1
fi

# Code/Desktop-feed deployments must derive from the exact ready image. Cloud
# Run source deployment would replace every baked mobile, Pylon, and Desktop
# seed with whatever happens to be in this checkout (including absent/stale
# gitignored directories). Resolve the ready revision to an immutable digest,
# then build only the new server and independent Desktop descriptor over it.
base_image="${OA_UPDATES_BASE_IMAGE:-}"
if [[ -z "$base_image" ]]; then
  if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" == "1" ]]; then
    echo "REFUSED: dry-run requires OA_UPDATES_BASE_IMAGE=<registry>@sha256:<64 hex>" >&2
    exit 1
  fi
  ready_revision="$(gcloud run services describe "$service" \
    --region "$region" \
    --format='value(status.latestReadyRevisionName)')"
  [[ -n "$ready_revision" ]] || {
    echo "REFUSED: Cloud Run has no latest ready revision to preserve" >&2
    exit 1
  }
  base_image="$(gcloud run revisions describe "$ready_revision" \
    --region "$region" \
    --format='value(status.imageDigest)')"
fi

if [[ ! "$base_image" =~ ^[^[:space:]]+@sha256:[0-9a-f]{64}$ ]]; then
  echo "REFUSED: preservation base must be an immutable sha256 image digest" >&2
  exit 1
fi

source_revision="${OA_UPDATES_SOURCE_REVISION:-$(git -C "$repo_dir" rev-parse HEAD)}"
if [[ ! "$source_revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "REFUSED: source revision must be an exact 40-character Git object id" >&2
  exit 1
fi

image_repository="${base_image%@sha256:*}"
image_tag="${OA_UPDATES_IMAGE_TAG:-${image_repository}:source-${source_revision}}"
build_args=(
  builds submit
  --config cloudbuild.incremental.yaml
  --substitutions "_BASE_IMAGE=${base_image},_IMAGE=${image_tag}"
  .
)

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" == "1" ]]; then
  built_digest="${OA_UPDATES_BUILT_IMAGE_DIGEST:-}"
else
  (cd "$app_dir" && gcloud "${build_args[@]}")
  built_digest="$(gcloud artifacts docker images describe "$image_tag" \
    --format='value(image_summary.digest)')"
fi

if [[ ! "$built_digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "REFUSED: incremental build did not resolve to an immutable sha256 digest" >&2
  exit 1
fi
deploy_image="${image_repository}@${built_digest}"

# Deploy OpenAgents Updates to Cloud Run from the oa-updates app directory.
#
# Required before running:
#   gcloud auth login
#   gcloud config set project <project-id>
#   export OA_PUBLIC_URL=https://<your-cloud-run-service-url>
#   export OA_SEED_DIST=/app/dist
#   export OA_SEED_RUNTIME=<runtime-version>
#
# Optional:
#   export OA_SEED_PLATFORM=ios
#   export OA_SEED_EXPO_CLIENT_PATH=/app/dist/expo-client.json
#   export OA_DESKTOP_RELEASES_DIST=/app/desktop-dist
#   export OA_OPENAGENTS_DESKTOP_RELEASE_DIST=/app/openagents-desktop-dist
#   export OA_RELEASE_SET_BUCKET=<dedicated-gcs-bucket>
#   export OA_RELEASE_SET_PINS_PATH=/app/openagents-desktop-dist/release-set-pins.json
#
# Code signing (#8530 / CFG-14): the OTA manifest signing key reaches the
# service as the OA_SIGNING_KEY env var mounted from GCP Secret Manager
# (secret `oa-updates-codesign-key`, project openagentsgemini) via
# --set-secrets. It is never passed as inline env. To point at a different
# secret/version, export OA_SIGNING_SECRET=<secret-name>:<version>; set it
# to the empty string to deploy without code signing (dev projects only).
#
# This script is intentionally not run by tests or setup. Run it manually when
# the target Google Cloud project and seed export are ready.

env_vars=("OA_PUBLIC_URL=${OA_PUBLIC_URL:?set OA_PUBLIC_URL}")

if [[ -n "${OA_SEED_DIST:-}" || -n "${OA_SEED_RUNTIME:-}" ]]; then
  env_vars+=(
    "OA_SEED_DIST=${OA_SEED_DIST:?set OA_SEED_DIST}"
    "OA_SEED_RUNTIME=${OA_SEED_RUNTIME:?set OA_SEED_RUNTIME}"
    "OA_SEED_PLATFORM=${OA_SEED_PLATFORM:-ios}"
  )

  if [[ -n "${OA_SEED_EXPO_CLIENT_PATH:-}" ]]; then
    env_vars+=("OA_SEED_EXPO_CLIENT_PATH=${OA_SEED_EXPO_CLIENT_PATH}")
  fi

  if [[ -n "${OA_SEED_BRANCH:-}" ]]; then
    env_vars+=("OA_SEED_BRANCH=${OA_SEED_BRANCH}")
  fi
fi

if [[ -n "${OA_DESKTOP_RELEASES_DIST:-}" ]]; then
  env_vars+=("OA_DESKTOP_RELEASES_DIST=${OA_DESKTOP_RELEASES_DIST}")
fi

if [[ -n "${OA_OPENAGENTS_DESKTOP_RELEASE_DIST:-}" ]]; then
  env_vars+=("OA_OPENAGENTS_DESKTOP_RELEASE_DIST=${OA_OPENAGENTS_DESKTOP_RELEASE_DIST}")
fi

if [[ -n "${OA_RELEASE_SET_BUCKET:-}" || -n "${OA_RELEASE_SET_PINS_PATH:-}" ]]; then
  env_vars+=(
    "OA_RELEASE_SET_BUCKET=${OA_RELEASE_SET_BUCKET:?set OA_RELEASE_SET_BUCKET}"
    "OA_RELEASE_SET_PINS_PATH=${OA_RELEASE_SET_PINS_PATH:?set OA_RELEASE_SET_PINS_PATH}"
  )
fi

env_csv="$(IFS=,; echo "${env_vars[*]}")"

args=(
  run deploy "$service"
  --image "$deploy_image"
  --region "$region" \
  --allow-unauthenticated \
  --port 8080 \
  # Additive by construction: gcloud's update form preserves every existing
  # env mapping not named in this invocation. A Desktop-only publication can
  # therefore never remove the mobile seed, and a mobile deploy can never
  # remove the ReleaseSet bucket/pin configuration.
  --update-env-vars "$env_csv"
)

# OA_SIGNING_KEY is mounted from Secret Manager, never inline (#8530).
# Update only this named secret; unrelated secret mappings remain attached.
signing_secret="${OA_SIGNING_SECRET-oa-updates-codesign-key:latest}"
if [[ -n "$signing_secret" ]]; then
  args+=(--update-secrets "OA_SIGNING_KEY=${signing_secret}")
fi

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" == "1" ]]; then
  printf 'BUILD_ARG=%s\n' "${build_args[@]}"
  printf 'DEPLOY_ARG=%s\n' "${args[@]}"
else
  gcloud "${args[@]}"
fi
