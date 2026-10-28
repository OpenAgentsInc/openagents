#!/usr/bin/env bash
# Build coax0 to coax/bin/coax: C++20 unity frontend plus a Coax importer
# compiled to C11. Host C/C++ compilers and shell tools only.
#   coax/build.sh          build
#   coax/build.sh test     build, then run tests/run.sh
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p bin
host_flags=(-std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then host_flags+=(-target "$COAX_HOST_TARGET"); fi
# Stage0 has the language frontend but deliberately no proto implementation.
# It compiles the authoritative Coax importer, then we link that C11 object
# into the standalone final compiler. Nothing is downloaded or interpreted.
work="bin/.bootstrap-$$"
mkdir -p "$work"
trap 'rm -rf "$work"' EXIT
{
  printf 'static constexpr char PROTO_CORE_SOURCE[] = R"COAX_CORE('
  cat lib/proto/wire.coax
  printf ')COAX_CORE";\nstatic constexpr char PROTO_MEMORY_SOURCE[] = R"COAX_MEMORY('
  cat lib/proto/memory.coax
  printf ')COAX_MEMORY";\nstatic constexpr char PROTO_IMPORTER_SOURCE[] = R"COAX_IMPORTER('
  cat lib/proto/importer.coax
  printf ')COAX_IMPORTER";\n'
} > "$work/proto_runtime.inc"
"${CXX:-c++}" "${host_flags[@]}" -DCOAX_BOOTSTRAP_IMPORTER -I "$work" -o "$work/stage0" src/main.cpp
gate() {
  if "$@" > "$work/gate.log" 2>&1; then return 0; fi
  cat "$work/gate.log" >&2
  return 1
}
gate "$work/stage0" test lib/proto/wire.coax
gate "$work/stage0" mutate lib/proto/wire.coax
gate "$work/stage0" test lib/proto/importer.coax
gate "$work/stage0" mutate lib/proto/importer.coax
"$work/stage0" c lib/proto/importer.coax --library --header "$work/proto_importer.h" > "$work/proto_importer.c"
c_flags=(-std=c11 -O1 -g)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then c_flags+=(-target "$COAX_HOST_TARGET"); fi
"${CC:-cc}" "${c_flags[@]}" -c "$work/proto_importer.c" -o "$work/proto_importer.o"
"${CXX:-c++}" "${host_flags[@]}" -I "$work" -o "$work/coax" src/main.cpp "$work/proto_importer.o"
"$work/coax" c lib/proto/importer.coax --library > "$work/rebuilt.c"
cmp "$work/proto_importer.c" "$work/rebuilt.c"
# Publish only a fully checked build. C/header remain available for native tests.
cp "$work/proto_runtime.inc" bin/proto_runtime.inc
cp "$work/proto_importer.c" bin/proto_importer.c
cp "$work/proto_importer.h" bin/proto_importer.h
mv "$work/coax" bin/coax
rm -rf "$work"
trap - EXIT
if [[ "${1:-}" == "test" ]]; then
  exec tests/run.sh
fi
