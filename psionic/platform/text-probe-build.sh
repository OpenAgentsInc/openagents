#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ $# == 2 && ( $2 == plain || $2 == sanitized ) ]] || { echo 'usage: text-probe-build.sh OUT plain|sanitized' >&2; exit 2; }
out=$1;coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
. coax/platform/tls-env.sh
mkdir -p "$out"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
if [[ $2 == sanitized ]];then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
"$coax" c psionic/lib/pylon/text-probe.coax --library --run-steps 250000000 --header "$out/text-probe.h" > "$out/text-probe.c"
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" > /dev/null
# Generated C and these native effects also form the portable Linux build context.
# shellcheck disable=SC2086
"$cc" "${flags[@]}" $CXTLS_CFLAGS -I"$out" -Icoax/platform "$out/text-probe.c" psionic/platform/text-probe-entry.c psionic/platform/pylon-host.c coax/platform/socket-native.c coax/platform/tls-native.c coax/platform/entropy-native.c coax/platform/entropy-foreign.c $CXTLS_LIBS -lm -lpthread -o "$out/pylon-text-probe"
