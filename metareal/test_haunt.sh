#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/metareal-haunt.XXXXXX")
coax/bin/coax test metareal/haunt.coax
coax/bin/coax test metareal/hud.coax
coax/bin/coax test metareal/haunt-light.coax
coax/bin/coax mutate metareal/haunt-buffer.coax
coax/bin/coax c metareal/haunt.coax --library --header "$work/Haunt.h" > "$work/Haunt.c"
coax/bin/coax c metareal/haunt-buffer.coax --library --header "$work/HauntBuffer.h" > "$work/HauntBuffer.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -Wno-unused-value -I "$work" "$work/Haunt.c" "$work/HauntBuffer.c" metareal/haunt_smoke.c -lm -o "$work/test"
"$work/test" apps/openagents-ios/OpenAgents/Resources
if [[ -n "${PROTOC:-}" && -n "${PROTO_PYTHON:-}" ]]; then
  "$PROTO_PYTHON" metareal/haunt_oracle.py "$work/test"
fi
