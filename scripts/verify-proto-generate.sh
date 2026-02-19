#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

TEMPLATE_PATH="${TMP_DIR}/buf.gen.verify.yaml"

cat > "${TEMPLATE_PATH}" <<EOF
version: v2

managed:
  enabled: false

plugins:
  - remote: buf.build/protocolbuffers/php
    out: ${TMP_DIR}/generated/php
  - remote: buf.build/bufbuild/es
    out: ${TMP_DIR}/generated/ts
    opt:
      - target=ts
EOF

(
  cd "${ROOT_DIR}"
  buf generate --template "${TEMPLATE_PATH}"
)

ts_count="$(find "${TMP_DIR}/generated/ts" -type f | wc -l | tr -d ' ')"
php_count="$(find "${TMP_DIR}/generated/php" -type f | wc -l | tr -d ' ')"

if [[ "${ts_count}" -eq 0 || "${php_count}" -eq 0 ]]; then
  echo "proto generation verification failed: expected non-empty TS and PHP outputs" >&2
  echo "ts_count=${ts_count} php_count=${php_count}" >&2
  exit 1
fi

echo "proto generation verification passed (ts_files=${ts_count}, php_files=${php_count})"
