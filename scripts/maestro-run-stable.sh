#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

flows=(
  ".maestro/flows/ui_drawer_settings.yaml"
  ".maestro/flows/settings_toggles.yaml"
  ".maestro/flows/ui_drawer_history_empty.yaml"
)

for f in "${flows[@]}"; do
  echo "==> Running $f"
  maestro test "$f"
done

echo "All stable flows completed."

