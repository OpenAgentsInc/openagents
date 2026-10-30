#!/usr/bin/env bash
# Chapter21 native free mesh adapters. Coax owns protocol, trust and policy.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 2 || $# -gt 3 || ( $2 != plain && $2 != sanitized ) ]]; then
  echo 'usage: psionic/platform/mesh-build.sh OUT plain|sanitized [all|fleet|check|pool]' >&2; exit 2
fi
out=$1 native=$2 target=${3:-all}
case $target in all|fleet|check|pool) ;; *) echo 'invalid mesh target' >&2; exit 2;; esac
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
host=${FLEET_HOST:-psionic/platform/fleet-host.c}
. coax/platform/tls-env.sh
mkdir -p "$out"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
if [[ $native == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
if [[ $target == all || $target == fleet ]]; then
  COAX="$coax" COAX_CC="$cc" FLEET_HOST="$host" psionic/platform/fleet-build.sh "$out" "$native"
fi
if [[ $target == fleet ]]; then exit 0; fi
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" >/dev/null
for role in check pool; do
  if [[ $target != all && $target != "$role" ]]; then continue; fi
  case $role in
    check) entry=psionic/lib/mesh/checker-main.coax; effect=psionic/platform/mesh-input.c;;
    pool) entry=psionic/lib/mesh/pool-main.coax; effect=psionic/platform/mesh-public-input.c;;
  esac
  symbol=coax_mesh_${role}_main
  "$coax" c "$entry" --run-steps 250000000 > "$out/mesh-$role.c"
  "$cc" "${flags[@]}" -Dmain="$symbol" -c "$out/mesh-$role.c" -o "$out/mesh-$role.o"
  # shellcheck disable=SC2086
  "$cc" "${flags[@]}" $CXTLS_CFLAGS -I"$out" -Icoax/platform -Dcoax_fleet_main="$symbol" \
    "$host" "$effect" coax/platform/socket-native.c coax/platform/tls-native.c \
    coax/platform/entropy-native.c coax/platform/entropy-foreign.c coax/platform/process-native.c \
    "$out/mesh-$role.o" $CXTLS_LIBS -lm -lpthread -o "$out/psionic-mesh-$role"
  echo "built $out/psionic-mesh-$role"
done
