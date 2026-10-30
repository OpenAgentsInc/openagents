#!/usr/bin/env bash
# Build the chapter-19 Pylon: OUT plain|sanitized.
# Coax emits one C program from psionic/lib/pylon/run.coax; the host
# (pylon-host.c) runs its main on a big-stack thread and links the platform
# effects: sockets, entropy and the signing double's process.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 2 || ( $2 != plain && $2 != sanitized ) ]]; then echo "usage: psionic/platform/pylon-build.sh OUT plain|sanitized" >&2; exit 2; fi
out=$1 native=$2
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
if [[ $native == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
# event.coax and the chapter 16 request tables together pass the default
# compile-time work bound; the bound is raised, not the program changed.
"$coax" c psionic/lib/pylon/run.coax --run-steps 20000000 > "$out/pylon.c"
# entropy-native.c includes the policy's header; the policy itself is in the program.
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" > /dev/null
"$cc" "${flags[@]}" -Dmain=coax_pylon_main -c "$out/pylon.c" -o "$out/pylon.o"
"$cc" "${flags[@]}" -I"$out" -Icoax/platform psionic/platform/pylon-host.c coax/platform/socket-native.c coax/platform/entropy-native.c \
    coax/platform/entropy-foreign.c coax/platform/process-native.c "$out/pylon.o" -lm -lpthread -o "$out/pylon"
echo "built $out/pylon"
