#!/usr/bin/env bash
# Build the Coax compiler (coax0) to coax/bin/coax: one unity translation
# unit, no dependencies beyond a C++20 compiler.
#   coax/build.sh          build
#   coax/build.sh test     build, then run tests/run.sh
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p bin
"${CXX:-c++}" -std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers -o bin/coax src/main.cpp
if [[ "${1:-}" == "test" ]]; then
  exec tests/run.sh
fi
