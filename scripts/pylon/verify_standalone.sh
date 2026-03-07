#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

CONFIG_PATH="$TMP_DIR/config.json"
export OPENAGENTS_PYLON_HOME="$TMP_DIR/home"

cd "$ROOT_DIR"

status_before_init="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" status)"
grep -q '^state: unconfigured$' <<<"$status_before_init"

cargo run -p pylon -- --config-path "$CONFIG_PATH" init >/dev/null

backends_json="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" backends --json)"
grep -q '"backend_id": "ollama"' <<<"$backends_json"
grep -q '"backend_id": "apple_foundation_models"' <<<"$backends_json"

products_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" products)"
grep -q '^product: ollama.text_generation$' <<<"$products_output"
grep -q '^product: ollama.embeddings$' <<<"$products_output"
if grep -q 'apple_foundation_models.embeddings' <<<"$products_output"; then
  echo "unexpected Apple FM embeddings product surfaced" >&2
  exit 1
fi

inventory_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" inventory)"
grep -q '^state: ' <<<"$inventory_output"

online_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" online)"
grep -q '^desired_mode: online$' <<<"$online_output"

pause_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" pause)"
grep -q '^state: paused$' <<<"$pause_output"

resume_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" resume)"
grep -q '^desired_mode: online$' <<<"$resume_output"

offline_output="$(cargo run -p pylon -- --config-path "$CONFIG_PATH" offline)"
grep -q '^desired_mode: offline$' <<<"$offline_output"

cargo run -p pylon -- --config-path "$CONFIG_PATH" jobs >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" earnings >/dev/null
cargo run -p pylon -- --config-path "$CONFIG_PATH" receipts >/dev/null

echo "Pylon standalone verification passed."
