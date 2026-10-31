#!/usr/bin/env bash
# Build coax0 to coax/bin/coax: C++20 evaluator/emitter plus Coax lexer, parser, checker, source loader and importer
# compiled to C11. Host C/C++ compilers and shell tools only.
# Internal second-stage bootstrap, run on an isolated source snapshot.
set -euo pipefail
[[ $# == 1 && "$1" == --private-snapshot ]] || { echo 'internal private-snapshot bootstrap: use coax/build.sh' >&2; exit 2; }
export LC_ALL=C
cd "$(dirname "$0")"
root="$(cd .. && pwd -P)"
input_identity="$(./provenance.sh fingerprint "$root")"
mkdir -p bin
host_flags=(-std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then host_flags+=(-target "$COAX_HOST_TARGET"); fi
c_flags=(-std=c11 -O1 -g)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then c_flags+=(-target "$COAX_HOST_TARGET"); fi
# Stage0 has the language frontend but deliberately no proto implementation.
# It compiles the authoritative Coax importer, then we link that C11 object
# into the standalone final compiler. Nothing is downloaded or interpreted.
./check-proto-cap.sh lib/proto/wire.coax lib/proto/importer.coax
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
  printf ')COAX_IMPORTER";\nstatic constexpr char PROTO_FINGERPRINT_SOURCE[] = R"COAX_FP('
  cat lib/proto/fingerprint.coax
  printf ')COAX_FP";\n'
} > "$work/proto_runtime.inc"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/budget.c -o "$work/seed-budget.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/profiles.c -o "$work/seed-profiles.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/mutation-schedule.c -o "$work/seed-mutation-schedule.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/lexer.c -o "$work/seed-lexer.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/parser.c -o "$work/seed-parser.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/checker.c -o "$work/seed-checker.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/sources.c -o "$work/seed-sources.o"
"${CXX:-c++}" "${host_flags[@]}" -DCOAX_BOOTSTRAP_IMPORTER -I bootstrap -I "$work" -o "$work/stage0" src/main.cpp "$work/seed-mutation-schedule.o" "$work/seed-budget.o" "$work/seed-profiles.o" "$work/seed-lexer.o" "$work/seed-parser.o" "$work/seed-checker.o" "$work/seed-sources.o"
gate() {
  if "$@" > "$work/gate.log" 2>&1; then return 0; fi
  cat "$work/gate.log" >&2
  return 1
}
# Validate release metadata before expensive compiler gates. Stage0 uses the
# checked-in header seed; final publication must match the canonical Coax data.
gate "$work/stage0" test release-model.coax
gate "$work/stage0" mutate release-model.coax
COAX_RELEASE_COMPILER="$PWD/$work/stage0" ../scripts/coax-release.sh --check
gate "$work/stage0" test lib/compiler/budget.coax
gate "$work/stage0" mutate lib/compiler/budget.coax
"$work/stage0" c lib/compiler/budget.coax --library --header "$work/budget.h" > "$work/budget.c"
cmp bootstrap/budget.c "$work/budget.c"
cmp bootstrap/budget.h "$work/budget.h"
gate "$work/stage0" test lib/compiler/profiles.coax
gate "$work/stage0" mutate lib/compiler/profiles.coax
"$work/stage0" c lib/compiler/profiles.coax --library --header "$work/profiles.h" > "$work/profiles.c"
cmp bootstrap/profiles.c "$work/profiles.c"
cmp bootstrap/profiles.h "$work/profiles.h"
gate "$work/stage0" test lib/compiler/provenance.coax
gate "$work/stage0" mutate lib/compiler/provenance.coax
"$work/stage0" c lib/compiler/provenance.coax --library --header "$work/provenance.h" > "$work/provenance.c"
gate "$work/stage0" test lib/compiler/mutation-schedule.coax
gate "$work/stage0" mutate lib/compiler/mutation-schedule.coax
"$work/stage0" c lib/compiler/mutation-schedule.coax --library --header "$work/mutation-schedule.h" > "$work/mutation-schedule.c"
cmp bootstrap/mutation-schedule.c "$work/mutation-schedule.c"
cmp bootstrap/mutation-schedule.h "$work/mutation-schedule.h"
gate "$work/stage0" test lib/compiler/compute.coax
gate "$work/stage0" mutate lib/compiler/compute.coax
gate "$work/stage0" test lib/compiler/spirv.coax
gate "$work/stage0" mutate lib/compiler/spirv.coax
gate "$work/stage0" test lib/compiler/lexer.coax
gate "$work/stage0" mutate lib/compiler/lexer.coax
"$work/stage0" c lib/compiler/lexer.coax --library --header "$work/lexer.h" > "$work/lexer.c"
if ! cmp bootstrap/lexer.c "$work/lexer.c" || ! cmp bootstrap/lexer.h "$work/lexer.h"; then
  echo "Lexer seed is stale. Regenerate it with coax/refresh-lexer-seed.sh using a trusted compiler." >&2
  exit 1
fi
gate "$work/stage0" test lib/compiler/sources.coax
gate "$work/stage0" mutate lib/compiler/sources.coax
"$work/stage0" c lib/compiler/sources.coax --library --header "$work/sources.h" > "$work/sources.c"
if ! cmp bootstrap/sources.c "$work/sources.c" || ! cmp bootstrap/sources.h "$work/sources.h"; then
  echo "Source loader seed is stale. Regenerate it with coax/refresh-sources-seed.sh using a trusted compiler." >&2
  exit 1
fi
gate "$work/stage0" test lib/compiler/parser.coax
gate "$work/stage0" mutate lib/compiler/parser.coax
"$work/stage0" c lib/compiler/parser.coax --library --header "$work/parser.h" > "$work/parser.c"
if ! cmp bootstrap/parser.c "$work/parser.c" || ! cmp bootstrap/parser.h "$work/parser.h"; then
  echo "Parser seed is stale. Regenerate it with coax/refresh-parser-seed.sh using a trusted compiler." >&2
  exit 1
fi
gate "$work/stage0" test lib/compiler/checker.coax
gate "$work/stage0" mutate lib/compiler/checker.coax
"$work/stage0" c lib/compiler/checker.coax --library --header "$work/checker.h" > "$work/checker.c"
if ! cmp bootstrap/checker.c "$work/checker.c" || ! cmp bootstrap/checker.h "$work/checker.h"; then
  echo "Checker seed is stale. Regenerate it with coax/refresh-checker-seed.sh using a trusted compiler." >&2
  exit 1
fi
gate "$work/stage0" test lib/proto/wire.coax
gate "$work/stage0" mutate lib/proto/wire.coax
gate "$work/stage0" test lib/proto/fingerprint.coax
gate "$work/stage0" mutate lib/proto/fingerprint.coax
gate "$work/stage0" test lib/proto/importer.coax
gate "$work/stage0" mutate lib/proto/importer.coax
"$work/stage0" c lib/compiler/compiler.coax --library --header "$work/proto_importer.h" > "$work/proto_importer.c"
"${CC:-cc}" "${c_flags[@]}" -c "$work/proto_importer.c" -o "$work/proto_importer.o"
"${CXX:-c++}" "${host_flags[@]}" -I "$work" -o "$work/coax" src/main.cpp "$work/proto_importer.o"
"$work/coax" c lib/compiler/compiler.coax --library > "$work/rebuilt.c"
cmp "$work/proto_importer.c" "$work/rebuilt.c"
gate "$work/coax" test lib/compiler/budget.coax
"$work/coax" c lib/compiler/budget.coax --library --header "$work/final-budget.h" > "$work/final-budget.c"
cmp bootstrap/budget.c "$work/final-budget.c"
cmp bootstrap/budget.h "$work/final-budget.h"
gate "$work/coax" test lib/compiler/profiles.coax
"$work/coax" c lib/compiler/profiles.coax --library --header "$work/final-profiles.h" > "$work/final-profiles.c"
cmp bootstrap/profiles.c "$work/final-profiles.c"
cmp bootstrap/profiles.h "$work/final-profiles.h"
gate "$work/coax" test lib/compiler/provenance.coax
"$work/coax" c lib/compiler/provenance.coax --library --header "$work/final-provenance.h" > "$work/final-provenance.c"
cmp "$work/provenance.c" "$work/final-provenance.c"
cmp "$work/provenance.h" "$work/final-provenance.h"
"${CC:-cc}" "${c_flags[@]}" -I "$work" "$work/provenance.c" provenance-bridge.c -o "$work/provenance-check"
gate "$work/coax" test lib/compiler/mutation-schedule.coax
"$work/coax" c lib/compiler/mutation-schedule.coax --library --header "$work/final-mutation-schedule.h" > "$work/final-mutation-schedule.c"
cmp bootstrap/mutation-schedule.c "$work/final-mutation-schedule.c"
cmp bootstrap/mutation-schedule.h "$work/final-mutation-schedule.h"
gate "$work/coax" test lib/compiler/lexer.coax
gate "$work/coax" test lib/compiler/parser.coax
gate "$work/coax" test lib/compiler/checker.coax
gate "$work/coax" test lib/compiler/sources.coax
"$work/coax" c lib/compiler/lexer.coax --library --header "$work/final-lexer.h" > "$work/final-lexer.c"
cmp bootstrap/lexer.c "$work/final-lexer.c"
"$work/coax" c lib/compiler/parser.coax --library --header "$work/final-parser.h" > "$work/final-parser.c"
cmp bootstrap/parser.c "$work/final-parser.c"
"$work/coax" c lib/compiler/checker.coax --library --header "$work/final-checker.h" > "$work/final-checker.c"
cmp bootstrap/checker.c "$work/final-checker.c"
cmp bootstrap/checker.h "$work/final-checker.h"
"$work/coax" c lib/compiler/sources.coax --library --header "$work/final-sources.h" > "$work/final-sources.c"
cmp bootstrap/sources.c "$work/final-sources.c"
cmp bootstrap/sources.h "$work/final-sources.h"
COAX_RELEASE_COMPILER="$PWD/$work/coax" ../scripts/coax-release.sh --check --binary "$PWD/$work/coax"
# Publish only a fully checked build. C/header remain available for native tests.
cp "$work/lexer.c" bin/lexer.c
cp "$work/profiles.c" bin/profiles.c
cp "$work/profiles.h" bin/profiles.h
cp "$work/lexer.h" bin/lexer.h
cp "$work/parser.c" bin/parser.c
cp "$work/parser.h" bin/parser.h
cp "$work/checker.c" bin/checker.c
cp "$work/checker.h" bin/checker.h
cp "$work/sources.c" bin/sources.c
cp "$work/sources.h" bin/sources.h
cp "$work/proto_runtime.inc" bin/proto_runtime.inc
cp "$work/proto_importer.c" bin/proto_importer.c
cp "$work/proto_importer.h" bin/proto_importer.h
# A record is published only after both bootstrap stages and all mandatory
# compiler gates pass, and only if the exact inputs stayed unchanged.
[[ "$(./provenance.sh fingerprint "$root")" == "$input_identity" ]] || { echo 'compiler inputs changed during bootstrap' >&2; exit 1; }
./provenance.sh write "$root" "$work/coax" "$work/provenance-check" "$work/provenance.record" "$input_identity"
./provenance.sh verify "$root" "$work/coax" "$work/provenance-check" "$work/provenance.record"
mv "$work/coax" bin/coax
mv "$work/provenance-check" bin/provenance-check
mv "$work/provenance.record" bin/provenance.record
rm -rf "$work"
trap - EXIT
