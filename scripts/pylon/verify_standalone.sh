#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

CONFIG_PATH="$TMP_DIR/config.json"
export OPENAGENTS_PYLON_HOME="$TMP_DIR/home"
PYLON_CARGO=(cargo run -p pylon --bin pylon --)

cd "$ROOT_DIR"

status_before_init="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" status)"
grep -q '^state: unconfigured$' <<<"$status_before_init"

"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" init >/dev/null
ISOLATED_ADMIN_PORT="$((24000 + RANDOM % 10000))"
"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" config set admin_listen_addr "127.0.0.1:$ISOLATED_ADMIN_PORT" >/dev/null

backends_json="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" backends --json)"
grep -q '"backend_id": "local_gemma"' <<<"$backends_json"
grep -q '"backend_id": "sandbox"' <<<"$backends_json"

products_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" products)"
grep -q '^product: psionic.local.inference.gemma.single_node$' <<<"$products_output"
grep -q '^product: psionic.cluster.training.adapter_contributor.cluster_attached$' <<<"$products_output"
if grep -q 'apple_foundation_models' <<<"$products_output"; then
  echo "unexpected legacy Apple FM product surfaced" >&2
  exit 1
fi

sandbox_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" sandbox)"
grep -q '^supported_execution_classes: ' <<<"$sandbox_output"

inventory_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" inventory)"
grep -q '^state: ' <<<"$inventory_output"

online_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" online)"
grep -q '^provider_mode: online$' <<<"$online_output"

pause_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" pause)"
grep -q '^provider_mode: paused$' <<<"$pause_output"

resume_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" resume)"
grep -q '^provider_mode: online$' <<<"$resume_output"

offline_output="$("${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" offline)"
grep -q '^provider_mode: offline$' <<<"$offline_output"

"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" jobs >/dev/null
"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" earnings >/dev/null
"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" receipts >/dev/null
"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" activity >/dev/null
"${PYLON_CARGO[@]}" --config-path "$CONFIG_PATH" payout >/dev/null

echo "Pylon standalone verification passed."
