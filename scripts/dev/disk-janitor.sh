#!/usr/bin/env bash
# Compatibility entry point. Cleanup previews by default; applying needs a saved plan.
set -euo pipefail
janitor_home=${JANITOR_HOME:-$HOME}
cleanup_bin=${OPENAGENTS_DISK_CLEANUP:-$HOME/.openagents/bin/openagents-disk-cleanup}
[ ! -e "$janitor_home/.openagents/disk-janitor.off" ] || exit 0
if [ ! -x "$cleanup_bin" ]; then
  echo 'Install the cleanup tool with scripts/dev/install-disk-janitor.sh, then try again.' >&2
  exit 1
fi
mode=scan
forward=()
for arg in "$@"; do
  case "$arg" in
    --now|--dry-run|--low) ;;
    --apply) mode=apply ;;
    *) forward+=("$arg") ;;
  esac
done
if [ "$mode" = scan ]; then
  exec "$cleanup_bin" scan --home "$janitor_home" --age-hours "${JANITOR_TARGET_IDLE_HOURS:-24}" "${forward[@]}"
fi
exec "$cleanup_bin" apply --home "$janitor_home" "${forward[@]}"
