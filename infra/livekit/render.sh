#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pins_path="${script_dir}/pins.lock.json"
values_path="${script_dir}/production/values.yaml"
post_renderer="${script_dir}/production/post-render.sh"
resource_root="${script_dir}/production/resources"
helm_bin="${HELM_BIN:-helm}"
kubectl_bin="${KUBECTL_BIN:-kubectl}"
output_directory=""

usage() {
  printf 'usage: %s --output DIRECTORY\n' "$0" >&2
}

while (($# > 0)); do
  case "$1" in
    --output)
      if (($# < 2)); then
        usage
        exit 2
      fi
      output_directory="$2"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${output_directory}" ]]; then
  usage
  exit 2
fi

for command_name in node curl tar shasum "${helm_bin}" "${kubectl_bin}"; do
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    printf 'required command is unavailable: %s\n' "${command_name}" >&2
    exit 1
  fi
done

read_pin() {
  node -e '
    const fs = require("node:fs");
    const document = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    let value = document;
    for (const segment of process.argv[2].split(".")) value = value[segment];
    if (typeof value !== "string") process.exit(2);
    process.stdout.write(value);
  ' "${pins_path}" "$1"
}

chart_commit="$(read_pin helm.sourceCommit)"
chart_version="$(read_pin helm.chartVersion)"
chart_archive_url="$(read_pin helm.archiveUrl)"
chart_archive_sha256="$(read_pin helm.archiveSha256)"
chart_path="$(read_pin helm.chartPath)"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/openagents-livekit-render.XXXXXX")"

cleanup() {
  case "${temporary_root}" in
    "${TMPDIR:-/tmp}"/openagents-livekit-render.*)
      rm -rf -- "${temporary_root}"
      ;;
    *)
      printf 'refusing to clean unexpected render path: %s\n' "${temporary_root}" >&2
      ;;
  esac
}
trap cleanup EXIT

archive_path="${temporary_root}/livekit-helm.tar.gz"
curl --fail --silent --show-error --location "${chart_archive_url}" --output "${archive_path}"
actual_archive_sha256="$(shasum -a 256 "${archive_path}" | awk '{print $1}')"
if [[ "${actual_archive_sha256}" != "${chart_archive_sha256}" ]]; then
  printf 'LiveKit Helm archive digest mismatch: expected %s, got %s\n' \
    "${chart_archive_sha256}" "${actual_archive_sha256}" >&2
  exit 1
fi

tar -xzf "${archive_path}" -C "${temporary_root}"
chart_root="${temporary_root}/livekit-helm-${chart_commit}/${chart_path}"
actual_chart_version="$(
  node -e '
    const fs = require("node:fs");
    const match = fs.readFileSync(process.argv[1], "utf8").match(/^version:\s*"?([^"\s]+)"?\s*$/m);
    if (!match) process.exit(2);
    process.stdout.write(match[1]);
  ' "${chart_root}/Chart.yaml"
)"
if [[ "${actual_chart_version}" != "${chart_version}" ]]; then
  printf 'LiveKit chart version mismatch: expected %s, got %s\n' \
    "${chart_version}" "${actual_chart_version}" >&2
  exit 1
fi

chart_manifest="${temporary_root}/chart.yaml"
resource_manifest="${temporary_root}/resources.yaml"
rendered_manifest="${temporary_root}/livekit-production.yaml"

KUBECTL_BIN="${kubectl_bin}" "${helm_bin}" template livekit-server "${chart_root}" \
  --namespace livekit-system \
  --include-crds \
  --skip-tests \
  --kube-version 1.33.0 \
  --values "${values_path}" \
  --post-renderer "${post_renderer}" >"${chart_manifest}"

"${kubectl_bin}" kustomize "${resource_root}" >"${resource_manifest}"

{
  printf '# Generated from %s at chart %s. Do not edit.\n' "${chart_commit}" "${chart_version}"
  cat "${resource_manifest}"
  printf '\n---\n'
  cat "${chart_manifest}"
} >"${rendered_manifest}"

mkdir -p "${output_directory}"
output_path="${output_directory}/livekit-production.yaml"
cp "${rendered_manifest}" "${output_path}.tmp"
mv "${output_path}.tmp" "${output_path}"
printf '%s\n' "${output_path}"
