#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

flows=(
  ".maestro/flows/ui_thread_composer.yaml"
  ".maestro/flows/ui_drawer_settings.yaml"
  ".maestro/flows/settings_toggles.yaml"
  ".maestro/flows/ui_drawer_history_empty.yaml"
  ".maestro/flows/bridge_connect_manual.yaml"
  ".maestro/flows/bridge_connect_and_stream.yaml"
  ".maestro/flows/bridge_disconnect.yaml"
)

for f in "${flows[@]}"; do
  echo "==> Running $f"
  maestro test "$f" || {
    echo "Flow failed: $f" >&2
    exit 1
  }
done

echo "All flows completed."
