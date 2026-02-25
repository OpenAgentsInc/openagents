#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_test() {
  local filter="$1"
  cargo test --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" "${filter}" -- --nocapture
}

echo "[vignette-hydra-mvp2] running Gate L MVP-2 assertions"
run_test "server::tests::hydra_routing_score_emits_receipt_and_is_idempotent"
run_test "server::tests::hydra_routing_score_filters_cep_candidate_when_breaker_halts_envelopes"
run_test "server::tests::hydra_observability_endpoint_reports_mvp2_metrics"
run_test "server::tests::internal_openapi_route_includes_credit_and_hydra_endpoints_and_schemas"
echo "[vignette-hydra-mvp2] PASS"
