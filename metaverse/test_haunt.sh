#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-haunt.XXXXXX")
python3 metaverse/tools/check_haunt_doors.py
coax/bin/coax test metaverse/haunt.coax
coax/bin/coax test metaverse/hud.coax
coax/bin/coax test metaverse/haunt-light.coax
coax/bin/coax mutate metaverse/haunt-buffer.coax
coax/bin/coax c metaverse/haunt.coax --library --header "$work/Haunt.h" > "$work/Haunt.c"
coax/bin/coax c metaverse/haunt-buffer.coax --library --header "$work/HauntBuffer.h" > "$work/HauntBuffer.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -Wno-unused-value -I "$work" "$work/Haunt.c" "$work/HauntBuffer.c" metaverse/haunt_smoke.c -lm -o "$work/test"
"$work/test" apps/openagents-ios/OpenAgents/Resources
if [[ -n "${PROTOC:-}" && -n "${PROTO_PYTHON:-}" ]]; then
  "$PROTO_PYTHON" metaverse/haunt_oracle.py "$work/test"
fi
