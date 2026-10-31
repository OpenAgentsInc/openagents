#!/usr/bin/env bash
# Bootstrap from a private source snapshot, publish only matching artifacts.
#   coax/build.sh          build
#   coax/build.sh test     build, then run tests/run.sh
#
# A checked build is kept in a cache keyed by the compiler-input fingerprint
# ($COAX_RELEASE_CACHE, default ~/.cache/coax-release/<fingerprint>/, the
# release's cache, coax/web/release.sh). When the cache has a build for this
# tree's fingerprint, build.sh installs it with provenance.sh install, which
# verifies its record against this tree's inputs first, exactly as a fresh
# build is verified; nothing is skipped or rewritten. Otherwise it bootstraps
# (one builder per fingerprint on this machine; others wait for it) and keeps
# the checked build there. COAX_BUILD_FRESH=1 always bootstraps.
set -euo pipefail
export LC_ALL=C
cd "$(dirname "$0")"
root="$(cd .. && pwd -P)"
expected="$(./provenance.sh fingerprint "$root")"
artifacts=(check-domains.c check-domains.h lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h mutation-survivors)
cache_root="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release}"
cache="$cache_root/$expected"
building="" mutation_changed=strict changed_file=""
trap '[[ -z "$building" ]] || rm -rf "$building"; [[ -z "$changed_file" ]] || rm -f "$changed_file"' EXIT

# Which surviving mutants stop this build (coax/mutation-gate.sh): those on
# lines changed since a base. The base is COAX_MUTATION_BASE, or else where
# this checkout left origin/main; COAX_MUTATION_CHANGED names a ready list
# (release.sh passes the lines changed since production). Survivors on other
# lines are reported in coax/bin/mutation-survivors. COAX_MUTATION_FULL=1, or
# no base, gates on every survivor: the full mutation check.
if [[ -n "${COAX_MUTATION_FULL:-}" ]]; then mutation_changed=strict
elif [[ -n "${COAX_MUTATION_CHANGED:-}" ]]; then
  mutation_changed="$COAX_MUTATION_CHANGED"
  [[ "$mutation_changed" == strict || -f "$mutation_changed" ]] || { echo "coax/build.sh: no changed-lines file $mutation_changed" >&2; exit 2; }
  [[ "$mutation_changed" == strict ]] || mutation_changed="$(cd "$(dirname "$mutation_changed")" && pwd -P)/$(basename "$mutation_changed")"
else
  base="${COAX_MUTATION_BASE:-}"
  [[ -n "$base" ]] || base="$(git -C "$root" merge-base HEAD origin/main 2> /dev/null || true)"
  if [[ -n "$base" ]]; then
    changed_file="$(mktemp "${TMPDIR:-/tmp}/coax-mutation-changed.XXXXXXXX")"
    ./mutation-gate.sh changed "$root" "$base" > "$changed_file"
    mutation_changed="$changed_file"
  fi
fi
export COAX_MUTATION_CHANGED="$mutation_changed"
# The survivors a cached build tolerated, as PATH:LINE, must not gate here.
survivors_allowed() {
  local list
  [[ -s "$1/mutation-survivors" ]] || return 0
  list="$(mktemp "${TMPDIR:-/tmp}/coax-survivors.XXXXXXXX")"
  sed 's/^.*: //' "$1/mutation-survivors" > "$list"
  if ./mutation-gate.sh verdict "$mutation_changed" "$list" > /dev/null; then rm -f "$list"; return 0; fi
  rm -f "$list"; return 1
}
report_survivors() {
  [[ -s bin/mutation-survivors ]] || return 0
  echo "coax/build.sh: $(wc -l < bin/mutation-survivors | tr -d ' ') surviving mutant(s) on unchanged lines, reported in coax/bin/mutation-survivors, not gating (the full check: COAX_MUTATION_FULL=1 coax/build.sh)" >&2
}

