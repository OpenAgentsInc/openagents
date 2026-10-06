#!/usr/bin/env bash
# Bounded synthetic qualification only; no production wallet or host credentials.
set -euo pipefail
out=${1:?usage: later-markets.sh NEW_OUTPUT_DIRECTORY}
mkdir "$out"
out=$(cd "$out" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:?reuse the agent Cargo target}
"$(dirname "$0")/later-worker.sh" "$out/worker"
cargo test -j 1 -p pay-ledger --test bid_profile --test custody_profile --test training_profile --test contribution_profile -- --nocapture > "$out/contracts.log" 2>&1
cargo run -q -p pay-ledger --example bid-qualification > "$out/bids.json"
cargo run -q -p pay-ledger --example custody-qualification > "$out/custody.json"
cat > "$out/qualification.txt" <<'REPORT'
No-spend and fake rails only. No production dispatch, deposit, release, refund, or payout.
Worker and training artifacts are synthetic, and fixture operator control is shared.
Independent operators, quoted provider latency, incremental coordination cost,
real training improvement, commercial custody, useful contributions, and funded
adoption: UNVERIFIED. See NEEDS_OWNER.md for exact qualification steps.
REPORT
