#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/spacetime/provision-check.sh <dev|staging|prod>

Checks required Spacetime environment variables and probes health endpoint.
USAGE
}

if [[ "${1:-}" == "" ]]; then
  usage
  exit 1
fi

env_name="$(printf '%s' "$1" | tr '[:lower:]' '[:upper:]')"
case "$env_name" in
  DEV|STAGING|PROD)
    ;;
  *)
    usage
    exit 1
    ;;
esac

required_keys=(
  "OA_SPACETIME_${env_name}_HTTP_BASE_URL"
  "OA_SPACETIME_${env_name}_DATABASE"
  "OA_SPACETIME_${env_name}_TOKEN_ISSUER"
  "OA_SPACETIME_${env_name}_TOKEN_AUDIENCE"
  "OA_SPACETIME_${env_name}_JWT_SIGNING_KEY"
)

missing=0
for key in "${required_keys[@]}"; do
  if [[ -z "${!key:-}" ]]; then
    echo "missing required env: ${key}" >&2
    missing=1
  fi
done

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

base_url_var="OA_SPACETIME_${env_name}_HTTP_BASE_URL"
base_url="${!base_url_var}"
if [[ ! "$base_url" =~ ^https?:// ]]; then
  echo "invalid ${base_url_var}: must start with http:// or https://" >&2
  exit 1
fi

health_path_var="OA_SPACETIME_${env_name}_HEALTH_PATH"
health_path="${!health_path_var:-/health}"
health_url="${base_url%/}${health_path}"

echo "probing ${health_url}"
if ! curl -fsS --max-time 10 "$health_url" >/dev/null; then
  echo "health probe failed: ${health_url}" >&2
  exit 1
fi

echo "spacetime provision-check passed for ${env_name}"

