#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "==> runtime sync parity checks"
cargo test -p openagents-runtime-service server::tests::spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture
cargo test -p openagents-runtime-service spacetime_publisher::tests::publisher_is_idempotent_on_duplicate_publish -- --nocapture
cargo test -p openagents-runtime-service spacetime_publisher::tests::publisher_rejects_out_of_order_sequence_for_same_topic -- --nocapture
cargo test -p openagents-runtime-service spacetime_publisher::tests::http_publish_failure_queues_outbox_for_retry -- --nocapture
cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture

echo "==> shared client replay/resume checks"
cargo test -p autopilot-spacetime client::tests -- --nocapture

echo "==> desktop replay/resume checks"
cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture
cargo test -p autopilot-desktop sync_apply_engine -- --nocapture
cargo test -p autopilot-desktop sync_lifecycle -- --nocapture

echo "Spacetime replay/resume parity harness passed."
