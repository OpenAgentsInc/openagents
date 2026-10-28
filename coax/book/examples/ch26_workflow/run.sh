#!/usr/bin/env bash
# Real local file effect, generated protobuf, and the emitted request machine.
# No network service, simulator or Python dependency.
set -euo pipefail
cd "$(dirname "$0")"
root=$(cd ../../../.. && pwd)
compiler="$root/coax/bin/coax"
out=$(mktemp -d)
trap 'find "$out" -depth -delete' EXIT
"$compiler" test workflow.coax > "$out/checks.log"
"$compiler" c workflow.coax --library --header "$out/workflow.h" > "$out/workflow.c"
if [[ ${BOOK_MUTATE:-0} == 1 ]]; then
  "$compiler" mutate ../ch23_requests.coax
fi
for mode in debug release; do
  flags=(-O0)
  if [[ $mode == release ]]; then flags=(-O2); fi
  if [[ ${BOOK_SANITIZE:-0} == 1 ]]; then flags+=(-g -fno-omit-frame-pointer -fsanitize=address,undefined); fi
  "${COAX_CC:-cc}" -std=c11 -Wall -Wextra -Wno-unused-parameter "${flags[@]}" -I "$out" "$out/workflow.c" host.c -lm -o "$out/demo"
  "$out/demo" "$out/preview.pb"
done
if [[ -n ${PROTOC:-} ]]; then
  "$PROTOC" --descriptor_set_in="$root/proto/descriptors/ios-home.pb" \
    --decode=openagents.engine.v1.HudInvocation < "$out/preview.pb" > "$out/decoded.txt"
  diff -u expected-protoc.txt "$out/decoded.txt"
  "$PROTOC" --descriptor_set_in="$root/proto/descriptors/ios-home.pb" \
    --encode=openagents.engine.v1.HudInvocation < expected-protoc.txt > "$out/oracle.pb"
  cmp "$out/preview.pb" "$out/oracle.pb"
  echo "protoc: both directions agree"
fi
