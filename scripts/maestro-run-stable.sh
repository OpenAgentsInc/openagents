#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

flows=(
  ".maestro/flows/ui_drawer_settings.yaml"
  ".maestro/flows/settings_toggles.yaml"
  ".maestro/flows/ui_drawer_history_empty.yaml"
  ".maestro/flows/bridge_header_indicator.yaml"
  ".maestro/flows/thread_send_and_assert.yaml"
)

for f in "${flows[@]}"; do
  echo "==> Running $f"
  if [[ -n "${MAESTRO_ENV_FILE:-}" ]]; then
    ENV_ARGS=()
    while IFS= read -r line || [[ -n "$line" ]]; do
      [[ -z "$line" || "$line" =~ ^# ]] && continue
      ENV_ARGS+=("-e" "$line")
    done < "$MAESTRO_ENV_FILE"
    maestro test "${ENV_ARGS[@]}" "$f"
  else
    maestro test "$f"
  fi
done

echo "All stable flows completed."
