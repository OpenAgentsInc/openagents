#!/usr/bin/env bash
set -euo pipefail

# Non-interactive staging reconcile helper for Phase 2B.
# Requires env vars documented in apps/lightning-ops/README.md.

if [[ -z "${OA_LIGHTNING_OPS_CONVEX_URL:-}" ]]; then
  echo "OA_LIGHTNING_OPS_CONVEX_URL is required" >&2
  exit 1
fi
if [[ -z "${OA_LIGHTNING_OPS_SECRET:-}" ]]; then
  echo "OA_LIGHTNING_OPS_SECRET is required" >&2
  exit 1
fi
if [[ -z "${OA_LIGHTNING_OPS_GATEWAY_BASE_URL:-}" ]]; then
  echo "OA_LIGHTNING_OPS_GATEWAY_BASE_URL is required" >&2
  exit 1
fi
if [[ -z "${OA_LIGHTNING_OPS_CHALLENGE_URL:-}" ]]; then
  echo "OA_LIGHTNING_OPS_CHALLENGE_URL is required" >&2
  exit 1
fi
if [[ -z "${OA_LIGHTNING_OPS_PROXY_URL:-}" ]]; then
  echo "OA_LIGHTNING_OPS_PROXY_URL is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${APP_DIR}"
npm run smoke:staging -- --json --mode convex
