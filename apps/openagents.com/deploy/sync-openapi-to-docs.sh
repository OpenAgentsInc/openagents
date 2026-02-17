#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

DOCS_REPO="${DOCS_REPO:-${HOME}/code/docs}"
DOCS_BRANCH="${DOCS_BRANCH:-main}"
DOCS_OPENAPI_PATH="${DOCS_OPENAPI_PATH:-api/openapi.json}"
SKIP_PUSH="${SKIP_PUSH:-0}"

if [[ ! -d "${DOCS_REPO}/.git" ]]; then
  echo "error: docs repo not found at ${DOCS_REPO}" >&2
  exit 1
fi

TMP_OPENAPI="$(mktemp)"
cleanup() {
  rm -f "${TMP_OPENAPI}"
}
trap cleanup EXIT

echo "[openapi-sync] generating OpenAPI from ${APP_DIR}"
(
  cd "${APP_DIR}"
  php artisan openapi:generate --output="${TMP_OPENAPI}" --ansi >/dev/null
)

php -r '
$json = @file_get_contents($argv[1]);
if ($json === false) {
    fwrite(STDERR, "error: unable to read generated OpenAPI file\\n");
    exit(1);
}
json_decode($json, true);
if (json_last_error() !== JSON_ERROR_NONE) {
    fwrite(STDERR, "error: generated OpenAPI is invalid JSON: " . json_last_error_msg() . "\\n");
    exit(1);
}
' "${TMP_OPENAPI}"

TARGET="${DOCS_REPO}/${DOCS_OPENAPI_PATH}"
mkdir -p "$(dirname "${TARGET}")"
cp "${TMP_OPENAPI}" "${TARGET}"

echo "[openapi-sync] checking for docs changes in ${DOCS_REPO}"
(
  cd "${DOCS_REPO}"
  git add "${DOCS_OPENAPI_PATH}"
  if git diff --cached --quiet -- "${DOCS_OPENAPI_PATH}"; then
    echo "[openapi-sync] no OpenAPI changes detected; skipping docs commit/push"
    exit 0
  fi

  SOURCE_SHA="$(git -C "${APP_DIR}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
  COMMIT_MSG="docs(openapi): sync openagents OpenAPI from ${SOURCE_SHA}"
  git commit -m "${COMMIT_MSG}" -- "${DOCS_OPENAPI_PATH}"

  if [[ "${SKIP_PUSH}" == "1" ]]; then
    echo "[openapi-sync] SKIP_PUSH=1 set; leaving docs commit local only"
    exit 0
  fi

  git push origin "${DOCS_BRANCH}"
  echo "[openapi-sync] docs push complete (${DOCS_BRANCH})"
)
