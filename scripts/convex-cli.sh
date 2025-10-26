#!/usr/bin/env bash
set -euo pipefail

ACTION="${1:-dev}"
shift || true

URL="${CONVEX_URL:-}"
ADMIN="${CONVEX_ADMIN_KEY:-${CONVEX_SELF_HOSTED_ADMIN_KEY:-}}"
if [ -f ".env.local" ]; then
  if [ -z "${URL}" ]; then URL=$(grep -E '^CONVEX_SELF_HOSTED_URL=' .env.local | sed -E 's/^[^=]+=//; s/\r$//') || true; fi
  if [ -z "${URL}" ]; then URL=$(grep -E '^CONVEX_URL=' .env.local | sed -E 's/^[^=]+=//; s/\r$//') || true; fi
  if [ -z "${ADMIN}" ]; then ADMIN=$(grep -E '^CONVEX_SELF_HOSTED_ADMIN_KEY=' .env.local | sed -E 's/^[^=]+=//; s/\r$//') || true; fi
  if [ -z "${ADMIN}" ]; then ADMIN=$(grep -E '^CONVEX_ADMIN_KEY=' .env.local | sed -E 's/^[^=]+=//; s/\r$//') || true; fi
fi

if [ -z "${URL}" ]; then URL="http://127.0.0.1:3210"; fi
if [ -z "${ADMIN}" ]; then
  echo "error: missing CONVEX_SELF_HOSTED_ADMIN_KEY or CONVEX_ADMIN_KEY in .env.local" >&2
  exit 1
fi

case "${ACTION}" in
  dev)
    exec convex dev --url "${URL}" --admin-key "${ADMIN}" --typecheck disable --codegen enable "$@"
    ;;
  dev:once)
    exec convex dev --once --url "${URL}" --admin-key "${ADMIN}" --typecheck disable --codegen enable "$@"
    ;;
  deploy)
    exec convex deploy --url "${URL}" --admin-key "${ADMIN}" --typecheck disable --codegen enable "$@"
    ;;
  *)
    echo "usage: scripts/convex-cli.sh [dev|dev:once|deploy]" >&2
    exit 2
    ;;
esac
