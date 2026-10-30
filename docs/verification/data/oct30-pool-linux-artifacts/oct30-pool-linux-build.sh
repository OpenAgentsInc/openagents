#!/usr/bin/env bash
set -euo pipefail
cd /tmp/oct30-pool-linux.LbbBa1
mkdir -p sources
 tar -xzf sources.tar.gz -C sources
cd sources
. ./tls-env.sh
printf 'SYSTEM '; uname -srmo
printf 'GCC '; /run/current-system/sw/bin/gcc -dumpfullversion
printf 'OPENSSL_DEV %s\nOPENSSL_LIB %s\n' "$OPENSSL_DEV" "$OPENSSL_LIB"
sha256sum *.c *.h > ../inputs.sha256
for mode in plain sanitized; do
 flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable)
 if [[ $mode == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
 /run/current-system/sw/bin/gcc "${flags[@]}" -Dmain=coax_mesh_pool_main -c mesh-pool.c -o "../$mode.o"
 /run/current-system/sw/bin/gcc "${flags[@]}" $CXTLS_CFLAGS -I. -Dcoax_fleet_main=coax_mesh_pool_main fleet-host.c mesh-public-input.c mesh-public-output.c socket-native.c tls-native.c entropy-native.c entropy-foreign.c process-native.c "../$mode.o" $CXTLS_LIBS -lm -lpthread -o "../pool-$mode"
 sha256sum "../pool-$mode"
done
