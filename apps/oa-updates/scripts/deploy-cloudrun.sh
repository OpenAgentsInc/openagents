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

# A seed-publishing deploy intentionally replaces the bytes baked into the
# image for one surface (the mobile Expo export, or the legacy Desktop v1
# archive tree). Track whether this invocation is one of those so image
# selection below can require the matching full rebuild.
seed_requested=0

if [[ -n "${OA_SEED_DIST:-}" || -n "${OA_SEED_RUNTIME:-}" ]]; then
  seed_requested=1
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
  seed_requested=1
  env_vars+=("OA_DESKTOP_RELEASES_DIST=${OA_DESKTOP_RELEASES_DIST}")
fi

if [[ -n "${OA_OPENAGENTS_DESKTOP_RELEASE_DIST:-}" ]]; then
  env_vars+=("OA_OPENAGENTS_DESKTOP_RELEASE_DIST=${OA_OPENAGENTS_DESKTOP_RELEASE_DIST}")
fi

# The signed ReleaseSet v2 desktop feed is wired by default so every deploy
# keeps the stable and rc channels served (REL-FEED-01 #8993). The pins file is
# committed under openagents-desktop-dist and baked into the image at the
# container path below. Override only when deploying a different environment.
env_vars+=(
  "OA_RELEASE_SET_BUCKET=${OA_RELEASE_SET_BUCKET:-openagentsgemini-oa-updates-release-set}"
  "OA_RELEASE_SET_PINS_PATH=${OA_RELEASE_SET_PINS_PATH:-/app/openagents-desktop-dist/release-set-pins.json}"
)

env_csv="$(IFS=,; echo "${env_vars[*]}")"

# Image selection ------------------------------------------------------------
#
# `--source .` bakes whatever currently sits in this checkout's gitignored
# dist/, desktop-dist/, pylon-dist/, and desktop-ota/ directories into a brand
# new image layer. That is correct, and required, exactly when this deploy is
# intentionally publishing a fresh mobile (OA_SEED_DIST) or legacy Desktop v1
# (OA_DESKTOP_RELEASES_DIST) seed -- publish-ota.sh always sets OA_SEED_DIST
# right before calling this script for exactly that reason, and must keep
# doing a full rebuild so the exported bundle actually ships.
#
# Any OTHER deploy -- a bare server code push, a ReleaseSet v2 bucket/pin
# config change, or staging a new v2 RC manifest in the git-tracked
# openagents-desktop-dist/ tree -- must NOT go through `--source .`, because
# those seed directories are almost always empty or stale in an ordinary
# checkout at that moment, and Docker COPY of an empty local directory
# silently erases the release bytes already baked into the currently running
# image (the mobile Expo export, the Desktop v1 archives, Pylon binaries).
# Resolve `Dockerfile.incremental` from the exact currently-ready Cloud Run
# image digest instead, so this class of deploy only ever advances the
# service code and the independent (git-tracked, always-present) Desktop v2
# descriptor tree, and can never regress an already-served seed.
deploy_mode="${OA_UPDATES_DEPLOY_MODE:-auto}"
if [[ "$deploy_mode" == "auto" ]]; then
  if [[ "$seed_requested" == "1" ]]; then
    deploy_mode="full"
  else
    deploy_mode="incremental"
  fi
fi

if [[ "$deploy_mode" == "incremental" && "$seed_requested" == "1" ]]; then
  echo "REFUSED: OA_UPDATES_DEPLOY_MODE=incremental cannot be combined with a seed publish (OA_SEED_DIST/OA_SEED_RUNTIME/OA_DESKTOP_RELEASES_DIST); Dockerfile.incremental never bakes those directories, so this would silently drop the requested seed" >&2
  exit 1
fi

build_args=()

case "$deploy_mode" in
  full)
    args=(
      run deploy "$service"
      --source .
      --region "$region" \
      --allow-unauthenticated \
      --port 8080 \
      # Additive by construction: gcloud's update form preserves every
      # existing env mapping not named in this invocation.
      --update-env-vars "$env_csv"
    )
    ;;
  incremental)
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

    args=(
      run deploy "$service"
      --image "$deploy_image"
      --region "$region" \
      --allow-unauthenticated \
      --port 8080 \
      # Additive by construction: gcloud's update form preserves every
      # existing env mapping not named in this invocation. This branch also
      # never touches the mobile/Desktop-v1/Pylon seed layers baked into
      # $base_image, so a Desktop-v2-only publication can neither remove nor
      # silently blank the mobile seed.
      --update-env-vars "$env_csv"
    )
    ;;
  *)
    echo "REFUSED: OA_UPDATES_DEPLOY_MODE must be auto, full, or incremental" >&2
    exit 1
    ;;
esac

# OA_SIGNING_KEY is mounted from Secret Manager, never inline (#8530).
# Update only this named secret; unrelated secret mappings remain attached.
signing_secret="${OA_SIGNING_SECRET-oa-updates-codesign-key:latest}"
if [[ -n "$signing_secret" ]]; then
  args+=(--update-secrets "OA_SIGNING_KEY=${signing_secret}")
fi

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" == "1" ]]; then
  if [[ "${#build_args[@]}" -gt 0 ]]; then
    printf 'BUILD_ARG=%s\n' "${build_args[@]}"
  fi
  printf 'DEPLOY_ARG=%s\n' "${args[@]}"
else
  (cd "$app_dir" && gcloud "${args[@]}")
fi
