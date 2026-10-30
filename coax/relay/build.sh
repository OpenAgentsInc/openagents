#!/usr/bin/env bash
# Builds the Coax relay (R1) to OUT (default coax/relay/bin/relay).
#   coax/relay/build.sh [OUT] [--sanitize]     ASan+UBSan with --sanitize
# Links tls-native.c for --tls-cert/--tls-key (OpenSSL from Nix on Linux,
# through coax/platform/tls-env.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
out="${1:-$here/bin/relay}"
. "$root/coax/platform/tls-env.sh"
flags=(-std=c11 -O1 -g)
if [[ "${2:-}" == "--sanitize" ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
mkdir -p "$(dirname "$out")"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-relay-build.XXXXXX")"
trap 'rm -rf "$work"' EXIT
"${COAX:-$root/coax/bin/coax}" c "$here/relay.coax" > "$work/relay.c"
cc "${flags[@]}" -Dmain=coax_relay_main -c "$work/relay.c" -o "$work/relay.o"
# shellcheck disable=SC2086
cc "${flags[@]}" -Wall -Wextra -Werror $CXTLS_CFLAGS -I"$root/coax/platform" "$here/relay-host.c" "$root/coax/platform/socket-server-native.c" \
  "$root/coax/platform/store-file-native.c" "$root/coax/platform/tls-native.c" "$root/coax/platform/socket-native.c" \
  "$work/relay.o" $CXTLS_LIBS -lm -lpthread -o "$out"
echo "built $out"
