#!/usr/bin/env bash
# Firm reason: 5 (thin C/GTK/OpenSSL SDK linker; generated Coax units hold all rules).
set -euo pipefail
cd "$(dirname "$0")/../../.."
repo="$PWD";work="$repo/apps/openagents-desktop/build/native-c";out="$repo/apps/openagents-desktop/build/NativeLinux";p="$repo/coax/platform"
mkdir -p "$out"
cc -std=c11 -O2 -D_DEFAULT_SOURCE -I"$p" -Icoder/build/environment -o "$out/openagents-environment" \
  coder/build/environment/host.c coder/build/environment/entropy.c "$p"/tool-native.c "$p"/process-native.c "$p"/store-file-native.c "$p"/read-file-native.c "$p"/socket-native.c "$p"/entropy-native.c "$p"/entropy-foreign.c "$p"/agent-signer-none.c "$p"/tls-native.c \
  $(pkg-config --cflags --libs openssl) -lpthread -lm
for unit in program services; do cc -std=c11 -O2 -w -c "$work/$unit.c" -o "$work/$unit.o"; done
nm --defined-only "$work/program.o" | awk '$2=="T" && $3 ~ /^nd_/ {print $3}' > "$work/program.exports"
nm --defined-only "$work/services.o" | awk '$2=="T" && $3 ~ /^ls_/ {print $3}' > "$work/services.exports"
objcopy --keep-global-symbols="$work/program.exports" "$work/program.o"
objcopy --keep-global-symbols="$work/services.exports" "$work/services.o"
cc -std=c11 -O2 -D_DEFAULT_SOURCE -I"$p" -Icoder/build/environment -o "$out/OpenAgents" \
  apps/openagents-desktop/native/linux.c "$work/program.o" "$work/services.o" coder/build/environment/entropy.c \
  "$p"/process-native.c "$p"/store-file-native.c "$p"/socket-native.c "$p"/entropy-native.c "$p"/entropy-foreign.c "$p"/tls-native.c \
  $(pkg-config --cflags --libs gtk4 openssl) -lpthread -lm
"$out/OpenAgents" --inspect-program
echo "built $out/OpenAgents"
