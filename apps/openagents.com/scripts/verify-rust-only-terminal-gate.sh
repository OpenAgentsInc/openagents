#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command rg

cd "${ROOT_DIR}"

active_impl_files="$({
  rg --files \
    apps/openagents.com/service \
    apps/openagents.com/web-shell \
    apps/openagents.com/scripts \
    --glob '!apps/openagents.com/scripts/archived-laravel/**'
} | rg '\.(php|ts|tsx)$' || true)"

if [[ -n "${active_impl_files}" ]]; then
  echo "error: non-Rust implementation files found in active openagents.com lanes:" >&2
  echo "${active_impl_files}" >&2
  exit 1
fi

active_legacy_runtime_commands="$({
  rg -n \
    'php artisan|composer install|composer update|npm run dev|npm run build:ssr|laravel-vite-plugin' \
    apps/openagents.com/scripts \
    --glob '!apps/openagents.com/scripts/verify-rust-only-terminal-gate.sh' \
    --glob '!apps/openagents.com/scripts/archived-laravel/**'
} || true)"

if [[ -n "${active_legacy_runtime_commands}" ]]; then
  echo "error: legacy php/typescript runtime command references found in active scripts:" >&2
  echo "${active_legacy_runtime_commands}" >&2
  exit 1
fi

all_impl_files="$({
  rg --files \
    apps/openagents.com \
    --glob '!apps/openagents.com/node_modules/**' \
    --glob '!apps/openagents.com/vendor/**' \
    --glob '!apps/openagents.com/public/build/**'
} | rg '\.(php|ts|tsx)$' || true)"

unexpected_paths=""
if [[ -n "${all_impl_files}" ]]; then
  while IFS= read -r path; do
    case "${path}" in
      apps/openagents.com/app/* | \
      apps/openagents.com/bootstrap/* | \
      apps/openagents.com/config/* | \
      apps/openagents.com/database/* | \
      apps/openagents.com/resources/* | \
      apps/openagents.com/routes/* | \
      apps/openagents.com/tests/* | \
      apps/openagents.com/scripts/archived-laravel/* | \
      apps/openagents.com/public/index.php | \
      apps/openagents.com/vite.config.ts)
        ;;
      *)
        unexpected_paths+="${path}"$'\n'
        ;;
    esac
  done <<<"${all_impl_files}"
fi

if [[ -n "${unexpected_paths}" ]]; then
  echo "error: php/typescript files exist outside archived legacy directories:" >&2
  printf "%s" "${unexpected_paths}" >&2
  exit 1
fi

if ! rg -q "Legacy PHP/TypeScript implementation lanes" \
  apps/openagents.com/docs/archived/legacy-php-typescript-implementation-archive.md; then
  echo "error: legacy implementation archive manifest missing" >&2
  exit 1
fi

./apps/openagents.com/service/scripts/verify-laravel-serving-retired.sh

echo "verify-rust-only-terminal-gate: pass"
