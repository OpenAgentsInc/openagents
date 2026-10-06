#!/usr/bin/env bash
# Retain bounded synthetic evidence; this runner has no wallet or host credentials.
set -euo pipefail
out=${1:?usage: later-worker.sh NEW_OUTPUT_DIRECTORY}
mkdir "$out"
out=$(cd "$out" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:?reuse the agent Cargo target}
export CODER_LABOR_ACCEPTANCE_DIR="$out"
cargo test -j 1 -p pay-ledger --test worker_profile -- --nocapture > "$out/worker-contract.log" 2>&1
cargo test -j 1 -p coder-labor bounded_coding_order_runs_separate_buyer_check_and_accepts_over_relay -- --nocapture > "$out/free-order.log" 2>&1
cargo test -j 1 -p pay-ledger --test payouts -- --nocapture > "$out/fake-payout.log" 2>&1
cat > "$out/qualification.txt" <<'REPORT'
Profile: openagents.independent-worker-qualification.v1
Synthetic source and fake rails only. No wallet request or paid worker dispatch.
Separate buyer/provider roles remain under one fixture operator.
Independent operator and funded service qualifications: UNVERIFIED.
No-spend order: see retained free-order report for delivery/check/acceptance,
exact duplicate, role reconstruction after relay loss, and unmetered costs.
Fake payouts: see log for crash, unknown, retry, destination, and conservation cases.
REPORT
