#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
kubectl_bin="${KUBECTL_BIN:-kubectl}"
strict_config="${script_dir}/livekit.yaml"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/openagents-livekit-post-render.XXXXXX")"

cleanup() {
  case "${temporary_root}" in
    "${TMPDIR:-/tmp}"/openagents-livekit-post-render.*)
      rm -rf -- "${temporary_root}"
      ;;
    *)
      printf 'refusing to clean unexpected post-render path: %s\n' "${temporary_root}" >&2
      ;;
  esac
}
trap cleanup EXIT

cp "${script_dir}/post-render/kustomization.yaml" "${temporary_root}/kustomization.yaml"
cp "${script_dir}/post-render/deployment.patch.yaml" "${temporary_root}/deployment.patch.yaml"
cp "${script_dir}/post-render/service-account.patch.yaml" "${temporary_root}/service-account.patch.yaml"
cp "${script_dir}/post-render/signaling-service.patch.yaml" "${temporary_root}/signaling-service.patch.yaml"
cp "${script_dir}/post-render/turn-service.patch.yaml" "${temporary_root}/turn-service.patch.yaml"
cp "${script_dir}/post-render/ingress.patch.yaml" "${temporary_root}/ingress.patch.yaml"

tee "${temporary_root}/chart-from-helm.yaml" >/dev/null
node "${script_dir}/strict-config-post-render.mjs" \
  "${temporary_root}/chart-from-helm.yaml" \
  "${strict_config}" >"${temporary_root}/chart.yaml"
"${kubectl_bin}" kustomize "${temporary_root}"
