#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LANE="${1:-all}"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command for legacy comms lane: $cmd" >&2
    exit 1
  fi
}

run_laravel() {
  echo "==> Laravel comms security/replay lane"
  require_cmd php
  (
    cd "${ROOT_DIR}/apps/openagents.com"
    php artisan test \
      tests/Feature/Settings/IntegrationSettingsTest.php \
      tests/Feature/Settings/IntegrationSecretLifecycleTest.php \
      tests/Feature/Api/Webhooks/ResendWebhookPipelineTest.php \
      tests/Feature/Api/Internal/RuntimeSecretFetchTest.php \
      tests/Feature/Settings/CommsOwnershipProjectionFlowTest.php
  )
}

run_runtime() {
  echo "==> Runtime comms security/replay lane"
  require_cmd mix
  (
    cd "${ROOT_DIR}/apps/runtime"
    mix test --warnings-as-errors \
      test/openagents_runtime/tools/comms/kernel_test.exs \
      test/openagents_runtime/tools/comms/providers/resend_adapter_test.exs \
      test/openagents_runtime/integrations/laravel_secret_client_test.exs \
      test/openagents_runtime/integrations/comms_security_replay_matrix_test.exs \
      test/openagents_runtime/security/sanitization_integration_test.exs
  )
}

echo "==> legacy compatibility lane: comms security/replay matrix (non-Rust, opt-in)"

case "${LANE}" in
  laravel)
    run_laravel
    ;;
  runtime)
    run_runtime
    ;;
  all)
    run_laravel
    run_runtime
    ;;
  *)
    echo "Usage: scripts/comms-security-replay-matrix.sh [laravel|runtime|all]" >&2
    exit 2
    ;;
esac
