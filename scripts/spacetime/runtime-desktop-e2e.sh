#!/usr/bin/env bash
set -euo pipefail

run_case() {
  local label="$1"
  shift
  echo "==> $label"
  "$@"
}

run_case "runtime publisher idempotency" \
  cargo test -p openagents-runtime-service spacetime_publisher::tests::publisher_is_idempotent_on_duplicate_publish -- --nocapture

run_case "runtime publisher sequence conflict guard" \
  cargo test -p openagents-runtime-service spacetime_publisher::tests::publisher_rejects_out_of_order_sequence_for_same_topic -- --nocapture

run_case "runtime publisher durable outbox on network failure" \
  cargo test -p openagents-runtime-service spacetime_publisher::tests::http_publish_failure_queues_outbox_for_retry -- --nocapture

run_case "runtime retired route guard" \
  cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture

run_case "desktop parser rejects legacy frame protocol" \
  cargo test -p autopilot-desktop runtime_codex_proto::tests::parse_spacetime_server_message_rejects_legacy_phoenix_frames -- --nocapture

run_case "desktop stream event extraction on Spacetime update shape" \
  cargo test -p autopilot-desktop runtime_codex_proto::tests::extract_runtime_events_from_spacetime_update_filters_stream_and_worker -- --nocapture

run_case "desktop handshake dedupe under replay/retry" \
  cargo test -p autopilot-desktop runtime_codex_proto::tests::handshake_retry_harness_replays_until_ack_is_observed_once -- --nocapture

echo "Runtime-to-desktop Spacetime e2e suite passed."
