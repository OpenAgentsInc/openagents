#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-haunt.XXXXXX")
python3 verse/tools/check_haunt_doors.py
coax/bin/coax test verse/haunt.coax
coax/bin/coax test verse/hud.coax
coax/bin/coax test verse/haunt-light.coax
coax/bin/coax mutate verse/haunt-buffer.coax
coax/bin/coax c verse/haunt.coax --library --header "$work/Haunt.h" > "$work/Haunt.c"
coax/bin/coax c verse/haunt-buffer.coax --library --header "$work/HauntBuffer.h" > "$work/HauntBuffer.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -Wno-unused-value -I "$work" "$work/Haunt.c" "$work/HauntBuffer.c" verse/haunt_smoke.c -lm -o "$work/test"
"$work/test" apps/openagents-ios/OpenAgents/Resources
if [[ -n "${PROTOC:-}" && -n "${PROTO_PYTHON:-}" ]]; then
  "$PROTO_PYTHON" verse/haunt_oracle.py "$work/test"
fi
