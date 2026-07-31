#!/usr/bin/env bash

set -euo pipefail

if (($# > 1)) || (($# == 1 && "$1" != "--apply")); then
  printf 'usage: %s [--apply]\n' "$0" >&2
  exit 2
fi

apply="false"
if (($# == 1)); then
  apply="true"
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/../.." && pwd)"
project="openagentsgemini"
region="us-central1"
revision="$(git -C "${repository_root}" rev-parse HEAD)"
remote_revision="$(git -C "${repository_root}" ls-remote --exit-code origin refs/heads/main | awk '{print $1}')"
builder="projects/${project}/serviceAccounts/oa-livekit-image-builder@${project}.iam.gserviceaccount.com"
source_staging="gs://${project}-livekit-build-source/source"
image="${region}-docker.pkg.dev/${project}/oa-cloud/livekit-production-deployer:source-${revision}"
configuration="${repository_root}/docker/cloud/cloudbuild-livekit-production-deployer.yaml"

if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ || "${revision}" != "${remote_revision}" ]]; then
  printf 'refusing deployer build unless HEAD is current remote main\n' >&2
  exit 1
fi
if [[ -n "$(git -C "${repository_root}" status --porcelain --untracked-files=normal)" ]]; then
  printf 'refusing deployer build from a dirty worktree\n' >&2
  exit 1
fi

if [[ "${apply}" != "true" ]]; then
  printf 'Cloud Build dry run; no cloud state changed.\n'
  printf 'source revision: %s\n' "${revision}"
  printf 'build service account: %s\n' "${builder}"
  printf 'mutable build tag: %s\n' "${image}"
  printf 'rerun with --apply to publish and resolve the immutable digest\n'
  exit 0
fi

node "${repository_root}/scripts/google-cloud-authority-guard.mjs"
build_id="$(
  cd "${repository_root}"
  gcloud builds submit . \
    --project "${project}" \
    --region "${region}" \
    --service-account "${builder}" \
    --gcs-source-staging-dir "${source_staging}" \
    --async \
    --format='value(id)' \
    --config "${configuration}" \
    --substitutions "_IMAGE=${image}"
)"
if [[ ! "${build_id}" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$ ]]; then
  printf 'Cloud Build did not return a canonical build id\n' >&2
  exit 1
fi

build_status=""
for _ in $(seq 1 240); do
  build_status="$(
    gcloud builds describe "${build_id}" \
      --project "${project}" \
      --region "${region}" \
      --format='value(status)'
  )"
  case "${build_status}" in
    SUCCESS)
      break
      ;;
    FAILURE|INTERNAL_ERROR|TIMEOUT|CANCELLED|EXPIRED)
      printf 'LiveKit deployer image build failed: %s (%s)\n' \
        "${build_id}" "${build_status}" >&2
      exit 1
      ;;
  esac
  sleep 5
done
if [[ "${build_status}" != "SUCCESS" ]]; then
  printf 'LiveKit deployer image build did not reach SUCCESS: %s\n' \
    "${build_id}" >&2
  exit 1
fi

digest="$(
  gcloud artifacts docker images describe "${image}" \
    --project "${project}" \
    --format='value(image_summary.digest)'
)"
if [[ ! "${digest}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  printf 'published deployer image did not resolve to an immutable digest\n' >&2
  exit 1
fi

printf 'Cloud Build id: %s\n' "${build_id}"
printf 'TF_VAR_deployment_executor_image=%s@%s\n' \
  "${region}-docker.pkg.dev/${project}/oa-cloud/livekit-production-deployer" "${digest}"
