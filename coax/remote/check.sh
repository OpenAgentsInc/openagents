#!/usr/bin/env bash
# Firm reason: 5. One check of the green-main pipeline (docs/verse/green-main-pipeline.md,
# #13102), run from the root of the commit under check, on a remote build host
# (integrate.sh and after.sh send it with remote.sh run --commit). A thin
# wrapper: git, coax/web/tool.sh suites, coax/tests/run.sh, the web build and
# the report checks, then grep and awk over their output.
#
#   coax/remote/check.sh branch BASE      the suites whose sources changed since BASE,
#                                         and the web build when the branch touched
#                                         what it builds (the integrator's check)
#   coax/remote/check.sh suites I/N       the I-th of N parts of every suite
#   coax/remote/check.sh only PATH,PATH   these suites (a bisect)
#   coax/remote/check.sh compiler I/N     the I-th of N parts of coax/tests/run.sh
#   coax/remote/check.sh wasm             the web build: every module against its budget
#   coax/remote/check.sh modules I/N      the I-th of N parts of the web build's modules
#                                         (wasm.sh alone, as build.sh runs each)
#   coax/remote/check.sh tools            tools-check, everglade-parity, the committed
#                                         shaders against their Coax, the room build
#
# It writes build/pipeline/: MODE.log (the output), status, failed (one line
# per failing suite or step), limits.txt (each compile-time limit note:
# "PERCENT CODE RESOURCE USED LIMIT UNIT"), sizes.txt ("MODULE BYTES BUDGET
# PERCENT") and warnings.txt (limits above 90%, modules above 98% of budget).
# Exit 0 when the check passed, 1 when it failed.
set -uo pipefail
export LC_ALL=C
mode="${1:-}"; arg="${2:-}"
root="$(git rev-parse --show-toplevel)" || exit 2
cd "$root" || exit 2
out="$root/build/pipeline"; rm -rf "$out"; mkdir -p "$out"; : > "$out/started"   # sizes() reads modules newer than this
log="$out/$mode.log"; : > "$out/failed"
step() {   # name cmd...: run one step into the log; a failure is listed
  local name="$1"; shift
  echo "== $name" >> "$log"
  local t0; t0=$(date +%s)
  "$@" >> "$log" 2>&1
  local s=$?
  echo "== $name: status $s, $(( $(date +%s) - t0 )) s" >> "$log"
  (( s == 0 )) || echo "step $name" >> "$out/failed"
  return "$s"
}
# The compiler's build record names the C compilers and settings it was built
# with (coax/provenance.sh), and tool.sh suites refuses a compiler whose record
# doesn't match. A host builds it with static GCC for musl from nixpkgs
# (host-job.sh), so the suites run in that environment there.
compiler_env=()
if ! coax/provenance.sh verify "$root" coax/bin/coax coax/bin/provenance-check coax/bin/provenance.record > /dev/null 2>&1 &&
   command -v nix > /dev/null; then
  m="$(uname -m)"
  compiler_env=(nix shell nixpkgs#pkgsStatic.stdenv.cc nixpkgs#python3 nixpkgs#gnumake --command
    env CC="$m-unknown-linux-musl-gcc" CXX="$m-unknown-linux-musl-g++" COAX_LDFLAGS=-static)
