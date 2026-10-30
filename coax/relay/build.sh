#!/usr/bin/env bash
# Builds the Coax relay (R1) to OUT (default coax/relay/bin/relay).
#   coax/relay/build.sh [OUT] [--sanitize]     ASan+UBSan with --sanitize
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
out="${1:-$here/bin/relay}"
flags=(-std=c11 -O1 -g)
if [[ "${2:-}" == "--sanitize" ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
mkdir -p "$(dirname "$out")"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-relay-build.XXXXXX")"
trap 'rm -rf "$work"' EXIT
"${COAX:-$root/coax/bin/coax}" c "$here/relay.coax" > "$work/relay.c"
cc "${flags[@]}" -Dmain=coax_relay_main -c "$work/relay.c" -o "$work/relay.o"
cc "${flags[@]}" -I"$root/coax/platform" "$here/relay-host.c" "$root/coax/platform/socket-server-native.c" "$root/coax/platform/store-file-native.c" "$work/relay.o" -lm -lpthread -o "$out"
echo "built $out"
