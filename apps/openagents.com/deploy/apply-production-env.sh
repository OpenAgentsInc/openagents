#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

ENV_FILE="${1:-${APP_DIR}/.env.production}"
GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
GCP_REGION="${GCP_REGION:-us-central1}"
CLOUD_RUN_SERVICE="${CLOUD_RUN_SERVICE:-openagents-web}"
DRY_RUN="${DRY_RUN:-0}"

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "error: env file not found: ${ENV_FILE}" >&2
  exit 1
fi

# Explicit non-secret allowlist. Secrets must go through Secret Manager + --update-secrets.
declare -a ALLOWLIST_KEYS=(
  APP_ENV
  APP_DEBUG
  APP_URL
  ASSET_URL
  SESSION_DOMAIN
  LOG_CHANNEL
  AI_DEFAULT

  DB_CONNECTION
  DB_HOST
  DB_DATABASE
  DB_USERNAME

  CACHE_STORE
  QUEUE_CONNECTION
  SESSION_DRIVER
  REDIS_HOST
  REDIS_PORT

  L402_ALLOWLIST_HOSTS
  L402_CREDENTIAL_TTL_SECONDS
  L402_RESPONSE_MAX_BYTES
  L402_RESPONSE_PREVIEW_BYTES
  L402_PAYMENT_TIMEOUT_MS
  L402_INVOICE_PAYER
  L402_DEMO_SATS4AI_URL

  LND_REST_BASE_URL
  LND_REST_TLS_VERIFY
)

# Optional additional allowlist keys can be supplied at runtime:
#   OA_ENV_ALLOWLIST_EXTRA="KEY_A,KEY_B"
if [[ -n "${OA_ENV_ALLOWLIST_EXTRA:-}" ]]; then
  IFS=',' read -r -a extra_keys <<<"${OA_ENV_ALLOWLIST_EXTRA}"
  for key in "${extra_keys[@]}"; do
    key="${key//[[:space:]]/}"
    if [[ -n "${key}" ]]; then
      ALLOWLIST_KEYS+=("${key}")
    fi
  done
fi

is_allowed_key() {
  local target="$1"
  local key
  for key in "${ALLOWLIST_KEYS[@]}"; do
    if [[ "${key}" == "${target}" ]]; then
      return 0
    fi
  done
  return 1
}

trim() {
  local s="$1"
  # shellcheck disable=SC2001
  s="$(echo "${s}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  printf '%s' "${s}"
}

strip_wrapping_quotes() {
  local v="$1"
  if [[ "${v}" =~ ^\".*\"$ ]]; then
    printf '%s' "${v:1:${#v}-2}"
    return
  fi
  if [[ "${v}" =~ ^\'.*\'$ ]]; then
    printf '%s' "${v:1:${#v}-2}"
    return
  fi
  printf '%s' "${v}"
}

declare -a pairs=()
declare -a applied=()
declare -a skipped=()

while IFS= read -r raw || [[ -n "${raw}" ]]; do
  line="${raw%$'\r'}"
  line="$(trim "${line}")"

  if [[ -z "${line}" || "${line}" == \#* ]]; then
    continue
  fi

  if [[ "${line}" == export\ * ]]; then
    line="${line#export }"
    line="$(trim "${line}")"
  fi

  if [[ "${line}" != *=* ]]; then
    echo "warn: skipping malformed line: ${line}" >&2
    continue
  fi

  key="${line%%=*}"
  value="${line#*=}"
  key="$(trim "${key}")"
  value="$(trim "${value}")"
  value="$(strip_wrapping_quotes "${value}")"

  if [[ -z "${key}" ]]; then
    continue
  fi

  if is_allowed_key "${key}"; then
    # Use custom delimiter for --update-env-vars so commas inside values are safe.
    value="${value//|/\\|}"
    pairs+=("${key}=${value}")
    applied+=("${key}")
  else
    skipped+=("${key}")
  fi
done <"${ENV_FILE}"

if (( ${#pairs[@]} == 0 )); then
  echo "error: no non-secret keys found to apply from ${ENV_FILE}" >&2
  exit 1
fi

env_arg="^|^$(IFS='|'; echo "${pairs[*]}")"

echo "Applying non-secret env vars to Cloud Run service ${CLOUD_RUN_SERVICE} (${GCP_PROJECT}/${GCP_REGION})"
echo "Env file: ${ENV_FILE}"
echo "Applied keys (${#applied[@]}): ${applied[*]}"
if (( ${#skipped[@]} > 0 )); then
  echo "Skipped keys (${#skipped[@]}) (not allowlisted): ${skipped[*]}"
fi

cmd=(
  gcloud run services update "${CLOUD_RUN_SERVICE}"
  --project "${GCP_PROJECT}"
  --region "${GCP_REGION}"
  --update-env-vars "${env_arg}"
)

if [[ "${DRY_RUN}" == "1" ]]; then
  echo
  echo "DRY_RUN=1, command not executed."
  printf 'Command: '
  printf '%q ' "${cmd[@]}"
  echo
  exit 0
fi

"${cmd[@]}"

echo "Done."