# The C artifacts beside the compiler (native tests link them). A cached build
# kept by build.sh has them; one kept by release.sh has only the compiler, and
# they are derived: the seeds' bytes are what the bootstrap's gates compared
# them to (bootstrap/*.c and .h), the proto runtime is its sources, and the
# importer is the verified compiler's own output, as the bootstrap made it.
artifacts_from() {
  local from="$1" name work
  work="$(mktemp -d bin/.artifacts.XXXXXXXX)"
  for name in "${artifacts[@]}"; do
    if [[ -f "$from/$name" && ! -L "$from/$name" ]]; then cp "$from/$name" "$work/$name"; continue; fi
    case "$name" in
      check-domains.*|lexer.*|parser.*|checker.*|sources.*) cp "bootstrap/$name" "$work/$name" ;;
      mutation-survivors) : > "$work/$name" ;;   # kept before survivors were reported: a strict build
      proto_runtime.inc)
        { printf 'static constexpr char PROTO_CORE_SOURCE[] = R"COAX_CORE('; cat lib/proto/wire.coax
          printf ')COAX_CORE";\nstatic constexpr char PROTO_MEMORY_SOURCE[] = R"COAX_MEMORY('; cat lib/proto/memory.coax
          printf ')COAX_MEMORY";\nstatic constexpr char PROTO_IMPORTER_SOURCE[] = R"COAX_IMPORTER('; cat lib/proto/importer.coax
          printf ')COAX_IMPORTER";\nstatic constexpr char PROTO_FINGERPRINT_SOURCE[] = R"COAX_FP('; cat lib/proto/fingerprint.coax
          printf ')COAX_FP";\n'; } > "$work/$name" ;;
      proto_importer.c) bin/coax c lib/compiler/compiler.coax --library --header "$work/proto_importer.h" > "$work/proto_importer.c" ;;
      proto_importer.h) [[ -f "$work/proto_importer.h" ]] || bin/coax c lib/compiler/compiler.coax --library --header "$work/proto_importer.h" > /dev/null ;;
    esac
  done
  for name in "${artifacts[@]}"; do mv "$work/$name" "bin/$name"; done
  rm -rf "$work"
}
# Documentation is not a compiler input: it is validated on every build, with
# the verified compiler that is about to be installed, and stale documents
# leave coax/bin unchanged. A warm build compiles nothing; a bootstrap also
# checks them with its stage0 before the expensive gates.
# An explicit generator override (COAX_RELEASE_COMPILER) is trusted by its caller.
check_documents() {
  if [[ -n "${1:-}" && -z ${COAX_RELEASE_COMPILER:-} ]]; then COAX_RELEASE_COMPILER="$1" "$root/scripts/coax-release.sh" --check; return; fi
  if [[ -n ${COAX_RELEASE_COMPILER:-} ]] || ./provenance.sh verify "$root" "$root/coax/bin/coax" "$root/coax/bin/provenance-check" "$root/coax/bin/provenance.record" > /dev/null 2>&1; then
    "$root/scripts/coax-release.sh" --check
  else
    COAX_RELEASE_COMPILER=/dev/null "$root/scripts/coax-release.sh" --check
  fi
}
# The hash of this checkout's compiler sources beside the compiler, so a
# failing command can say the compiler is older than the checkout (coax
# source-digest). A compiler without the command leaves no digest.
write_source_digest() {
  if bin/coax source-digest "$root" > "bin/.source-digest.$$" 2> /dev/null; then mv "bin/.source-digest.$$" bin/source-digest
  else rm -f "bin/.source-digest.$$" bin/source-digest; fi
}
from_cache() {
  [[ -z "${COAX_BUILD_FRESH:-}" && -d "$cache" ]] || return 1
  survivors_allowed "$cache" || return 1
  # Verify the cached build, then validate the release documents with it
  # before anything is installed: stale documents leave coax/bin unchanged.
  ./provenance.sh verify "$root" "$cache/coax" "$cache/provenance-check" "$cache/provenance.record" 2> /dev/null || return 1
  check_documents "$cache/coax" || exit 1
  mkdir -p bin
  ./provenance.sh install "$root" "$cache" 2> /dev/null || return 1
  artifacts_from "$cache"
  write_source_digest
  echo "coax/build.sh: installed the checked build ${expected:0:12} from $cache_root"
}
# A checked build into the cache, whole or not at all, verified again first.
keep() {
  local into name
  mkdir -p "$cache_root" 2> /dev/null && into="$(mktemp -d "$cache_root/.keep.XXXXXXXX" 2> /dev/null)" || { echo "coax/build.sh: can't write $cache_root; not cached" >&2; return 0; }
  cp bin/coax bin/provenance-check bin/provenance.record "$into/"
  for name in "${artifacts[@]}"; do cp "bin/$name" "$into/"; done
  if ./provenance.sh verify "$root" "$into/coax" "$into/provenance-check" "$into/provenance.record"; then
    rm -rf "$cache"; mv "$into" "$cache"
  else rm -rf "$into"; fi
}

