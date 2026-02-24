#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

TARGETS=(
  "crates/openagents-cli"
  "crates/autopilot_ui"
  "crates/runtime"
  "apps/openagents.com/web-shell"
  "crates/autopilot"
)

count_loc() {
  local path="$1"
  (
    cd "$ROOT_DIR"
    rg --files "$path" -g '*.rs' | xargs wc -l | tail -n1 | awk '{print $1}'
  )
}

count_markers() {
  local path="$1"
  (
    cd "$ROOT_DIR"
    rg -n "#\[test\]|#\[tokio::test|#\[cfg\(test\)\]" "$path" -g '*.rs' | wc -l | tr -d ' '
  )
}

printf "%-34s %10s %10s %14s\n" "target" "loc" "markers" "markers_per_kloc"
printf "%-34s %10s %10s %14s\n" "----------------------------------" "----------" "----------" "--------------"

for target in "${TARGETS[@]}"; do
  loc="$(count_loc "$target")"
  markers="$(count_markers "$target")"
  density="$(awk -v m="$markers" -v l="$loc" 'BEGIN { if (l == 0) { printf "0.00" } else { printf "%.2f", (m * 1000.0) / l } }')"
  printf "%-34s %10s %10s %14s\n" "$target" "$loc" "$markers" "$density"
done
