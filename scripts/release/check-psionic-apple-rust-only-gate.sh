#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
PSIONIC_REPO="${OPENAGENTS_PSIONIC_REPO:-$ROOT_DIR/../psionic}"
PSIONIC_TRAIN_SYSTEM_DOC="${OPENAGENTS_PSIONIC_TRAIN_SYSTEM_DOC:-$PSIONIC_REPO/docs/TRAIN_SYSTEM.md}"

log() {
  echo "[check-psionic-apple-rust-only-gate] $*"
}

die() {
  echo "[check-psionic-apple-rust-only-gate] ERROR: $*" >&2
  exit 1
}

require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

scan_forbidden_ref() {
  local description="$1"
  local pattern="$2"
  local matches
  matches="$(
    rg -n \
      --glob '!psionic-train/src/apple_toolkit.rs' \
      "$pattern" \
      apps/autopilot-desktop \
      scripts/release \
      "$PSIONIC_REPO/crates" \
      "$PSIONIC_REPO/scripts" \
      || true
  )"
  if [[ -n "$matches" ]]; then
    printf '%s\n' "$matches" >&2
    die "Found forbidden Rust-only boundary regression: ${description}"
  fi
}

require_command cargo
require_command rg
[[ -f "$PSIONIC_TRAIN_SYSTEM_DOC" ]] || die "Missing standalone Psionic train-system doc at ${PSIONIC_TRAIN_SYSTEM_DOC}; set OPENAGENTS_PSIONIC_REPO or OPENAGENTS_PSIONIC_TRAIN_SYSTEM_DOC"

log "Checking shipped Apple lane for toolkit/Python regressions"
scan_forbidden_ref \
  "authoritative toolkit training shell-out" \
  'run_apple_adapter_toolkit_training\('
scan_forbidden_ref \
  "authoritative toolkit export shell-out" \
  'run_apple_adapter_toolkit_export\('
scan_forbidden_ref \
  "toolkit installation discovery in shipped Apple path" \
  'AppleAdapterToolkitInstallation::discover\('
scan_forbidden_ref \
  "toolkit environment discovery in shipped Apple path" \
  'OPENAGENTS_APPLE_TOOLKIT_(ROOT|PYTHON)'
scan_forbidden_ref \
  "toolkit python discovery helper usage in shipped Apple path" \
  'discover_python_path\('
scan_forbidden_ref \
  "toolkit Python module command targets in shipped Apple path" \
  'examples\.train_adapter|export\.export_fmadapter'

log "Checking authoritative Apple backend ids"
cargo test -p autopilot-desktop apple_operator_authoritative_backends_are_rust_only

log "Checking docs describe the Rust-only boundary"
if rg -n 'toolkit-backed training/export lane' docs/headless-compute.md >/dev/null 2>&1; then
  die "docs/headless-compute.md still describes the shipped Apple lane as toolkit-backed"
fi
rg -n 'Rust-native Psionic' docs/headless-compute.md >/dev/null 2>&1 \
  || die "docs/headless-compute.md must describe the shipped Apple lane as Rust-native"
rg -n 'check-psionic-apple-rust-only-gate\.sh' \
  docs/headless-compute.md \
  "$PSIONIC_TRAIN_SYSTEM_DOC" \
  >/dev/null 2>&1 \
  || die "Rust-only gate script must be documented in the canonical Apple training docs"

log "Psionic Apple Rust-only gate passed."
