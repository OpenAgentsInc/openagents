#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="$(cd "${SERVICE_DIR}/.." && pwd)"
REPO_ROOT="$(git -C "${APP_DIR}" rev-parse --show-toplevel)"

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service}"
IMAGE="${IMAGE:-}"

if [[ -z "${PROJECT}" || "${PROJECT}" == "(unset)" ]]; then
  echo "error: PROJECT is required (or set active gcloud project)" >&2
  exit 1
fi

if [[ -z "${IMAGE}" ]]; then
  echo "error: IMAGE is required (Rust control-service image URI)." >&2
  exit 1
fi

echo "[deploy] rust web-shell dist build"
"${APP_DIR}/web-shell/build-dist.sh"

if [[ "${SKIP_VERIFY:-0}" != "1" ]]; then
  echo "[deploy] verify rust control-service + rust web-shell"
  cargo test --manifest-path "${SERVICE_DIR}/Cargo.toml"
  cargo check -p openagents-web-shell --target wasm32-unknown-unknown
  "${APP_DIR}/web-shell/scripts/sw-policy-verify.sh"
  "${APP_DIR}/web-shell/scripts/perf-budget-gate.sh"
fi

echo "[deploy] deploy no-traffic revision image=${IMAGE}"
PROJECT="${PROJECT}" REGION="${REGION}" SERVICE="${SERVICE}" \
  "${SCRIPT_DIR}/canary-rollout.sh" deploy-no-traffic "${IMAGE}"

echo "[deploy] done. continue with staged traffic in canary runbook:"
echo "  apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md"
