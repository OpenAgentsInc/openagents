#!/usr/bin/env bash
# Chapter20 loopback HTTP-to-Pylon adapter, Coax protocol and policy.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 2 || ( $2 != plain && $2 != sanitized ) ]]; then
  echo 'usage: psionic/platform/fleet-build.sh OUT plain|sanitized' >&2; exit 2
fi
out=$1 native=$2
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
host=${FLEET_HOST:-psionic/platform/fleet-host.c}
. coax/platform/tls-env.sh
mkdir -p "$out"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
if [[ $native == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
"$coax" c psionic/lib/fleet/run.coax --run-steps 250000000 > "$out/fleet.c"
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" > /dev/null
"$cc" "${flags[@]}" -Dmain=coax_fleet_main -c "$out/fleet.c" -o "$out/fleet.o"
# shellcheck disable=SC2086
"$cc" "${flags[@]}" $CXTLS_CFLAGS -I"$out" -Icoax/platform "$host" coax/platform/socket-native.c \
  coax/platform/tls-native.c coax/platform/entropy-native.c coax/platform/entropy-foreign.c coax/platform/process-native.c \
  "$out/fleet.o" $CXTLS_LIBS -lm -lpthread -o "$out/psionic-fleet"
echo "built $out/psionic-fleet"
