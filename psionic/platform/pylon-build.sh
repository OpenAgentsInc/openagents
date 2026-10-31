#!/usr/bin/env bash
# Build the chapter-24 Pylon: OUT plain|sanitized [--test-trust].
# Coax emits one C program from psionic/lib/pylon/run.coax; the host
# (pylon-host.c) runs its main on a big-stack thread and links the platform
# effects: sockets, TLS (tls-native.c with OpenSSL from Nix through
# coax/platform/tls-env.sh, linked even for ws://) and entropy. Signing is
# Coax (event_sign.coax); no signer process.
# --test-trust builds with -DCXTLS_TEST_TRUST: wss:// then trusts exactly the
# CA file named by $CXTLS_TEST_CA when it is set (tests generate a throwaway
# one). Test builds only; without it the system trust store applies.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 2 || $# -gt 3 || ( $2 != plain && $2 != sanitized ) || ( $# == 3 && $3 != --test-trust ) ]]; then
    echo "usage: psionic/platform/pylon-build.sh OUT plain|sanitized [--test-trust]" >&2; exit 2
fi
out=$1 native=$2
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
. coax/platform/tls-env.sh
mkdir -p "$out"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
if [[ $native == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
trust=()
if [[ ${3:-} == --test-trust ]]; then trust=(-DCXTLS_TEST_TRUST); fi
# event_sign.coax (sf_carry's precomputed tables), event.coax and nip44-key.coax
# pass the default compile-time work bound (as coax/tests/nostr/schnorr_sign.py); the bound is raised,
# not the program changed.
"$coax" c psionic/lib/pylon/run.coax --run-steps 250000000 > "$out/pylon.c"
# entropy-native.c includes the policy's header; the policy itself is in the program.
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" > /dev/null
"$cc" "${flags[@]}" -Dmain=coax_pylon_main -c "$out/pylon.c" -o "$out/pylon.o"
# shellcheck disable=SC2086
"$cc" "${flags[@]}" "${trust[@]}" $CXTLS_CFLAGS -I"$out" -Icoax/platform psionic/platform/pylon-host.c coax/platform/socket-native.c \
    coax/platform/tls-native.c coax/platform/entropy-native.c coax/platform/entropy-foreign.c \
    "$out/pylon.o" $CXTLS_LIBS -lm -lpthread -o "$out/pylon"
echo "built $out/pylon"
