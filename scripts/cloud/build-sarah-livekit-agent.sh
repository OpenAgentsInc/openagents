#!/usr/bin/env bash

set -euo pipefail

usage() {
  printf 'usage: %s [--apply]\n' "$0" >&2
}

apply="false"
if (($# > 1)); then
  usage
  exit 2
fi
if (($# == 1)); then
  if [[ "$1" != "--apply" ]]; then
    usage
    exit 2
  fi
  apply="true"
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/../.." && pwd)"
project="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
region="${OPENAGENTS_GCP_REGION:-us-central1}"
repository="oa-cloud"
image_name="sarah-livekit-agent"
revision="$(git -C "${repository_root}" rev-parse HEAD)"
remote_revision="$(git -C "${repository_root}" ls-remote --exit-code origin refs/heads/main | awk '{print $1}')"

if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'refusing worker build without an exact Git source revision\n' >&2
  exit 1
fi
if [[ "${revision}" != "${remote_revision}" ]]; then
  printf 'refusing worker build unless HEAD is current remote main\n' >&2
  exit 1
fi
if [[ -n "$(git -C "${repository_root}" status --porcelain --untracked-files=normal)" ]]; then
  printf 'refusing worker build from a dirty Git worktree\n' >&2
  exit 1
fi

image_tag="${region}-docker.pkg.dev/${project}/${repository}/${image_name}:source-${revision}"
configuration="${repository_root}/docker/cloud/cloudbuild-sarah-livekit-agent.yaml"
ignore_file="${repository_root}/.gcloudignore.sarah-livekit-agent"
substitutions="_IMAGE=${image_tag},_REVISION=${revision}"
builder="projects/${project}/serviceAccounts/oa-livekit-image-builder@${project}.iam.gserviceaccount.com"
source_staging="gs://${project}-livekit-build-source/source"

if [[ "${apply}" != "true" ]]; then
  printf 'Cloud Build dry run; no cloud state changed.\n'
  printf 'source revision: %s\n' "${revision}"
  printf 'mutable build tag: %s\n' "${image_tag}"
  printf 'build service account: %s\n' "${builder}"
  printf 'command: gcloud builds submit . --project %s --region %s --service-account %s --config %s --ignore-file %s --substitutions %s\n' \
    "${project}" "${region}" "${builder}" "${configuration}" "${ignore_file}" "${substitutions}"
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
    --ignore-file "${ignore_file}" \
    --substitutions "${substitutions}"
)"
if [[ -z "${build_id}" ]]; then
  printf 'Cloud Build did not return a build id\n' >&2
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
      printf 'Sarah LiveKit worker build failed: %s (%s)\n' \
        "${build_id}" "${build_status}" >&2
      exit 1
      ;;
  esac
  sleep 5
done
if [[ "${build_status}" != "SUCCESS" ]]; then
  printf 'Sarah LiveKit worker build did not reach SUCCESS: %s\n' \
    "${build_id}" >&2
  exit 1
fi

digest="$(
  gcloud artifacts docker images describe "${image_tag}" \
    --project "${project}" \
    --format='value(image_summary.digest)'
)"
if [[ ! "${digest}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  printf 'published worker image did not resolve to an immutable digest\n' >&2
  exit 1
fi

printf 'Cloud Build id: %s\n' "${build_id}"
printf 'SARAH_LIVEKIT_AGENT_IMAGE=%s@%s\n' \
  "${region}-docker.pkg.dev/${project}/${repository}/${image_name}" "${digest}"
