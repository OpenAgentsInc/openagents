#!/usr/bin/env bash
# Firm reason: 5. One check of the green-main pipeline (docs/verse/green-main-pipeline.md,
# #13102), run from the root of the commit under check, on a remote build host
# (integrate.sh and after.sh send it with remote.sh run --commit). A thin
# wrapper: git, coax/web/tool.sh suites, coax/tests/run.sh, the web build and
# the report checks, then grep and awk over their output.
#
#   coax/remote/check.sh branch BASE [ALSO...]
#                                         the suites whose sources changed since BASE,
#                                         the web build when the branch touched what it
#                                         builds, and the report checks (the integrator's
#                                         check); ALSO is one more check that must pass
#                                         ("only PATH", "reports"...: a revert's own check)
#   coax/remote/check.sh since BASE I/N   the I-th of N parts of the suites whose sources
#                                         changed since BASE (after.sh's fast run)
#   coax/remote/check.sh web BASE         the web modules the change since BASE touched,
#                                         and the report checks (a fast run)
#   coax/remote/check.sh room-since BASE  the room build, when the change since BASE
#                                         reaches it (a fast run; its own job: it takes
#                                         25 minutes)
#   coax/remote/check.sh suites I/N       the I-th of N parts of every suite
#   coax/remote/check.sh only PATH,PATH   these suites (a bisect)
#   coax/remote/check.sh compiler I/N     the I-th of N parts of coax/tests/run.sh
#   coax/remote/check.sh unit UNIT        one unit of coax/tests/run.sh (a bisect)
#   coax/remote/check.sh wasm             the web build: every module against its budget
#   coax/remote/check.sh modules I/N      the I-th of N parts of the web build's modules
#                                         (wasm.sh alone, as build.sh runs each)
#   coax/remote/check.sh module NAME      one module of the web build (a bisect)
#   coax/remote/check.sh reports          tools-check, test-remote.sh, everglade-parity, wire-check, layer-check, the pre-push replay, the committed
#                                         shaders, realm pages and changelog page against
#                                         their sources, and every realm's link-preview card
#   coax/remote/check.sh visuals          check-visuals alone (the web build's first stop:
#                                         a page or the host building visible UI)
#   coax/remote/check.sh room             the room build
#   coax/remote/check.sh tools            reports and room
#
# It writes build/pipeline/: MODE.log (the output), status, failed (one line
# per failing suite or step), limits.txt (each compile-time limit note:
# "PERCENT CODE RESOURCE USED LIMIT UNIT"), sizes.txt ("MODULE BYTES BUDGET
# PERCENT") and warnings.txt (limits above 90%, modules above 98% of budget).
# Exit 0 when the check passed, 1 when it failed, 97 when the host could not
# judge it (its compiler doesn't match the tree).
set -uo pipefail
export LC_ALL=C
mode="${1:-}"; arg="${2:-}"; also=("${@:3}")
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
  sed -nE 's/^FAIL ([^ :]+\.coax)([ :].*)?$/suite \1/p' "$log" | sort -u >> "$out/failed"
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
    grep 'coax/web/wasm.sh"' | grep -vE '/build/(project|projects)\.wasm"' |
    # build.sh's step timer (`timed NAME CMD`, #13154) and its `& made+=($!)` are its own: the command alone.
    sed -E 's/(^|[[:space:]])timed[[:space:]]+[A-Za-z0-9_-]+[[:space:]]+/\1/; s/[[:space:]]+&([[:space:]].*)?$//'
}
# The .coax files the change reaches through #load, in $out/affected, once: 0,
# 3 when the graph can't tell, else the tool failed.
affected_files() {
  if [[ ! -f "$out/affected.status" ]]; then
    ${compiler_env[@]+"${compiler_env[@]}"} coax/web/tool.sh suites --since "$arg" --affected --faces > "$out/affected" 2>> "$log"
    echo $? > "$out/affected.status"
    faces
  fi
  return "$(cat "$out/affected.status")"
}
# The changed C faces and headers, and every tracked .c or .h that includes
# one (by name, through any depth), in $out/faces: a module is touched when
# its command names one.
faces() {
  grep -E '\.[ch]$' "$out/changed" | sort -u > "$out/faces"
  local round new
  for round in 1 2 3 4 5 6; do
    [[ -s "$out/faces" ]] || break
    new="$(sed -E 's#.*/##; s#[.]#[.]#g' "$out/faces" | sort -u | paste -sd'|' -)"
    git grep -lE "#include \"([^\"]*/)?($new)\"" -- '*.c' '*.h' 2> /dev/null | cat - "$out/faces" | sort -u > "$out/faces.new"
    if cmp -s "$out/faces" "$out/faces.new"; then break; fi
    mv "$out/faces.new" "$out/faces"
  done
  rm -f "$out/faces.new"
}
# The room server builds room.coax and Everglade's headless world (room/build.sh).
room_units='coax/web/room/room.coax apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax'
room_touched() {   # whether the change since $arg reaches the room build (yes when it can't tell)
  grep -Eq "$web_whole|^coax/web/room/" "$out/changed" && return 0
  affected_files || return 0
  local u; for u in $room_units; do grep -qxF "$u" "$out/affected" && return 0; done
  return 1
}
web_touched() {
  local here="$root/apps/openagents-web" m a cmd words w hit
  m="$here/../../verse"; a="$here/../../aethyr"
  : > "$out/modules"
  # The graph (suites --affected) names the touched files; when it can't tell,
  # every module is built.
  affected_files
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
        *.c|*.coax) grep -qxF "$(realpath -m --relative-to="$root" "$w" 2> /dev/null)" "$out/affected" "$out/changed" "$out/faces" && { hit=1; break; } ;;
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
changed_since() {   # the files changed since $arg, in $out/changed
  [[ -n "$arg" ]] || { echo "check.sh $mode BASE" >&2; exit 2; }
  git diff --name-only "$arg" HEAD > "$out/changed" || exit 2
  echo "$(wc -l < "$out/changed" | tr -d ' ') files changed since ${arg:0:10}" >> "$log"
}
reports() {
  step tools-check coax/web/tool.sh tools-check || status=1
  step remote-cleanup coax/remote/test-remote.sh || status=1
  step everglade-parity coax/web/tool.sh everglade-parity || status=1
  step wire-check coax/web/tool.sh wire-check || status=1
  step layer-check coax/web/tool.sh layer-check || status=1
  step prepush-replay coax/web/tool.sh prepush --replay-check || status=1
  step wgsl coax/web/tool.sh coax/web/wgsl-module.coax --site --check || status=1
  visuals
  generated
  cards
}
# build.sh stops on it before any module, and no module job runs build.sh.
visuals() { step check-visuals coax/web/check-visuals.sh apps/openagents-web || status=1; }
# The other committed files the web build writes (#13117): each realm's page
# from realms.json, and the changelog page from coax/CHANGELOG.md.
generated() {
  step realm-pages bun coax/web/realms.ts --check || status=1
  step changelog-page coax/web/tool.sh coax/web/changelog-page.coax --check || status=1
}
# Every realm in realms.json has its link-preview card (#13144): a release refuses a realm without
# one, so it shows at landing, not at release.
cards() { step realm-cards bun coax/web/og.ts --cards apps/openagents-web || status=1; }
# and the WebTransport stack's check (#13175): wt-echo against a headless Chrome, where there is one
room() { step room-build coax/web/room/build.sh || status=1; step room-webtransport coax/web/room/wt-check.sh 9441 || status=1; }
# The web build for a change: all of it, its touched modules, or nothing.
web_since() {
  if grep -Eq "$web_whole" "$out/changed"; then
    step wasm env COAXIAL_MODE=release apps/openagents-web/build.sh || status=1
  elif grep -v '\.md$' "$out/changed" | grep -Eq "$web_inputs"; then
    step modules web_touched || status=1
    cat "$out"/module-*.log >> "$log" 2> /dev/null
  else
    echo "== wasm: skipped, the change touches nothing the web build reads" >> "$log"
  fi
}
# The report checks for a change: a tool file or a zone's source can be any
# file but a document; the shaders follow the web build's inputs.
reports_since() {
  if grep -q '^coax/remote/' "$out/changed"; then step remote-cleanup coax/remote/test-remote.sh || status=1; fi
  if grep -qv '\.md$' "$out/changed"; then
    step tools-check coax/web/tool.sh tools-check || status=1
    step everglade-parity coax/web/tool.sh everglade-parity || status=1
    # every wire shape registered (proto/registry.json, #13236); the allowlist against the base's copy
    step wire-check coax/web/tool.sh wire-check --base "$arg" || status=1
    # no load up the layers (aethyr/README.md "Layers", #13264); the allowlist against the base's copy
    step layer-check coax/web/tool.sh layer-check --base "$arg" || status=1
  fi
  # the pre-push check still refuses the historical clobbers and passes their fixes (#13265)
  if grep -Eq '^coax/(web/prepush\.coax|remote/hooks/)' "$out/changed"; then step prepush-replay coax/web/tool.sh prepush --replay-check || status=1; fi
  if grep -v '\.md$' "$out/changed" | grep -Eq "$web_inputs"; then
    step wgsl coax/web/tool.sh coax/web/wgsl-module.coax --site --check || status=1
    visuals
  fi
  # Their sources: the realm table and its generator, and the compiler's changelog.
  if grep -Eq '^(apps/openagents-web/realms|coax/web/realms\.ts|coax/CHANGELOG\.md|coax/web/changelog-page\.coax|apps/openagents-web/(changelog-content\.coax|coax/changelog/))' "$out/changed"; then
    generated
  fi
  if grep -Eq '^(apps/openagents-web/(realms|og/)|coax/web/og\.ts)' "$out/changed"; then
    cards
  fi
}
run() {   # mode arg: one check
  local mode="$1" arg="${2:-}"
  case "$mode" in
    suites) suites --part "$arg" || status=1 ;;
    only) suites --only "$arg" || status=1 ;;
    compiler|unit)
      # In the host's own toolchain, not compiler_env: the units build their own
      # sanitized programs with cc and c++, and the static musl toolchain has no
      # ASan or UBSan runtimes (cannot find libasan_preinit.o, #13134).
      if [[ "$mode" == unit ]]; then step compiler coax/tests/run.sh --only "$arg" || status=1
      else step compiler coax/tests/run.sh --part "$arg" || status=1; fi
      # A unit that failed ("== UNIT: FAILED (N) in S s") or run.sh's own note
      # ("FAIL UNIT: why"); a unit's inner "FAIL some case" lines name no unit.
      sed -nE 's/^== ([^ :]+): FAILED .*/compiler \1/p; s/^FAIL ([^ :]+): .*/compiler \1/p' "$log" | sort -u >> "$out/failed" ;;
    wasm) step wasm env COAXIAL_MODE=release apps/openagents-web/build.sh || status=1 ;;
    modules)
      module_cmds | awk -v p="$arg" 'BEGIN { split(p, f, "/") } (NR - 1) % f[2] == f[1] - 1' > "$out/modules"
      step modules build_modules || status=1
      cat "$out"/module-*.log >> "$log" 2> /dev/null ;;
    module)
      module_cmds | grep -F "/build/$arg.wasm\"" > "$out/modules"
      [[ -s "$out/modules" ]] || echo "module $arg: build.sh has no such module at this commit" >> "$log"
      step modules build_modules || status=1
      cat "$out"/module-*.log >> "$log" 2> /dev/null ;;
    reports) reports ;;
    visuals) visuals ;;
    room) room ;;
    tools) reports; room ;;
    *) return 2 ;;
  esac
  return 0
}
case "$mode" in
  branch)
    changed_since
    # Documents alone run nothing (suites --since reads every suite's sources).
    if grep -qvE '(\.md$|^docs/)' "$out/changed"; then suites --since "$arg" || status=1
    else echo "== suites: skipped, the branch changes only documents" >> "$log"; fi
    web_since
    reports_since
    if (( ${#also[@]} > 0 )); then run "${also[@]}" || { echo "check.sh: unknown check ${also[*]}" >&2; exit 2; }; fi ;;
  since)
    part="${3:-1/1}"
    changed_since
    if grep -qvE '(\.md$|^docs/)' "$out/changed"; then suites --since "$arg" --part "$part" || status=1
    else echo "== suites: skipped, the change is only documents" >> "$log"; fi ;;
  web)
    changed_since
    web_since
    reports_since ;;
  room-since)
    changed_since
    if grep -v '\.md$' "$out/changed" | grep -Eq "$web_inputs" && room_touched; then room
    else echo "== room-build: skipped, the change doesn't reach the room" >> "$log"; fi ;;
  *) run "$mode" "$arg" || { sed -n '8,35p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; } ;;
esac
limits; sizes
# tool.sh suites refused the compiler: the host's compiler doesn't match this
# tree in its toolchain (nixpkgs moved under the slot). That says nothing of
# the commit, so the host could not run the check (97): the caller runs it
# again, and host-job.sh rebuilds the compiler first (#13255).
if [[ $status != 0 ]] && grep -q 'was not built from this tree; run coax/build.sh' "$log"; then
  echo 97 > "$out/status"
  echo "check $mode${arg:+ ${arg:0:60}}: the host's compiler doesn't match this tree; not judged (97)"
  exit 97
fi
echo "$status" > "$out/status"
[[ ! -s "$out/warnings.txt" ]] || { echo "warnings:"; sed 's/^/  /' "$out/warnings.txt"; }
[[ ! -s "$out/failed" ]] || { echo "failed:"; sed 's/^/  /' "$out/failed"; }
echo "check $mode${arg:+ ${arg:0:60}}: $([[ $status == 0 ]] && echo passed || echo FAILED)"
exit "$status"
