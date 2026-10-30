#!/usr/bin/env bash
# Live Microcoder smoke on Vertex gemini-3.8-flash (falling back to the
# Gemini API and OpenRouter): "what is 2+2" in one step, and "how many Swift
# files are in this folder" in one command on a scratch folder of three. Credentials as in
# coax/lib/gateway/live.sh; skipped when there is none. Never prints a key.
#
#   coax/lib/microcoder/live.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"; coax="${COAX:-$repo/coax/bin/coax}"
secrets="$HOME/work/.secrets"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
if [ -z "${VERTEX_ACCESS_TOKEN:-}" ] && [ -z "${GOOGLE_APPLICATION_CREDENTIALS:-}" ] && [ -f "$secrets/vertex-sa-inference.json" ]; then
  export GOOGLE_APPLICATION_CREDENTIALS="$secrets/vertex-sa-inference.json"
fi
[ -z "${GEMINI_API_KEY:-}" ] && [ -f "$secrets/openagents-gemini-provider.env" ] && { set -a; . "$secrets/openagents-gemini-provider.env"; set +a; }
[ -z "${OPENROUTER_API_KEY:-}" ] && [ -f "$secrets/openrouter.env" ] && { set -a; . "$secrets/openrouter.env"; set +a; }
if [ -z "${VERTEX_ACCESS_TOKEN:-}${GOOGLE_APPLICATION_CREDENTIALS:-}${GEMINI_API_KEY:-}${OPENROUTER_API_KEY:-}" ]; then
  echo "microcoder: skipped (no Vertex, Gemini or OpenRouter credential)"; exit 0
fi
mkdir -p "$work/folder"
for f in App Model View; do printf 'struct %s {}\n' "$f" > "$work/folder/$f.swift"; done
printf '# demo\n' > "$work/folder/README.md"
"$coax" c "$here/live.coax" > "$work/live.c"
"$cc" -std=c11 -O1 -g -o "$work/live" "$work/live.c" "$repo/coax/platform/process-native.c" -lm
MC_LIVE_DIR="$work/folder" CLOUDSDK_CONFIG="$work/gcloud" "$work/live" > "$work/out" 2>&1 || true
cat "$work/out"
for s in "${VERTEX_ACCESS_TOKEN:-}" "${GEMINI_API_KEY:-}" "${OPENROUTER_API_KEY:-}"; do
  [ -n "$s" ] && grep -qF -- "$s" "$work/out" && { echo "microcoder: a secret appeared in the output"; exit 1; }
done
tail -1 "$work/out" | grep -q '^smoke: ok$'
