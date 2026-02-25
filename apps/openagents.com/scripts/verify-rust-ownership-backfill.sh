#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <manifest-path>" >&2
  exit 1
fi

MANIFEST_PATH="$1"

if [[ ! -f "${MANIFEST_PATH}" ]]; then
  echo "error: manifest not found: ${MANIFEST_PATH}" >&2
  exit 1
fi

require_command() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "error: missing required command: ${name}" >&2
    exit 1
  fi
}

require_command jq
require_command sha256sum

status=0

while IFS=$'\t' read -r store path expected_sha; do
  if [[ ! -f "${path}" ]]; then
    echo "error: [${store}] store file missing: ${path}" >&2
    status=1
    continue
  fi

  actual_sha="$(sha256sum "${path}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "error: [${store}] sha mismatch" >&2
    echo "  expected: ${expected_sha}" >&2
    echo "  actual:   ${actual_sha}" >&2
    status=1
    continue
  fi

  echo "ok: [${store}] ${path}"
done < <(jq -r '.stores[] | [.store, .path, .after_sha256] | @tsv' "${MANIFEST_PATH}")

if [[ "${status}" -ne 0 ]]; then
  exit "${status}"
fi

echo "rust store backfill verification passed: ${MANIFEST_PATH}"