if from_cache; then
  report_survivors
else
  # One builder per fingerprint on this machine (release.sh builds through
  # here too); a lock left by a dead builder is taken over.
  if mkdir -p "$cache_root" 2> /dev/null; then
    lock="$cache_root/$expected.building"
    until mkdir "$lock" 2> /dev/null; do
      holder="$(cat "$lock/pid" 2> /dev/null || true)"
      if [[ -n "$holder" ]] && ! kill -0 "$holder" 2> /dev/null; then rm -rf "$lock"; continue; fi
      # A lock whose taker died before writing its pid: stale after a minute.
      if [[ -z "$holder" && -n "$(find "$lock" -maxdepth 0 -mmin +1 2> /dev/null)" ]]; then rmdir "$lock" 2> /dev/null || true; continue; fi
      echo "coax/build.sh: waiting for the build of ${expected:0:12} (pid ${holder:-?})" >&2; sleep 3
    done
    building="$lock"; echo $$ > "$lock/pid"
  fi
  if from_cache; then :
  else
    snapshot="$(mktemp -d "${TMPDIR:-/tmp}/coax-build-snapshot.XXXXXXXX")"
    trap 'rm -rf "$snapshot"; [[ -z "$building" ]] || rm -rf "$building"; [[ -z "$changed_file" ]] || rm -f "$changed_file"' EXIT
    # Only what the bootstrap reads: its scripts and models (coax/*, files
    # only), the compiler's sources and seeds, and the documents
    # coax-release.sh checks. Never web/, tests/, node_modules or bin/.
    mkdir -p "$snapshot/coax" "$snapshot/scripts"
    find . -maxdepth 1 -type f -exec cp -p {} "$snapshot/coax/" \;
    for dir in src bootstrap lib/compiler lib/proto book generated; do
      [[ -d "$dir" ]] || continue
      mkdir -p "$snapshot/coax/$(dirname "$dir")"
      cp -R "$dir" "$snapshot/coax/$dir"
    done
    cp "$root/scripts/coax-release.sh" "$snapshot/scripts/coax-release.sh"
    [[ "$("$snapshot/coax/provenance.sh" fingerprint "$snapshot")" == "$expected" ]] || { echo 'compiler inputs changed while creating snapshot' >&2; exit 1; }
    "$snapshot/coax/bootstrap-build.sh" --private-snapshot
    # Compare the compiled snapshot to the current target inputs before any
    # compiler or supporting artifact is installed.
    ./provenance.sh verify "$root" "$snapshot/coax/bin/coax" "$snapshot/coax/bin/provenance-check" "$snapshot/coax/bin/provenance.record"
    check_documents "$snapshot/coax/bin/coax"
    mkdir -p bin
    [[ -f "$snapshot/coax/bin/mutation-survivors" ]] || : > "$snapshot/coax/bin/mutation-survivors"
    for artifact in "${artifacts[@]}"; do
      cp "$snapshot/coax/bin/$artifact" "bin/.$artifact.$$"
      mv "bin/.$artifact.$$" "bin/$artifact"
    done
    ./provenance.sh install "$root" "$snapshot/coax/bin"
    write_source_digest
    keep
  fi
  report_survivors
fi
if [[ "${1:-}" == "test" ]]; then
  [[ -z "${snapshot:-}" ]] || rm -rf "$snapshot"
  [[ -z "$building" ]] || rm -rf "$building"
  [[ -z "$changed_file" ]] || rm -f "$changed_file"
  trap - EXIT
  exec tests/run.sh
fi
