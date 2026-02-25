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

status=0

while IFS=$'\t' read -r store path existed backup_path; do
  if [[ "${existed}" == "true" ]]; then
    if [[ -z "${backup_path}" || ! -f "${backup_path}" ]]; then
      echo "error: [${store}] missing backup for restore: ${backup_path}" >&2
      status=1
      continue
    fi

    mkdir -p "$(dirname "${path}")"
    cp "${backup_path}" "${path}"
    echo "restored: [${store}] ${path}"
  else
    if [[ -f "${path}" ]]; then
      rm -f "${path}"
      echo "removed: [${store}] ${path}"
    else
      echo "skipped: [${store}] ${path} (not present)"
    fi
  fi
done < <(jq -r '.stores[] | [.store, .path, (.existed|tostring), (.backup_path // "")] | @tsv' "${MANIFEST_PATH}")

if [[ "${status}" -ne 0 ]]; then
  exit "${status}"
fi

echo "rollback completed using manifest: ${MANIFEST_PATH}"
