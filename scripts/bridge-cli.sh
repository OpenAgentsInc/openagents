#!/usr/bin/env bash
set -euo pipefail

# Lightweight wrapper to launch the oa-bridge binary if present,
# falling back to cargo. Mirrors tricoder's behavior in a shell-friendly way.

BIN_OVERRIDE="${OPENAGENTS_BRIDGE_BIN:-${OABRIDGE_BIN:-}}"

pick_cached_bin() {
  local exe
  case "$(uname -s 2>/dev/null)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) exe="oa-bridge.exe" ;;
    *) exe="oa-bridge" ;;
  esac
  local base=""
  if [[ "$(uname -s 2>/dev/null)" == "Darwin" ]]; then
    base="$HOME/Library/Caches/openagents"
  elif [[ "$(uname -s 2>/dev/null)" == "Windows_NT" ]]; then
    base="${LOCALAPPDATA:-$HOME/AppData/Local}/OpenAgents"
  else
    base="${XDG_CACHE_HOME:-$HOME/.cache}/openagents"
  fi
  local dir="$base/binaries"
  [[ -d "$dir" ]] || return 1
  local latest="$(ls -1 "$dir" 2>/dev/null | sort -r | head -n1 || true)"
  [[ -n "$latest" ]] || return 1
  local bin="$dir/$latest/$exe"
  [[ -x "$bin" ]] && { echo "$bin"; return 0; }
  return 1
}

pick_home_bin() {
  local exe
  case "$(uname -s 2>/dev/null)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) exe="oa-bridge.exe" ;;
    *) exe="oa-bridge" ;;
  esac
  local bin="$HOME/.openagents/bin/$exe"
  [[ -x "$bin" ]] && { echo "$bin"; return 0; }
  return 1
}

run_cargo() {
  local bind_addr="${OPENAGENTS_BIND:-0.0.0.0:8787}"
  echo "[bridge-cli] falling back to cargo run (bind $bind_addr)" >&2
  exec cargo run -p oa-bridge -- --bind "$bind_addr" "$@"
}

main() {
  local bin=""
  if [[ -n "${BIN_OVERRIDE}" && -x "${BIN_OVERRIDE}" ]]; then
    bin="$BIN_OVERRIDE"
  else
    bin="$(pick_home_bin || true)"
    if [[ -z "$bin" ]]; then
      bin="$(pick_cached_bin || true)"
    fi
  fi
  if [[ -n "$bin" ]]; then
    exec "$bin" "$@"
  else
    run_cargo "$@"
  fi
}

main "$@"

