#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/openagents-livekit-verify.XXXXXX")"

if ! command -v yq >/dev/null 2>&1; then
  printf 'required command is unavailable: yq\n' >&2
  exit 1
fi

cleanup() {
  case "${temporary_root}" in
    "${TMPDIR:-/tmp}"/openagents-livekit-verify.*)
      rm -rf -- "${temporary_root}"
      ;;
    *)
      printf 'refusing to clean unexpected verification path: %s\n' "${temporary_root}" >&2
      ;;
  esac
}
trap cleanup EXIT

"${script_dir}/render.sh" --output "${temporary_root}" >/dev/null
node "${script_dir}/verify.mjs" \
  "${script_dir}/bundle.json" \
  "${temporary_root}/livekit-production.yaml"
