#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACTS_DIR="${OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR:-${ROOT_DIR}/target/pylon-ldk-wallet-regtest/latest}"

cd "$ROOT_DIR"

cat <<EOF
Pylon LDK wallet regtest harness
artifacts: ${ARTIFACTS_DIR}

This is an opt-in heavy harness. It starts local regtest bitcoind/electrsd via
the Rust integration test, opens a real channel, pays a BOLT11 invoice, checks
restart and restore state, and writes harness-summary.json.

Optional overrides:
  BITCOIND_EXE=/path/to/bitcoind
  ELECTRS_EXE=/path/to/electrs
  OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR=/path/to/artifacts
EOF

mkdir -p "$ARTIFACTS_DIR"
export OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR="$ARTIFACTS_DIR"

cargo test -p pylon pylon_ldk_wallet_harness_plan_covers_required_evidence --lib

if [[ -z "${ELECTRS_EXE:-}" && "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
  cat >&2 <<EOF
electrsd 0.36 may download an x86_64 electrs binary on Apple Silicon.
Install or build a native electrs binary and rerun:

  cargo install electrs --version 0.10.6 --root target/pylon-ldk-wallet-tools-0106 --locked
  ELECTRS_EXE=/path/to/electrs scripts/pylon/ldk-wallet-regtest-harness.sh

BITCOIND_EXE can also be set if the downloaded bitcoind binary is unavailable.
EOF
  exit 2
fi

cargo test -p pylon --test ldk_wallet_regtest_harness -- --ignored --nocapture

if command -v jq >/dev/null 2>&1; then
  jq -e '
    .channel_readiness.cases[]
    | select(.case == "no_channel_before_open"
      and .can_receive_lightning == false
      and .warning_code == "lightning_receive_unavailable_no_channels")
  ' "${ARTIFACTS_DIR}/harness-summary.json" >/dev/null
  jq -e '
    .channel_readiness.cases[]
    | select(.case == "usable_channel_after_open"
      and .can_receive_lightning == true
      and .inbound_sats > 0)
  ' "${ARTIFACTS_DIR}/harness-summary.json" >/dev/null
  jq -e '
    .channel_readiness.cases[]
    | select(.case == "route_failure_projection"
      and .warning_code == "lightning_receive_needs_inbound_liquidity")
  ' "${ARTIFACTS_DIR}/harness-summary.json" >/dev/null
else
  echo "jq not found; Rust harness assertions already checked channel readiness cases." >&2
fi

echo "Pylon LDK wallet regtest harness complete."
echo "Summary: ${ARTIFACTS_DIR}/harness-summary.json"
