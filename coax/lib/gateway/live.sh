#!/usr/bin/env bash
# Live smokes against real providers, one per gateway, each skipped when its
# credential is absent. Keys come from the environment or ~/work/.secrets/
# and are never printed; every run's output is checked for them.
#
#   coax/lib/gateway/live.sh            every gateway
#   coax/lib/gateway/live.sh vertex     one
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"; coax="${COAX:-$repo/coax/bin/coax}"
secrets="$HOME/work/.secrets"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$here/live.coax" > "$work/live.c"
"$cc" -std=c11 -O1 -g -o "$work/live" "$work/live.c" "$repo/coax/platform/process-native.c" -lm
want="${1:-all}"; failed=0
run() {   # name, then the secrets to scan the output for
  local name="$1"; shift
  GW_LIVE="$name" "$work/live" > "$work/$name.out" 2>&1 || true
  cat "$work/$name.out"
  for s in "$@"; do [ -n "$s" ] && grep -qF -- "$s" "$work/$name.out" && { echo "$name: a secret appeared in the output"; failed=1; }; done
  tail -1 "$work/$name.out" | grep -q '^smoke: ok$' || failed=1
}
if [ "$want" = all ] || [ "$want" = vertex ]; then
  if [ -z "${VERTEX_ACCESS_TOKEN:-}" ] && [ -z "${GOOGLE_APPLICATION_CREDENTIALS:-}" ] && [ -f "$secrets/vertex-sa-inference.json" ]; then
    export GOOGLE_APPLICATION_CREDENTIALS="$secrets/vertex-sa-inference.json"
  fi
  if [ -n "${VERTEX_ACCESS_TOKEN:-}" ] || [ -n "${GOOGLE_APPLICATION_CREDENTIALS:-}" ]; then
    # gcloud gets a config of its own, so the user's stays untouched
    CLOUDSDK_CONFIG="$work/gcloud" run vertex "${VERTEX_ACCESS_TOKEN:-}"
  else
    echo "vertex: skipped (no VERTEX_ACCESS_TOKEN, GOOGLE_APPLICATION_CREDENTIALS or $secrets/vertex-sa-inference.json)"
  fi
fi
if [ "$want" = all ] || [ "$want" = gemini ]; then
  if [ -z "${GEMINI_API_KEY:-}" ] && [ -f "$secrets/openagents-gemini-provider.env" ]; then
    set -a; . "$secrets/openagents-gemini-provider.env"; set +a
  fi
  if [ -n "${GEMINI_API_KEY:-}" ]; then run gemini "$GEMINI_API_KEY"
  else echo "gemini: skipped (no GEMINI_API_KEY or $secrets/openagents-gemini-provider.env)"; fi
fi
[ "$failed" = 0 ] && echo "live: ok" || { echo "live: FAILED"; exit 1; }