fi
suites() {   # args for tool.sh suites
  step suites ${compiler_env[@]+"${compiler_env[@]}"} coax/web/tool.sh suites --jobs "${OA_CHECK_JOBS:-8}" "$@"
  local s=$?
  grep -E '^FAIL ' "$log" | sed -E 's/^FAIL ([^ :]+).*/suite \1/' | sort -u >> "$out/failed"
  return "$s"
}
# Each wasm.sh unit compile prints its near-limit notes on stderr
# ("coax: note[E0400]: NAME: USED of LIMIT used (P%), ..."). wasm.sh names the
# module on its own lines, so a note goes with the module named last before it
# in the module's own output. build.sh interleaves modules, so the unit is the
# best guess, "?" when there is none.
limits() {
  awk '
    /^wasm\.sh: [A-Za-z0-9_-]+\.wasm/ { split($2, a, "."); unit = a[1] }
    /^coax: note\[E[0-9]+\]: / {
      code = $2; sub(/^note\[/, "", code); sub(/\]:$/, "", code)
      line = $0; sub(/^coax: note\[E[0-9]+\]: /, "", line)
      name = line; sub(/: [0-9]+ of [0-9]+ used.*/, "", name); gsub(/ /, "_", name)
      if (match(line, /[0-9]+ of [0-9]+ used \([0-9]+%\)/)) {
        split(substr(line, RSTART, RLENGTH), f, /[ ()%]+/)
        print f[5], code, name, f[1], f[3], (unit == "" ? "?" : unit)
      }
    }' "$out"/*.log | sort -u | sort -k1,1nr > "$out/limits.txt"
  awk '$1 >= 90 { print "limit " $6 ": " $3 " at " $1 "% (" $4 " of " $5 ", " $2 ")" }' "$out/limits.txt" >> "$out/warnings.txt"
}
sizes() {
  local budgets=apps/openagents-web/wasm-budgets.txt
  [[ -f "$budgets" ]] || return 0
  awk '!/^#/ && NF == 2 { print $1, $2 }' "$budgets" | while read -r name budget; do
    local f="apps/openagents-web/build/$name.wasm"
    [[ -f "$f" && "$f" -nt "$out/started" ]] || continue
    local bytes; bytes=$(wc -c < "$f" | tr -d ' ')
    echo "$name $bytes $budget $(( bytes * 100 / budget ))"
  done > "$out/sizes.txt"
  awk '$4 >= 98 { print "size " $1 ": " $2 " bytes of " $3 " (" $4 "%)" }' "$out/sizes.txt" >> "$out/warnings.txt"
}
# What the web build builds: a branch that changes none of it skips the build.
web_inputs='^(apps/openagents-web/|aethyr/|verse/|coax/web/|coax/lib/|coax/src/|coax/platform/|coax/bootstrap/|psionic/|protocol/|everglade/)'
# A change to these rebuilds every module (the compiler, the Wasm toolchain, the build itself).
web_whole='^(coax/src/|coax/bootstrap/|coax/lib/compiler/|coax/platform/|coax/web/wasm\.sh|coax/web/toolchain\.sh|coax/web/wasm-size\.coax|apps/openagents-web/build\.sh)'
# The modules a branch touched, from build.sh's wasm.sh lines: a module is
# touched when the branch changed one of its units' sources (coax deps) or
# its face. Each runs as build.sh runs it; wasm.sh checks its budget.
# build.sh's wasm.sh commands, one a line. project and projects draw from
# tables build.sh writes from Nostr first (openagents project bake), so only
# build.sh (the release) builds them.
module_cmds() {
  awk '{ if (sub(/\\$/, "")) { buf = buf $0 " "; next } print buf $0; buf = "" }' "$root/apps/openagents-web/build.sh" |
    grep 'coax/web/wasm.sh"' | sed -E 's/[[:space:]]*&[[:space:]]*$//' | grep -vE '/build/(project|projects)\.wasm"'
}
web_touched() {
  local here="$root/apps/openagents-web" m a cmd words w hit
  m="$here/../../verse"; a="$here/../../aethyr"
  : > "$out/modules"
  # The .coax files the change reaches through #load (suites --affected); when
  # the graph can't tell, every module is built.
  ${compiler_env[@]+"${compiler_env[@]}"} coax/web/tool.sh suites --since "$arg" --affected > "$out/affected" 2>> "$log"
  case $? in
    0) ;;
    3) module_cmds > "$out/modules"; build_modules; return ;;
    *) echo "modules: suites --affected failed, so the touched modules are unknown" >> "$log"; return 1 ;;
  esac
  while IFS= read -r cmd; do
    eval "words=($cmd)" 2> /dev/null || continue
    hit=0
    for w in "${words[@]}"; do
      case "$w" in
        *.c|*.coax) grep -qxF "$(realpath -m --relative-to="$root" "$w" 2> /dev/null)" "$out/affected" "$out/changed" && { hit=1; break; } ;;
      esac
    done
    (( hit == 0 )) || printf '%s\n' "$cmd" >> "$out/modules"
  done < <(module_cmds)
  build_modules
}
build_modules() {   # each command in $out/modules, as build.sh runs it
  local here="$root/apps/openagents-web" m a cmd
  m="$here/../../verse"; a="$here/../../aethyr"
  # build.sh builds wasm.sh's size tool once before the modules start side by side.
  (cd "$root" && coax/web/tool.sh wasm-size sources apps/openagents-web/fixture.coax > /dev/null 2>&1)
  echo "$(wc -l < "$out/modules" | tr -d ' ') module(s):" >> "$log"
  sed -E 's#.*/build/([A-Za-z0-9_-]+)\.wasm.*#  \1#' "$out/modules" >> "$log"
  local status=0 pids=() p
  while IFS= read -r cmd; do
    local name; name="$(sed -E 's#.*/build/([A-Za-z0-9_-]+)\.wasm.*#\1#' <<< "$cmd")"
    ( cd "$here" && here="$here" m="$m" a="$a" COAXIAL_MODE=release bash -c "$cmd" > "$out/module-$name.log" 2>&1 ||
        { echo "module $name" >> "$out/failed"; exit 1; } ) & pids+=($!)
    if (( ${#pids[@]} >= ${OA_CHECK_MODULES:-4} )); then wait "${pids[0]}" || status=1; pids=("${pids[@]:1}"); fi
  done < "$out/modules"
  for p in ${pids[@]+"${pids[@]}"}; do wait "$p" || status=1; done
  return "$status"
}

: > "$out/warnings.txt"
status=0
case "$mode" in
  branch)
    [[ -n "$arg" ]] || { echo "check.sh branch BASE" >&2; exit 2; }
    git diff --name-only "$arg" HEAD > "$out/changed"
    echo "$(wc -l < "$out/changed" | tr -d ' ') files changed since ${arg:0:10}" > "$log"
    # Documents alone run nothing (suites --since reads every suite's sources).
    if grep -qvE '(\.md$|^docs/)' "$out/changed"; then suites --since "$arg" || status=1
    else echo "== suites: skipped, the branch changes only documents" >> "$log"; fi
    if grep -Eq "$web_whole" "$out/changed"; then
      step wasm env COAXIAL_MODE=release apps/openagents-web/build.sh || status=1
    elif grep -v '\.md$' "$out/changed" | grep -Eq "$web_inputs"; then
      step modules web_touched || status=1
      cat "$out"/module-*.log >> "$log" 2> /dev/null
    else
      echo "== wasm: skipped, the branch changes nothing the web build reads" >> "$log"
    fi ;;
  suites) suites --part "$arg" || status=1 ;;
  only) suites --only "$arg" || status=1 ;;
  compiler)
    # In the host's own toolchain, not compiler_env: the units build their own
    # sanitized programs with cc and c++, and the static musl toolchain has no
    # ASan or UBSan runtimes (cannot find libasan_preinit.o, #13134).
    step compiler coax/tests/run.sh --part "$arg" || status=1
    grep -E '^FAIL ' "$log" | sed -E 's/^FAIL ([^ :]+).*/compiler \1/' | sort -u >> "$out/failed" ;;
  wasm) step wasm env COAXIAL_MODE=release apps/openagents-web/build.sh || status=1 ;;
  modules)
    module_cmds | awk -v p="$arg" 'BEGIN { split(p, f, "/") } (NR - 1) % f[2] == f[1] - 1' > "$out/modules"
    step modules build_modules || status=1
    cat "$out"/module-*.log >> "$log" 2> /dev/null ;;
  tools)
    step tools-check coax/web/tool.sh tools-check || status=1
    step everglade-parity coax/web/tool.sh everglade-parity || status=1
    step wgsl coax/web/tool.sh coax/web/wgsl-module.coax --site --check || status=1
    step room-build coax/web/room/build.sh || status=1 ;;
  *) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
limits; sizes
echo "$status" > "$out/status"
[[ ! -s "$out/warnings.txt" ]] || { echo "warnings:"; sed 's/^/  /' "$out/warnings.txt"; }
[[ ! -s "$out/failed" ]] || { echo "failed:"; sed 's/^/  /' "$out/failed"; }
echo "check $mode${arg:+ ${arg:0:60}}: $([[ $status == 0 ]] && echo passed || echo FAILED)"
exit "$status"
