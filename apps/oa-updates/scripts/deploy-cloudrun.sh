#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd "$script_dir/.." && pwd)"
repo_dir="$(cd "$app_dir/../.." && pwd)"

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" != "1" ]]; then
  (cd "$repo_dir" && pnpm --dir apps/oa-updates run build:server)
fi

service="${OA_UPDATES_SERVICE:-oa-updates}"
project="${OA_UPDATES_PROJECT:-openagentsgemini}"
region="${OA_UPDATES_REGION:-us-central1}"
build_service_account="projects/${project}/serviceAccounts/oa-cloud-run-source-builder@${project}.iam.gserviceaccount.com"
image_build_service_account="projects/${project}/serviceAccounts/oa-cloud-image-builder@${project}.iam.gserviceaccount.com"
image_build_source="gs://${project}-cloud-build-source/source"

# Deploy OpenAgents Updates to Cloud Run from the oa-updates app directory.
#
# Required before running:
#   gcloud auth login
#   gcloud config set project <project-id>
#   export OA_PUBLIC_URL=https://<your-cloud-run-service-url>
#
# Optional, for a Pylon release publish only:
#   export OA_PYLON_RELEASES_DIST=/app/pylon-dist
#
# The service serves ONE surface: the signed per-platform Pylon release feed
# (plus Pylon node discovery). The Expo mobile OTA surface was retired on
# 2026-08-05 (#9325) after the owner confirmed there are no installed mobile
# users, so this script no longer emits OA_SEED_DIST, OA_SEED_RUNTIME,
# OA_SEED_PLATFORM, OA_SEED_BRANCH, OA_SEED_EXPO_CLIENT_PATH, or the
# OA_SIGNING_KEY manifest code-signing secret (#8530 / CFG-14) — src/serve.ts
# no longer reads any of them, so a stale operator environment cannot
# resurrect a retired feed.
#
# This script is intentionally not run by tests or setup. Run it manually when
# the target Google Cloud project is ready.

env_vars=("OA_PUBLIC_URL=${OA_PUBLIC_URL:?set OA_PUBLIC_URL}")

# A seed-publishing deploy intentionally replaces the release bytes baked into
# the image (the Pylon binaries in pylon-dist/). Track whether this invocation
# is one of those so image selection below can require the matching full
# rebuild.
seed_requested=0

if [[ -n "${OA_PYLON_RELEASES_DIST:-}" ]]; then
  seed_requested=1
  env_vars+=("OA_PYLON_RELEASES_DIST=${OA_PYLON_RELEASES_DIST}")
fi

env_csv="$(IFS=,; echo "${env_vars[*]}")"

# Image selection ------------------------------------------------------------
#
# `--source .` bakes whatever currently sits in this checkout's gitignored
# pylon-dist/ directory into a brand new image layer. That is correct, and
# required, exactly when this deploy is intentionally publishing fresh signed
# Pylon binaries (OA_PYLON_RELEASES_DIST) -- publish-pylon-release.ts stages
# those bytes right before this script runs for exactly that reason, and must
# keep doing a full rebuild so the staged binaries actually ship.
#
# Any OTHER deploy -- notably a bare server code push -- must NOT go through
# `--source .`, because that seed directory is almost always empty or stale in
# an ordinary checkout at that moment, and Docker COPY of an empty local
# directory silently erases the release bytes already baked into the currently
# running image (the Pylon binaries). Resolve `Dockerfile.incremental` from the
# exact currently-ready Cloud Run image digest instead, so this class of deploy
# only ever advances the service code and can never regress an already-served
# seed.
deploy_mode="${OA_UPDATES_DEPLOY_MODE:-auto}"
if [[ "$deploy_mode" == "auto" ]]; then
  if [[ "$seed_requested" == "1" ]]; then
    deploy_mode="full"
  else
    deploy_mode="incremental"
  fi
fi

if [[ "$deploy_mode" == "incremental" && "$seed_requested" == "1" ]]; then
  echo "REFUSED: OA_UPDATES_DEPLOY_MODE=incremental cannot be combined with a seed publish (OA_PYLON_RELEASES_DIST); Dockerfile.incremental never bakes that directory, so this would silently drop the requested seed" >&2
  exit 1
fi

build_args=()

case "$deploy_mode" in
  full)
    args=(
      run deploy "$service"
      --project "$project"
      --source .
      --build-service-account "$build_service_account"
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
      --project "$project"
      --region "$region"
      --service-account "$image_build_service_account"
      --gcs-source-staging-dir "$image_build_source"
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
      # never touches the Pylon seed layer baked into $base_image, so a
      # code-only deploy can neither remove nor silently blank it.
      --update-env-vars "$env_csv"
    )
    ;;
  *)
    echo "REFUSED: OA_UPDATES_DEPLOY_MODE must be auto, full, or incremental" >&2
    exit 1
    ;;
esac

# No secret is attached by this deploy. The OA_SIGNING_KEY mapping this script
# used to set (#8530 / CFG-14) signed Expo manifests only, and the mobile OTA
# surface it served was retired on 2026-08-05 (#9325). Pylon release signatures
# are minted offline by scripts/sign-release.ts and verified by clients against
# the pinned public key in keys/release-pubkey.json, so the running service
# holds no signing material at all.

if [[ "${OA_UPDATES_DEPLOY_DRY_RUN:-0}" == "1" ]]; then
  if [[ "${#build_args[@]}" -gt 0 ]]; then
    printf 'BUILD_ARG=%s\n' "${build_args[@]}"
  fi
  printf 'DEPLOY_ARG=%s\n' "${args[@]}"
else
  (cd "$app_dir" && gcloud "${args[@]}")
fi
