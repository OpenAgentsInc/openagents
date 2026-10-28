#!/usr/bin/env bash
# Build the Coax compiler (coax0) to coax/bin/coax: one unity translation
# unit, no dependencies beyond a C++20 compiler.
#   coax/build.sh          build
#   coax/build.sh test     build, then run tests/run.sh
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p bin
host_flags=(-std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then host_flags+=(-target "$COAX_HOST_TARGET"); fi
# Publish atomically: another build may still be using the last good compiler.
output="bin/.coax-build-$$"
runtime="bin/.proto-runtime-$$"
trap 'rm -f "$output" "$runtime"' EXIT
# Embed the authoritative Coax sources verbatim; no generator/interpreter needed.
{
  printf 'static constexpr char PROTO_CORE_SOURCE[] = R"COAX_CORE('
  cat lib/proto/wire.coax
  printf ')COAX_CORE";\nstatic constexpr char PROTO_MEMORY_SOURCE[] = R"COAX_MEMORY('
  cat lib/proto/memory.coax
  printf ')COAX_MEMORY";\n'
} > "$runtime"
mv "$runtime" bin/proto_runtime.inc
"${CXX:-c++}" "${host_flags[@]}" -I bin -o "$output" src/main.cpp
# Do not publish a compiler embedding an unchecked wire-policy core.
"$output" test lib/proto/wire.coax > /dev/null
"$output" mutate lib/proto/wire.coax > /dev/null
mv "$output" bin/coax
trap - EXIT
if [[ "${1:-}" == "test" ]]; then
  exec tests/run.sh
fi
