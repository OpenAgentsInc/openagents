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
trap 'rm -f "$output"' EXIT
"${CXX:-c++}" "${host_flags[@]}" -o "$output" src/main.cpp
mv "$output" bin/coax
trap - EXIT
if [[ "${1:-}" == "test" ]]; then
  exec tests/run.sh
fi
