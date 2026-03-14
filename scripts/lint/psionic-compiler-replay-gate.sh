#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

log() {
  echo "[psionic-compiler-replay-gate] $*"
}

log "cargo test -p psionic-compiler --test process_replay -- --nocapture"
cargo test -p psionic-compiler --test process_replay -- --nocapture

log "Psionic compiler replay gate passed"
