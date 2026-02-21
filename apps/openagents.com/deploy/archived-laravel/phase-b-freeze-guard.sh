#!/usr/bin/env bash
set -euo pipefail

# OA-RUST-111 Phase B freeze gate:
# Legacy Laravel deploy commands are blocked by default and can only be
# unblocked for an explicitly approved rollback/cutover window.
if [[ "${OA_LEGACY_LARAVEL_UNFREEZE:-0}" != "1" ]]; then
  cat >&2 <<'MSG'
error: legacy Laravel deploy lane is frozen (OA-RUST-111 Phase B).
error: this script is archived and blocked by default.
error: to run intentionally, set:
error:   OA_LEGACY_LARAVEL_UNFREEZE=1
error:   OA_LEGACY_LARAVEL_CHANGE_TICKET=<approved-ticket-id>
MSG
  exit 78
fi

if [[ -z "${OA_LEGACY_LARAVEL_CHANGE_TICKET:-}" ]]; then
  echo "error: OA_LEGACY_LARAVEL_CHANGE_TICKET is required when unfreezing legacy deploy lane" >&2
  exit 78
fi

echo "[legacy-unfreeze] approved ticket: ${OA_LEGACY_LARAVEL_CHANGE_TICKET}" >&2
