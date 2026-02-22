#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
IOS_PROJECT="${ROOT_DIR}/apps/autopilot-ios/Autopilot/Autopilot.xcodeproj"
IOS_SCHEME="Autopilot"

run_rust_case() {
  local label="$1"
  local filter="$2"
  echo "==> rust parity: ${label}"
  (
    cd "${ROOT_DIR}"
    cargo test -p openagents-client-core "${filter}"
  )
}

run_rust_case "auth send/verify race semantics" "auth_flow_state_drops_stale_send_completion"
run_rust_case "auth stale verify invalidation" "auth_flow_state_rejects_stale_verify_responses"
run_rust_case "worker selection ranking" "prefers_desktop_shared_running_worker_when_available"
run_rust_case "worker selection running fallback" "falls_back_to_running_workers_when_no_desktop_worker_is_running"
run_rust_case "desktop handshake ack extraction" "extract_desktop_handshake_ack_id_requires_proto_fields"
run_rust_case "khala stream resume subscribe path" "session_starts_with_join_and_subscribe_on_reply"
run_rust_case "khala stream stale-cursor mapping" "stale_cursor_maps_conflict_error"
run_rust_case "thread start + interrupt control lifecycle" "control_coordinator_turn_start_then_interrupt_scenario"
run_rust_case "reconnect backoff policy" "khala_reconnect_policy_uses_bounded_backoff"
run_rust_case "reconnect error classification" "khala_reconnect_classifier_maps_failure_classes"

if [[ "${OA_IOS_SKIP_XCODE_TESTS:-0}" == "1" ]]; then
  echo "Skipping xcodebuild parity tests (OA_IOS_SKIP_XCODE_TESTS=1)."
  echo "iOS Codex parity harness passed (Rust-only mode)."
  exit 0
fi

if ! command -v xcodebuild >/dev/null 2>&1 || ! command -v xcrun >/dev/null 2>&1; then
  echo "Missing xcodebuild/xcrun; cannot run iOS integration parity tests." >&2
  exit 1
fi

destination_id="$({
  xcrun simctl list devices available \
    | rg 'iPhone .*\(([0-9A-F-]{8}-[0-9A-F-]{4}-[0-9A-F-]{4}-[0-9A-F-]{4}-[0-9A-F-]{12})\) \((Booted|Shutdown)\)' -or '$1' \
    | head -n 1
} || true)"

if [[ -z "${destination_id}" ]]; then
  echo "No available iOS simulator destination found for parity harness." >&2
  exit 1
fi

echo "==> ios parity: runtime codex app-server integration tests"
(
  cd "${ROOT_DIR}"
  xcodebuild \
    -project "${IOS_PROJECT}" \
    -scheme "${IOS_SCHEME}" \
    -destination "id=${destination_id}" \
    -only-testing:AutopilotTests/AutopilotTests/runtimeCodexClientAuthWorkerAndSyncApisCoverAppServerContracts \
    -only-testing:AutopilotTests/AutopilotTests/runtimeCodexClientRequestStopApisEncodeAndMapErrors \
    -only-testing:AutopilotTests/AutopilotTests/rustBridgeWorkerSelectionParity \
    test
)

echo "iOS Codex parity harness passed."
