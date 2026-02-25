#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_test() {
  local filter="$1"
  cargo test --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" "${filter}" -- --nocapture
}

echo "[vignette-hydra-mvp3] running Gate L MVP-3 assertions"
run_test "server::tests::hydra_fx_rfq_endpoints_enforce_idempotency_and_readback"
run_test "fx::service::tests::rfq_create_or_get_conflicts_on_idempotency_drift"
run_test "server::tests::hydra_fx_quote_select_is_deterministic_and_tie_break_stable"
run_test "server::tests::hydra_fx_settle_releases_and_replays_without_double_spend"
run_test "server::tests::hydra_fx_settle_rejects_reservation_conflict"
run_test "server::tests::hydra_fx_settle_withholds_when_quote_expired"
run_test "server::tests::hydra_observability_tracks_fx_metrics"
run_test "server::tests::internal_openapi_route_includes_credit_and_hydra_endpoints_and_schemas"
echo "[vignette-hydra-mvp3] PASS"
