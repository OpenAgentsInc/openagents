#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DEPLOY_SCRIPT="${APP_DIR}/service/deploy/deploy-production.sh"

cat <<'MSG' >&2
[deprecated] apps/openagents.com/deploy/deploy-production.sh no longer performs Laravel/Inertia/React deploys.
[deprecated] Active web runtime path is Rust-only (control service + rust wasm shell).
[deprecated] Forwarding to apps/openagents.com/service/deploy/deploy-production.sh.
MSG

exec "${RUST_DEPLOY_SCRIPT}" "$@"
