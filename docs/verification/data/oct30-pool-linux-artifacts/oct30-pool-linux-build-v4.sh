#!/usr/bin/env bash
set -euo pipefail
cd /tmp/oct30-pool-linux.LbbBa1/sources
. ./tls-env.sh
sha256sum mesh-pool-v4.c
for mode in plain sanitized; do
 flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
 if [[ $mode == sanitized ]];then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
 /run/current-system/sw/bin/gcc "${flags[@]}" -Dmain=coax_mesh_pool_main -c mesh-pool-v4.c -o "../v4-$mode.o"
 /run/current-system/sw/bin/gcc "${flags[@]}" $CXTLS_CFLAGS -I. -Dcoax_fleet_main=coax_mesh_pool_main fleet-host.c mesh-public-input.c mesh-public-output.c socket-native.c tls-native.c entropy-native.c entropy-foreign.c process-native.c "../v4-$mode.o" $CXTLS_LIBS -lm -lpthread -o "../pool-v4-$mode"
 sha256sum "../pool-v4-$mode"
done
