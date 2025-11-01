#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

flows=(
  ".maestro/flows/ui_thread_composer.yaml"
  ".maestro/flows/ui_drawer_settings.yaml"
  ".maestro/flows/settings_toggles.yaml"
  ".maestro/flows/ui_drawer_history_empty.yaml"
  ".maestro/flows/bridge_header_indicator.yaml"
)

for f in "${flows[@]}"; do
  echo "==> Running $f"
  if [[ -n "${MAESTRO_ENV_FILE:-}" ]]; then
    maestro test -e "$MAESTRO_ENV_FILE" "$f"
  else
    maestro test "$f"
  fi
done

echo "All stable flows completed."
