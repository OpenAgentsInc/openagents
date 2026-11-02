#!/usr/bin/env bash
# Build coax0 to coax/bin/coax: C++20 evaluator/emitter plus Coax lexer, parser, checker, source loader and importer
# compiled to C11. Host C/C++ compilers and shell tools only.
# Internal second-stage bootstrap, run on an isolated source snapshot.
set -euo pipefail
replay_input=""
if [[ $# == 3 && "$1" == --private-replay-checker-cache ]]; then
  replay_input="$2"
  export COAX_CHECKER_MUTATION_SHARDS="$3"
elif [[ $# != 1 || "$1" != --private-snapshot ]]; then
  echo 'internal private-snapshot bootstrap: use coax/build.sh' >&2; exit 2
fi
export LC_ALL=C
cd "$(dirname "$0")"
root="$(cd .. && pwd -P)"
input_identity="$(./provenance.sh fingerprint "$root")"
mkdir -p bin
host_flags=(-std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers)
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then host_flags+=(-target "$COAX_HOST_TARGET"); fi
c_flags=(-std=c11 -O1 -g)
# Extra link flags for the executables (stage0, coax, provenance-check):
# published Linux builds link statically (COAX_LDFLAGS=-static), so one
# build runs on any Linux of its architecture (coax/RELEASE.md, Published builds).
read -r -a link_flags <<< "${COAX_LDFLAGS:-}"
if [[ -n "${COAX_HOST_TARGET:-}" ]]; then c_flags+=(-target "$COAX_HOST_TARGET"); fi
# Stage0 has the language frontend but deliberately no proto implementation.
# It compiles the authoritative Coax importer, then we link that C11 object
# into the standalone final compiler. Nothing is downloaded or interpreted.
./check-proto-cap.sh lib/proto/wire.coax lib/proto/importer.coax
# The emitter numbers temporaries as it goes, so two ex() calls in one C++
# expression (the operands of + are unsequenced) emit different C under Clang
# and GCC, and a seed refreshed on one fails the other (seed-portability.sh).
if grep -nE '\bex\([^;]*\bex\(' src/emit_c.cpp >&2; then
  echo 'src/emit_c.cpp: emit each operand into its own variable first, in source order (above)' >&2
  exit 1
fi
if [[ -n "$replay_input" ]]; then
  replay_input="$(cd "$replay_input" && pwd -P)"
  for artifact in stage0 proto_runtime.inc stage0.record; do
    [[ -f "$replay_input/$artifact" && ! -L "$replay_input/$artifact" ]] || { echo "coax bootstrap: missing retained $artifact" >&2;exit 1; }
  done
  work="$(mktemp -d bin/.replay-XXXXXXXX)"
  trap 'rm -rf "$work"' EXIT
  cp "$replay_input/stage0" "$work/stage0"
  cp "$replay_input/proto_runtime.inc" "$work/proto_runtime.inc"
else
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
  cat lib/crypto/sha256-kernel.coax lib/proto/fingerprint.coax
  printf ')COAX_FP";\n'
} > "$work/proto_runtime.inc"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/budget.c -o "$work/seed-budget.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/check-domains.c -o "$work/seed-check-domains.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/profiles.c -o "$work/seed-profiles.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/mutation-schedule.c -o "$work/seed-mutation-schedule.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/lexer.c -o "$work/seed-lexer.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/parser.c -o "$work/seed-parser.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/checker.c -o "$work/seed-checker.o"
"${CC:-cc}" "${c_flags[@]}" -c bootstrap/sources.c -o "$work/seed-sources.o"
"${CXX:-c++}" "${host_flags[@]}" -DCOAX_BOOTSTRAP_IMPORTER -I bootstrap -I "$work" -o "$work/stage0" src/main.cpp "$work/seed-check-domains.o" "$work/seed-mutation-schedule.o" "$work/seed-budget.o" "$work/seed-profiles.o" "$work/seed-lexer.o" "$work/seed-parser.o" "$work/seed-checker.o" "$work/seed-sources.o" ${link_flags[@]+"${link_flags[@]}"}
fi
# Gate results. A passing stage0 gate is kept in $COAX_GATE_CACHE (default
# ~/.cache/coax-gates/) under a key that hashes stage0's inputs, the gate's
# command and the bytes of every source the gate reads (`stage0 deps`). A gate
# with the same key passed before on this machine is reused, and the log names
# it; any other gate runs. Only passes are kept. COAX_BUILD_FRESH=1 runs every
# gate. Every gate's time is printed, so the log shows where the build goes.
sha() { if command -v shasum > /dev/null 2>&1; then shasum -a 256 | awk '{print $1}'; else sha256sum | awk '{print $1}'; fi; }
gate_cache="${COAX_GATE_CACHE:-$HOME/.cache/coax-gates}"
[[ -z "${COAX_BUILD_FRESH:-}" ]] || gate_cache=""
stage0_identity() {
  local path
  {
    printf 'coax-stage0-v1\0'
    while IFS= read -r -d '' path; do printf '%s\0%s\0' "$path" "$(sha < "$path")"; done < <(find src bootstrap -type f -print0 | sort -z)
    printf 'proto_runtime\0%s\0' "$(sha < "$work/proto_runtime.inc")"
    printf 'flags\0%s\0%s\0' "${host_flags[*]}" "${c_flags[*]}"
    printf 'cc\0%s\0%s\0' "$(command -v "${CC:-cc}")" "$("${CC:-cc}" --version)"
    printf 'cxx\0%s\0%s\0' "$(command -v "${CXX:-c++}")" "$("${CXX:-c++}" --version)"
    printf 'host\0%s\0%s\0' "$(uname -s)" "$(uname -m)"
    if [[ "$(uname -s)" == Darwin ]]; then printf 'sdk\0%s\0%s\0' "$(xcrun --show-sdk-path)" "$(xcrun --show-sdk-version)"; fi
    for path in COAX_CC COAX_LDFLAGS COAX_HOST_TARGET SDKROOT MACOSX_DEPLOYMENT_TARGET IPHONEOS_DEPLOYMENT_TARGET DEVELOPER_DIR CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH; do
      printf '%s\0%s\0' "$path" "${!path-}"
    done
  } | sha
}
# The key of a stage0 gate in $key, left empty when its sources can't be
# listed. Gates run in the main shell, so one file's closure is hashed once.
closure_file="" closure=""
gate_key() {
  local deps path list
  key=""
  if [[ "$closure_file" != "$3" ]]; then
    closure_file="$3" closure=""
    deps="$("$work/stage0" deps "$3" 2> /dev/null)" || return 0
    list="$(printf '%s' "$deps" | tr -d '\n' | sed -n 's/.*"sources":\[\(.*\)\],"descriptors":\[\(.*\)\]}$/\1,\2/p' | tr ',' '\n' | sed -e 's/^"//' -e 's/"$//')"
    [[ -n "$list" ]] || return 0
    while IFS= read -r path; do
      [[ -z "$path" || ( -f "$path" && ! -L "$path" ) ]] || return 0
    done <<< "$list"
    closure="$({
      printf '%s\0' "${deps//"$root"\//}"
      while IFS= read -r path; do
        [[ -n "$path" ]] && printf '%s\0%s\0' "${path#"$root"/}" "$(sha < "$path")"
      done <<< "$list"
    } | sha)"
  fi
  [[ -n "$closure" ]] || return 0
  key="$({ printf 'coax-gate-v1\0%s\0%s\0' "$stage0_id" "$closure"; printf '%s\0' "${@:2}"; } | sha)"
}
# A mutation gate that fails only on survivors is judged by mutation-gate.sh
# against COAX_MUTATION_CHANGED (the lines changed since a base, from
# coax/build.sh; unset or "strict": every survivor gates). A survivor gates on
# a changed line, or when the change removed a check from its file or from the
# gate's mutated file. Other survivors are listed in bin/mutation-survivors and
# do not stop the build. Their
# gate result is kept with its survivors, and judged again on reuse.
mutation_changed="${COAX_MUTATION_CHANGED:-strict}"
: > "$work/mutation-survivors"
judge() {   # judge LABEL SURVIVORS: 0 when none of them gates
  local gating
  # Labelled, so a check removed from the mutated file gates its survivors.
  sed "s|^|$1: |" "$2" > "$work/survivors.labelled"
  if gating="$(./mutation-gate.sh verdict "$mutation_changed" "$work/survivors.labelled")"; then
    echo "coax bootstrap: $(wc -l < "$2" | tr -d ' ') survivor(s) the change did not cause, reported, not gating: $1"
    sed "s|^|$1: |" "$2" >> "$work/mutation-survivors"
    return 0
  fi
  if [[ "$mutation_changed" == strict ]]; then echo "coax bootstrap: surviving mutants stop the build, the full mutation check ($1):" >&2
  else echo "coax bootstrap: surviving mutants on changed lines stop the build ($1):" >&2; fi
  printf '%s\n' "$gating" | sed 's/^/  /' >&2
  return 1
}
invoke_gate() {
  # A manifest is only an input to the real supported merge. The original
  # full mutation command remains the gate/cache identity, and gate() alone
  # records a successful verdict. No partial record or caller pass bit waives it.
  if [[ $# == 3 && "$1" == "$work/stage0" && "$2" == mutate && "$3" == lib/compiler/checker.coax && -n "${COAX_CHECKER_MUTATION_SHARDS:-}" ]]; then
    gate_key "$@"
    [[ -n "$closure" ]] || { echo 'coax bootstrap: cannot bind checker shard source closure' >&2;return 1; }
    ./bootstrap-mutation-shards.sh "$COAX_CHECKER_MUTATION_SHARDS" "$1" "$closure" "$3"
  else
    "$@"
  fi
}
gate() {
  local start=$SECONDS label="${*//"$work"\//}" key=""
  # COAX_MUTATION=0 (release.sh): no mutation gate runs. Mutation is a
  # report, never a wait: the publisher and the full check run it.
  if [[ "$2" == mutate && "${COAX_MUTATION:-1}" == 0 ]]; then echo "coax bootstrap: skipped  $label (COAX_MUTATION=0)"; return 0; fi
  if [[ -n "$gate_cache" && "$1" == "$work/stage0" && ( "$2" == test || "$2" == mutate ) ]]; then
    gate_key "$@"
    if [[ -n "$key" && -f "$gate_cache/$key" ]]; then
      sed -n 's/^survivor //p' "$gate_cache/$key" > "$work/survivors"
      if [[ ! -s "$work/survivors" ]]; then echo "coax bootstrap: reused   $label"; return 0; fi
      echo "coax bootstrap: reused   $label (with survivors)"
      judge "$label" "$work/survivors"; return
    fi
  fi
  if invoke_gate "$@" > "$work/gate.log" 2>&1; then
    echo "coax bootstrap: $(printf '%5d' $((SECONDS - start))) s  $label"
    if [[ -n "$key" ]] && mkdir -p "$gate_cache" 2> /dev/null; then
      printf '%s\n' "$label" > "$gate_cache/.$key.$$" && mv "$gate_cache/.$key.$$" "$gate_cache/$key"
    fi
    return 0
  fi
  if [[ "$2" == mutate ]] && ./mutation-gate.sh survivors < "$work/gate.log" > "$work/survivors"; then
    echo "coax bootstrap: $(printf '%5d' $((SECONDS - start))) s  $label"
    if [[ -n "$key" ]] && mkdir -p "$gate_cache" 2> /dev/null; then
      { printf '%s\n' "$label"; sed 's/^/survivor /' "$work/survivors"; } > "$gate_cache/.$key.$$" && mv "$gate_cache/.$key.$$" "$gate_cache/$key"
    fi
    judge "$label" "$work/survivors" && return 0
  fi
  cat "$work/gate.log" >&2
  return 1
}
stage0_id="$(stage0_identity)"
if [[ -n "$replay_input" ]]; then
  {
    IFS= read -r saved_schema; IFS= read -r saved_stage0; IFS= read -r saved_binary; IFS= read -r saved_input
    if IFS= read -r extra || [[ -n "${extra:-}" ]]; then echo 'coax bootstrap: malformed retained stage0 record' >&2;exit 1;fi
  } < "$replay_input/stage0.record"
  [[ "$saved_schema" == coax-stage0-artifact-v1 && "$saved_stage0" == "$stage0_id" && "$saved_binary" == "$(sha < "$work/stage0")" && "$saved_input" == "$input_identity" ]] || { echo 'coax bootstrap: retained stage0 binary or input identity differs' >&2;exit 1; }
  gate "$work/stage0" mutate lib/compiler/checker.coax
  exit 0
fi
# Construction identity is not a pass: replay still requires the real complete
# merge. Retaining it lets a bounded command finish that gate later through the
# same gate/cache machinery without rebuilding a different debug-path binary.
printf 'coax-stage0-artifact-v1\n%s\n%s\n%s\n' "$stage0_id" "$(sha < "$work/stage0")" "$input_identity" > "$work/stage0.record"
# Validate release metadata before expensive compiler gates. Stage0 uses the
# checked-in header seed; final publication must match the canonical Coax data.
gate "$work/stage0" test release-model.coax
gate "$work/stage0" mutate release-model.coax
gate "$work/stage0" test changelog-model.coax
gate "$work/stage0" mutate changelog-model.coax --root-only
COAX_RELEASE_COMPILER="$PWD/$work/stage0" ../scripts/coax-release.sh --check
gate "$work/stage0" test lib/compiler/budget.coax
gate "$work/stage0" mutate lib/compiler/budget.coax
"$work/stage0" c lib/compiler/budget.coax --library --header "$work/budget.h" > "$work/budget.c"
cmp bootstrap/budget.c "$work/budget.c"
cmp bootstrap/budget.h "$work/budget.h"
gate "$work/stage0" test lib/compiler/check-domains.coax
gate "$work/stage0" mutate lib/compiler/check-domains.coax --root-only
"$work/stage0" c lib/compiler/check-domains.coax --library --header "$work/check-domains.h" > "$work/check-domains.c"
cmp bootstrap/check-domains.c "$work/check-domains.c"
cmp bootstrap/check-domains.h "$work/check-domains.h"
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
gate "$work/stage0" test lib/compiler/effects.coax
gate "$work/stage0" mutate lib/compiler/effects.coax --root-only
gate "$work/stage0" test lib/compiler/quantity.coax
gate "$work/stage0" mutate lib/compiler/quantity.coax --root-only
gate "$work/stage0" test lib/compiler/native-checks.coax
gate "$work/stage0" mutate lib/compiler/native-checks.coax
gate "$work/stage0" test lib/compiler/native-execution.coax
# Dependency policies have their own mutation gates above; retain their full baseline here.
gate "$work/stage0" mutate lib/compiler/native-execution.coax --root-only
gate "$work/stage0" test lib/compiler/quantified-execution.coax
gate "$work/stage0" mutate lib/compiler/quantified-execution.coax --root-only
gate "$work/stage0" test lib/compiler/native-semantics.coax
gate "$work/stage0" mutate lib/compiler/native-semantics.coax --root-only
gate "$work/stage0" test lib/compiler/spirv.coax
gate "$work/stage0" mutate lib/compiler/spirv.coax
gate "$work/stage0" test lib/compiler/shader.coax
gate "$work/stage0" mutate lib/compiler/shader.coax
gate "$work/stage0" test lib/compiler/wgsl.coax
gate "$work/stage0" mutate lib/compiler/wgsl.coax
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
gate "$work/stage0" test lib/crypto/sha256-kernel.coax
gate "$work/stage0" mutate lib/crypto/sha256-kernel.coax
gate "$work/stage0" test lib/proto/fingerprint.coax
gate "$work/stage0" mutate lib/proto/fingerprint.coax
gate "$work/stage0" test lib/proto/importer.coax
gate "$work/stage0" mutate lib/proto/importer.coax
"$work/stage0" c lib/compiler/compiler.coax --library --header "$work/proto_importer.h" > "$work/proto_importer.c"
"${CC:-cc}" "${c_flags[@]}" -c "$work/proto_importer.c" -o "$work/proto_importer.o"
"${CXX:-c++}" "${host_flags[@]}" -I "$work" -o "$work/coax" src/main.cpp "$work/proto_importer.o" ${link_flags[@]+"${link_flags[@]}"}
"$work/coax" c lib/compiler/compiler.coax --library > "$work/rebuilt.c"
cmp "$work/proto_importer.c" "$work/rebuilt.c"
gate "$work/coax" test lib/compiler/budget.coax
"$work/coax" c lib/compiler/budget.coax --library --header "$work/final-budget.h" > "$work/final-budget.c"
cmp bootstrap/budget.c "$work/final-budget.c"
cmp bootstrap/budget.h "$work/final-budget.h"
gate "$work/coax" test lib/compiler/check-domains.coax
"$work/coax" c lib/compiler/check-domains.coax --library --header "$work/final-check-domains.h" > "$work/final-check-domains.c"
cmp bootstrap/check-domains.c "$work/final-check-domains.c"
cmp bootstrap/check-domains.h "$work/final-check-domains.h"
gate "$work/coax" test lib/compiler/profiles.coax
"$work/coax" c lib/compiler/profiles.coax --library --header "$work/final-profiles.h" > "$work/final-profiles.c"
cmp bootstrap/profiles.c "$work/final-profiles.c"
cmp bootstrap/profiles.h "$work/final-profiles.h"
gate "$work/coax" test lib/compiler/provenance.coax
"$work/coax" c lib/compiler/provenance.coax --library --header "$work/final-provenance.h" > "$work/final-provenance.c"
cmp "$work/provenance.c" "$work/final-provenance.c"
cmp "$work/provenance.h" "$work/final-provenance.h"
"${CC:-cc}" "${c_flags[@]}" -I "$work" "$work/provenance.c" provenance-bridge.c -o "$work/provenance-check" ${link_flags[@]+"${link_flags[@]}"}
gate "$work/coax" test lib/compiler/mutation-schedule.coax
"$work/coax" c lib/compiler/mutation-schedule.coax --library --header "$work/final-mutation-schedule.h" > "$work/final-mutation-schedule.c"
cmp bootstrap/mutation-schedule.c "$work/final-mutation-schedule.c"
cmp bootstrap/mutation-schedule.h "$work/final-mutation-schedule.h"
gate "$work/coax" test lib/compiler/lexer.coax
gate "$work/coax" test lib/compiler/parser.coax
gate "$work/coax" test lib/compiler/checker.coax
gate "$work/coax" test lib/compiler/sources.coax
gate "$work/coax" test lib/compiler/native-checks.coax
gate "$work/coax" test lib/compiler/native-execution.coax
gate "$work/coax" test lib/compiler/quantified-execution.coax
gate "$work/coax" test lib/compiler/native-semantics.coax
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
cp "$work/check-domains.c" bin/check-domains.c
cp "$work/check-domains.h" bin/check-domains.h
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
cp "$work/mutation-survivors" bin/mutation-survivors
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
