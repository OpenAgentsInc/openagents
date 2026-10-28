#!/usr/bin/env bash
# Builds and starts the voice host on 127.0.0.1:8787.
#   metareal/voice-host/run.sh             Google speech + Gemini agent
#   metareal/voice-host/run.sh --offline   local speech + echo agent (no network)
# Any other arguments go to voice-host (e.g. --listen 127.0.0.1:9000, --verbose).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
"$here/build.sh" >&2
args=(--provider google --agent gemini)
rest=()
for a in "$@"; do
  if [[ "$a" == "--offline" ]]; then args=(--provider local --agent echo); else rest+=("$a"); fi
done
exec "$here/build/voice-host" "${args[@]}" ${rest[@]+"${rest[@]}"}
