#!/usr/bin/env bash
# Firm reason: 5. One check of the green-main pipeline (docs/metaverse/green-main-pipeline.md,
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
#   coax/remote/check.sh room-since BASE [PROD]
#                                         the room build, when the change since BASE
#                                         reaches it (a fast run; its own job), and with
#                                         PROD Everglade II's pair against production (e2)
#   coax/remote/check.sh suites I/N       the I-th of N parts of every suite
#   coax/remote/check.sh only PATH,PATH   these suites (a bisect)
#   coax/remote/check.sh compiler I/N     the I-th of N parts of coax/tests/run.sh
#   coax/remote/check.sh unit UNIT        one unit of coax/tests/run.sh (a bisect)
#   coax/remote/check.sh wasm             the web build: every module against its budget
#   coax/remote/check.sh modules I/N      the I-th of N parts of the web build's modules
#                                         (wasm.sh alone, as build.sh runs each)
#   coax/remote/check.sh module NAME      one module of the web build (a bisect)
#   coax/remote/check.sh reports          tools-check, test-remote.sh, test-after.sh, test-vm-deploy.sh, everglade-parity, wire-check, layer-check,
#                                         the native apps' realms (metaverse/native/app-realms.coax, #13389),
#                                         packs paged by cell (metaverse-engine/host/check-pack-cells.coax, #13304),
#                                         the apps' packs admitted (metaverse-engine/tools/pack-seal.coax --check, #13304),
#                                         the API's migration numbers, hashes and schema.sql, the pre-push replay, the committed
#                                         shaders, realm pages and changelog page against
#                                         their sources, and every realm's link-preview card
#   coax/remote/check.sh visuals          check-visuals alone (the web build's first stop:
#                                         a page or the host building visible UI)
#   coax/remote/check.sh room [PROD]      the room build, its WebTransport check and the fuzzers' slice (and with PROD, e2 PROD)
#   coax/remote/check.sh api I/N          the I-th of N parts of the API suite
#                                         (coax/web/api/test.sh --part I/N: its own
#                                         Postgres and API on the host)
#   coax/remote/check.sh api-since BASE I/N
#                                         the same, when the change since BASE reaches
#                                         what the API suite builds or runs (a fast run)
#   coax/remote/check.sh tools [PROD]     reports and room
#   coax/remote/check.sh fuzz [HOURS]     the room's fuzzers for HOURS (4) on OA_FUZZ_WORKERS (9) processes:
#                                         after.sh's nightly run (coax/web/room/fuzz/fuzz.sh long, #13283)
#   coax/remote/check.sh soak [SECONDS [PLAYERS [DAY]]]
#                                         the nightly soak (#13296, docs/deployment/soak.md): the room build, the
#                                         soak's own check (a seeded slow tick and a seeded leak must each fail
#                                         a local run), then PLAYERS (32: staging's shard) protocol bots on staging's rooms for
#                                         SECONDS (7200); the night's record in build/pipeline/capacity.txt
#   coax/remote/check.sh e2 PROD          the room build and Everglade II's pair: this commit
#                                         against production's commit PROD (#13266)
#   coax/remote/check.sh counts BASE      no allowlist count raised since BASE (coax/remote/counts.sh)
#   coax/remote/check.sh coder            Coder's own checks (coax test coder/tui/coder.coax) and its
#                                         offline smoke (coder/tui/smoke.py, about 10 minutes)
#   coax/remote/check.sh coder-since BASE the same, when the change since BASE reaches Coder
#                                         (a fast run; its own job)
#
# It writes build/pipeline/: MODE.log (the output), status, failed (one line
# per failing suite or step), limits.txt (each compile-time limit note:
# "PERCENT CODE RESOURCE USED LIMIT UNIT"), sizes.txt ("MODULE BYTES BUDGET
# PERCENT") and warnings.txt (limits above 90%, modules above 98% of budget).
# Exit 0 when the check passed, 1 when it failed, 97 when the host could not
# judge it (not_judged, below: its compiler doesn't match the tree, or the
# check failed before any suite or unit ran).
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
web_inputs='^(apps/openagents-web/|metaverse-engine/|metaverse/|coax/web/|coax/lib/|coax/src/|coax/platform/|coax/bootstrap/|psionic/|protocol/|everglade/)'
# A change to these rebuilds every module (the compiler, the Wasm toolchain, the build itself).
web_whole='^(coax/src/|coax/bootstrap/|coax/lib/compiler/|coax/platform/|coax/web/wasm\.sh|coax/web/toolchain\.sh|coax/web/wasm-size\.coax|apps/openagents-web/build\.sh)'
# The modules a branch touched, from build.sh's wasm.sh lines: a module is
# touched when the branch changed one of its units' sources (coax deps) or
# its face. Each runs as build.sh runs it; wasm.sh checks its budget.
# build.sh's wasm.sh commands, one a line. project and projects draw from
# tables build.sh writes from Nostr first (openagents project bake), so only
# build.sh (the release) builds them.
# With "live", without the old set (build.sh's `old timed NAME`, #13269: v1 Everglade, Experimental and
# the labs): the default build doesn't make them, so a change's fast run and a branch's check don't
# either. The whole run's `modules I/N` and `module NAME` build them as before.
module_cmds() {   # [live]
  awk '{ if (sub(/\\$/, "")) { buf = buf $0 " "; next } print buf $0; buf = "" }' "$root/apps/openagents-web/build.sh" |
    grep 'coax/web/wasm.sh"' | grep -vE '/build/(project|projects)\.wasm"' |
    { if [[ "${1:-}" == live ]]; then grep -vE '(^|[[:space:]])old[[:space:]]+timed[[:space:]]'; else cat; fi; } |
    # build.sh's step timer (`timed NAME CMD`, #13154), its `old` and its `& made+=($!)` are its own: the command alone.
    sed -E 's/(^|[[:space:]])(old[[:space:]]+)?timed[[:space:]]+[A-Za-z0-9_-]+[[:space:]]+/\1/; s/[[:space:]]+&([[:space:]].*)?$//'
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
# What the API suite (coax/web/api/test.sh) builds and runs: its C and Coax, the
# project runner, the Coax library and platform it compiles with, the limits page it compares.
api_inputs='^(coax/web/api/|coax/web/agent/|coax/lib/|coax/platform/|coax/src/|coax/bootstrap/|apps/openagents-web/docs/limits/)'
api() { step api coax/web/api/test.sh --part "$1" || status=1; }
# Coder (#13239): neither its checks nor its smoke is a suite (tool.sh suites reads only
# apps/openagents-web, verse, aethyr, coax/web and coax/lib), so no check ran them after landing until
# this job: the smoke failed on main for six days (the worker's journal line, 410e68f65b) unseen.
coder() {
  step coder-checks coax/bin/coax test coder/tui/coder.coax || status=1
  step coder-smoke python3 coder/tui/smoke.py || status=1
}
coder_touched() {   # whether the change since $arg reaches Coder (yes when the graph can't tell)
  grep -Eq '^coder/' "$out/changed" && return 0
  affected_files || return 0
  grep -qxF coder/tui/coder.coax "$out/affected"
}
# The room server builds room.coax and the controller's unit (room/build.sh, #13267).
room_units='coax/web/room/room.coax everglade/shared/everglade-step.coax'
room_touched() {   # whether the change since $arg reaches the room build (yes when it can't tell)
  grep -Eq "$web_whole|^coax/web/room/" "$out/changed" && return 0
  affected_files || return 0
  local u; for u in $room_units; do grep -qxF "$u" "$out/affected" && return 0; done
  return 1
}
web_touched() {
  local here="$root/apps/openagents-web" m a cmd words w hit
  m="$here/../../metaverse"; a="$here/../../metaverse-engine"
  : > "$out/modules"
  # The graph (suites --affected) names the touched files; when it can't tell,
  # every module is built.
  affected_files
  case $? in
    0) ;;
    3) module_cmds live > "$out/modules"; build_modules; return ;;
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
  done < <(module_cmds live)
  build_modules
}
build_modules() {   # each command in $out/modules, as build.sh runs it
  local here="$root/apps/openagents-web" m a cmd
  m="$here/../../metaverse"; a="$here/../../metaverse-engine"
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
  step after-gates coax/remote/test-after.sh || status=1
  step release-rules coax/web/room/test-vm-deploy.sh || status=1   # a promotion pins its build; a release holds staging (#13398)
  step everglade-parity coax/web/tool.sh everglade-parity || status=1
  # Splice's page is built with its content tables' own bytes (#13404)
  step splice-content coax/web/tool.sh metaverse/splice/tools/content-check.coax metaverse/splice/content/splice.content --check || status=1
  # Everglade's tables hold their rules, and the kept tables of the release before are the revision before them (#13408)
  step everglade-content coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content || status=1
  step wire-check coax/web/tool.sh wire-check || status=1
  step layer-check coax/web/tool.sh layer-check || status=1
  migrations
  step prepush-replay coax/web/tool.sh prepush --replay-check || status=1
  step wgsl coax/web/tool.sh coax/web/wgsl-module.coax --site --check || status=1
  visuals
  generated
  pack_reader_c
  packs_admitted
  frame_plan_h
  frame_plan_walk
  app_realms
  pack_cells
  cards
}
# The API's migrations (#13301, coax/web/api/migrations/README.md): a new one takes a number above
# BASE's and no other file's, one that landed never changes (migrations.sha256), and schema.sql is
# the schema a fresh database has after all of them (a throwaway Postgres on the host).
migrations() {   # [BASE]
  step migration-check coax/web/tool.sh coax/web/api/migration-check.coax ${1:+--base "$1"} || status=1
  step migration-schema coax/web/tool.sh coax/web/api/migration-check.coax --schema || status=1
}
# build.sh stops on it before any module, and no module job runs build.sh.
visuals() { step check-visuals coax/web/check-visuals.sh apps/openagents-web || status=1; }
# The other committed files the web build writes (#13117): each realm's page
# from realms.json, and the changelog page from coax/CHANGELOG.md.
generated() {
  step realm-pages bun coax/web/realms.ts --check || status=1
  step changelog-page coax/web/tool.sh coax/web/changelog-page.coax --check || status=1
}
# metaverse/native/pack-table-gen.h is the C the compiler writes for the one pack reader's face
# (#13304): the native realm host includes it, since most of the builds that compile
# metaverse/native/realm-host.c take no Coax step. Committed, so it is compared with its source here.
pack_reader_c() {
  step pack-reader-c bash -c 'coax/bin/coax c metaverse-engine/host/pack-table-face.coax --library | cmp -s - metaverse/native/pack-table-gen.h || { echo "metaverse/native/pack-table-gen.h is not what the compiler writes for metaverse-engine/host/pack-table-face.coax. From the repository root: coax/bin/coax c metaverse-engine/host/pack-table-face.coax --library > metaverse/native/pack-table-gen.h, then commit it."; exit 1; }' || status=1
}
# The packs the Mac and iPhone apps ship are packs the admission takes as they are (#13304): Everglade
# II's, and Experimental's once inflated. A pack tool that leaves its working form fails here; the
# last step of a pack build is metaverse-engine/tools/pack-seal.coax.
packs_admitted() {
  step packs-admitted bash -c 'set -e; t="$(mktemp -d)"; gzip -dc apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz > "$t/glade.pack"; r=0; coax/web/tool.sh metaverse-engine/tools/pack-seal.coax --check apps/openagents-web/realms/everglade2/assets/everglade2.pack "$t/glade.pack" || r=1; rm -f "$t/glade.pack"; rmdir "$t"; exit $r' || status=1
}
# The frame plan's numbers for C hosts (#13303): metaverse-engine/host/frame-plan.h is what metaverse-engine/render/frame-plan.coax
# writes. Committed, because a host's build takes no Coax step for it; compared with its source here.
frame_plan_h() {
  step frame-plan-h bash -c 'coax/web/tool.sh metaverse-engine/tools/frame-plan-header.coax | cmp -s - metaverse-engine/host/frame-plan.h || { echo "metaverse-engine/host/frame-plan.h is not what metaverse-engine/render/frame-plan.coax writes; run: coax/web/tool.sh metaverse-engine/tools/frame-plan-header.coax > metaverse-engine/host/frame-plan.h"; exit 1; }' || status=1
}
# The frame plan's walk for C hosts (#13303): metaverse-engine/host/core/frame-plan-walk.h calls what Coax says a
# walk of each plan must call, and refuses a malformed plan with no call.
frame_plan_walk() { step frame-plan-walk metaverse-engine/host/core/check-frame-plan-walk.sh || status=1; }
# The Mac and iPhone apps' realms and passes still resolve, emit and compile (#13389): a client's
# Coax moved (#13268) and both apps stopped building with no check saying so. Their Swift and
# Objective-C need a Mac (apps/shared/host/apple/check.sh); this is the part any host can do.
# A pack paged by world cell (#13304, for #13280): the browser's pack module fetches a cell's pages
# on demand exactly as the one reader says (metaverse-engine/host/check-pack-cells.coax; it builds the pack
# reactor and runs coax/web/pack-host.js in bun over a fixture it writes).
pack_cells() { step pack-cells coax/web/tool.sh metaverse-engine/host/check-pack-cells.coax "$PWD" coax/web/build/pack-cells || status=1; }
app_realms() { step app-realms coax/web/tool.sh metaverse/native/app-realms.coax "$PWD" coax/web/build/app-realms || status=1; }
# Every realm in realms.json has its link-preview card (#13144): a release refuses a realm without
# one, so it shows at landing, not at release.
cards() { step realm-cards bun coax/web/og.ts --cards apps/openagents-web || status=1; }
# and the WebTransport stack's check (#13175): wt-echo against a headless Chrome, where there is one;
# the encounter table (#13288): every class and level's time to kill in the room's rules against the
# pinned docs/dnd/encounter-ttk.md, failing on a row moved more than its share (the log names each);
# with PROD, Everglade II's pair against production's (e2_gate)
room() {
  step room-build coax/web/room/build.sh || { status=1; return; }
  step room-webtransport coax/web/room/wt-check.sh 9441 || status=1
  step encounter-ttk coax/web/tool.sh coax/web/room/encounter-sim.coax --check docs/dnd/encounter-ttk.md || status=1
  step bot-bars coax/web/tool.sh coax/web/room/encounter-sim.coax --check-bars coax/web/room/bot-bars.coax || status=1   # the bots' bars are the classes' (#13296)
  # the zone's cells (#13280, #13281): the committed manifest is what the land builds now, every spawn and named point
  # can be walked to, and a creature walks the named points' routes on the navigation cells
  step zone-cells coax/web/tool.sh everglade/model/tools/zone-cells.coax --check everglade/world/zone-cells.coax everglade/content/everglade.content || status=1
  # The fuzzers' slice is off by default (owner, 2026-11-12: no fuzzing for now, #13283 closed); OA_FUZZ_SLICE=60 turns it on.
  if [[ -n "${OA_FUZZ_SLICE:-}" ]]; then step room-fuzz coax/web/room/fuzz/fuzz.sh slice "$OA_FUZZ_SLICE" || status=1; fi
  [[ -z "${1:-}" ]] || e2_gate "$1" || status=1
}
# Everglade II against production (#13266, docs/metaverse/green-main-pipeline.md "Everglade II against
# production"): two bots (load.coax everglade2: the real socket protocol, a hello, inputs at 30 Hz, a
# cast every 5 s, pings) in one Everglade II room of a local room server, for OA_E2_SECONDS (10),
# OA_E2_ROUNDS (3) times on this commit's room and production's commit PROD's, in turn on this host.
# Production's room and client are built once per host in ~/oa-e2/PROD from its own tree. Each side's
# medians; this commit is worse than production when:
#   it fails the load's own checks in most rounds and production holds in most (both failing: a report)
#   the room's tick p95 is over production's by more than OA_E2_TICK_US (300 µs) and half of it
#   the input-to-ACK p95 is over production's by more than OA_E2_ACK_MS (25 ms)
#   the bytes a player hears a second are over production's by more than OA_E2_BYTES_PCT (25%)
#   production's own bots are refused or fight nothing on this commit's room (e2_prev, "e2 prev-wire")
#   production's pins or content tables are not ones this commit's room seats (e2_pins, "e2 prev-pins")
# The margins come from the noise measured on 2026-11-11 (the doc). Each failing measure is a line
# "e2 NAME" in failed; e2.tsv has every round's numbers.
e2_gate() {   # prod
  local prod="$1" w="$out/e2" pt key="local-development-room-ticket-key-not-a-secret"
  local rounds="${OA_E2_ROUNDS:-3}" secs="${OA_E2_SECONDS:-10}" players="${OA_E2_PLAYERS:-2}" r side t0
  mkdir -p "$w"; t0=$(date +%s)
  echo "== e2-pair against production ${prod:0:10}" >> "$log"
  git rev-parse -q --verify "$prod^{commit}" > /dev/null 2>&1 || git -C "$HOME/oa" fetch -q origin main 2> /dev/null
  git rev-parse -q --verify "$prod^{commit}" > /dev/null 2>&1 || { echo "e2-pair: production's commit $prod is not here; not judged" >> "$log"; return 0; }
  [[ -x coax/web/room/bin/room ]] || { echo "e2 room" >> "$out/failed"; return 1; }
  pt="$HOME/oa-e2/${prod:0:12}"
  # Production's room, its client and its compiler, once a host (the clone's lock: one checkout at a time).
  (
    flock 9
    [[ -x "$pt/room" ]] && exit 0
    rm -rf "$pt"; mkdir -p "$HOME/oa-e2"
    git -C "$HOME/oa" worktree prune 2> /dev/null
    flock "$HOME/oa/.oa-remote.lock" git -C "$HOME/oa" worktree add -q --detach -f "$pt/tree" "$prod" || exit 1
    # its compiler in the host's release toolchain, as host-job.sh builds a slot's
    local tc=()
    command -v nix > /dev/null && tc=(nix shell nixpkgs#pkgsStatic.stdenv.cc nixpkgs#python3 nixpkgs#gnumake --command
      env CC="$(uname -m)-unknown-linux-musl-gcc" CXX="$(uname -m)-unknown-linux-musl-g++" COAX_LDFLAGS=-static)
    (cd "$pt/tree" && ${tc[@]+"${tc[@]}"} coax/build.sh > "$pt/compiler.log" 2>&1) || exit 1
    (cd "$pt/tree" && coax/web/room/build.sh "$pt/room.new" > "$pt/build.log" 2>&1) || exit 1
    mv "$pt/room.new" "$pt/room"
    # The two newest productions stay.
    local old
    for old in $(ls -td "$HOME"/oa-e2/*/ 2> /dev/null | tail -n +3); do
      git -C "$HOME/oa" worktree remove --force "$old/tree" 2> /dev/null; rm -rf "$old"
    done
  ) 9> "$HOME/oa-e2.lock" >> "$log" 2>&1 || { echo "e2-pair: production's room did not build on this host; not judged" >> "$log"; return 0; }
  e2_one() {   # side room tree round: one load run on its own port; its numbers on one line of e2.tsv
    local side="$1" bin="$2" tree="$3" r="$4" port pid n
    # a port nothing answers on (another slot's room may be on one)
    for n in 1 2 3 4 5 6 7 8; do port=$(( 20000 + RANDOM % 20000 )); curl -s -m 1 -o /dev/null "http://127.0.0.1:$port/" 2> /dev/null || break; done
    OA_ROOM_TICKET_KEY="$key" "$bin" --port "$port" --dev "$tree/coax/web/room/dev" > "$w/$side-$r.room.log" 2>&1 & pid=$!
    for n in $(seq 1 100); do curl -fs "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break; read -r -t 0.1 < /dev/zero || true; done
    (cd "$tree" && OA_ROOM_TICKET_KEY="$key" coax/web/tool.sh coax/web/room/load.coax everglade2 "ws://localhost:$port" "$players" "$secs") > "$w/$side-$r.load" 2>&1
    curl -fs "http://127.0.0.1:$port/health" > "$w/$side-$r.health" 2>&1
    kill "$pid" 2> /dev/null; wait "$pid" 2> /dev/null
    # side round holds ack_p95 bytes tick_p95
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$side" "$r" "$(tail -1 "$w/$side-$r.load" | grep -qx holds && echo 1 || echo 0)" \
      "$(grep -o '"ack_p95_ms":-\{0,1\}[0-9]*' "$w/$side-$r.load" | head -1 | cut -d: -f2)" \
      "$(grep -o '"bytes_per_s":[0-9]*' "$w/$side-$r.load" | head -1 | cut -d: -f2)" \
      "$(sed -nE 's/.*everglade tick p50 [0-9]+ us at the 95th ([0-9]+) us.*/\1/p' "$w/$side-$r.health" | head -1)" >> "$w/e2.tsv"
  }
  : > "$w/e2.tsv"
  for (( r = 1; r <= rounds; r++ )); do
    e2_one head coax/web/room/bin/room "$root" "$r"
    e2_one prod "$pt/room" "$pt/tree" "$r"
  done
  e2_prev "$pt" "$key" || true
  e2_pins "$prod" || true
  cp "$w/e2.tsv" "$out/e2.tsv"
  sed 's/^/  /' "$w/e2.tsv" >> "$log"
  awk -F'\t' -v tick="${OA_E2_TICK_US:-300}" -v ack="${OA_E2_ACK_MS:-25}" -v pct="${OA_E2_BYTES_PCT:-25}" -v prod="${prod:0:10}" '
    function med(s, k,   n, a, i, j, t) { n = split(s, a, " "); for (i = 1; i <= n; i++) for (j = i + 1; j <= n; j++) if (a[j] + 0 < a[i] + 0) { t = a[i]; a[i] = a[j]; a[j] = t }; return n ? a[int((n + 1) / 2)] + 0 : -1 }
    { holds[$1] += $3; runs[$1]++; if ($4 != "" && $4 >= 0) a[$1] = a[$1] " " $4; if ($5 != "") b[$1] = b[$1] " " $5; if ($6 != "") t[$1] = t[$1] " " $6 }
    END {
      hh = holds["head"] * 2 > runs["head"]; ph = holds["prod"] * 2 > runs["prod"]
      printf "e2 holds: this commit %d of %d rounds, production %s %d of %d\n", holds["head"], runs["head"], prod, holds["prod"], runs["prod"] > "/dev/stderr"
      if (!hh && ph) print "e2 holds"
      else if (!hh) print "e2-pair: both fail the load'"'"'s checks: a report, not a regression" > "/dev/stderr"
      ht = med(t["head"]); pt = med(t["prod"]); m = tick; if (pt / 2 > m) m = pt / 2
      printf "e2 tick-p95: this commit %d us, production %d us (margin %d)\n", ht, pt, m > "/dev/stderr"
      if (ht >= 0 && pt >= 0 && ht > pt + m) print "e2 tick-p95"
      ha = med(a["head"]); pa = med(a["prod"])
      printf "e2 ack-p95: this commit %d ms, production %d ms (margin %d)\n", ha, pa, ack > "/dev/stderr"
      if (ha >= 0 && pa >= 0 && ha > pa + ack) print "e2 ack-p95"
      hb = med(b["head"]); pb = med(b["prod"])
      printf "e2 bytes: this commit %d B/s, production %d B/s (margin %d%%)\n", hb, pb, pct > "/dev/stderr"
      if (hb >= 0 && pb > 0 && hb * 100 > pb * (100 + pct)) print "e2 bytes"
    }' "$w/e2.tsv" > "$w/worse" 2>> "$log"
  [[ ! -s "$w/prev.worse" ]] || cat "$w/prev.worse" >> "$w/worse"
  echo "== e2-pair: $( [[ -s "$w/worse" ]] && echo "WORSE than production: $(paste -sd, "$w/worse")" || echo "not worse than production"), $(( $(date +%s) - t0 )) s" >> "$log"
  [[ ! -s "$w/worse" ]] || { cat "$w/worse" >> "$out/failed"; return 1; }
}
# The previous wire (#13398, everglade/server/wire-prev.coax): production's own protocol bots, run from
# production's tree and so speaking production's wire with production's readers, play this commit's
# room for OA_E2_PREV_SECONDS (150), and production's room the same way beside it. A release moves its
# rooms before its page, so this commit's room must serve production's client: "e2 prev-wire" when the
# room refuses a bot for its version, seats fewer bots than production's room did, or takes no cast,
# tells no monster's blow or fells no monster where production's room did (the bots aim at the monsters
# they are told of: none told, none fought). A wire raised without the version before it kept fails here.
e2_prev() {   # pt key
  local pt="$1" key="$2" secs="${OA_E2_PREV_SECONDS:-150}" players="${OA_E2_PREV_PLAYERS:-4}" side bin port pid n
  : > "$w/prev.worse"
  [[ -f "$pt/tree/coax/web/room/bots.coax" ]] || { echo "e2 prev-wire: production's tree has no protocol bots; not judged" >> "$log"; return 0; }
  for side in head prod; do
    bin="coax/web/room/bin/room"; [[ "$side" == head ]] || bin="$pt/room"
    for n in 1 2 3 4 5 6 7 8; do port=$(( 20000 + RANDOM % 20000 )); curl -s -m 1 -o /dev/null "http://127.0.0.1:$port/" 2> /dev/null || break; done
    OA_ROOM_TICKET_KEY="$key" "$bin" --port "$port" --dev "$( [[ "$side" == head ]] && echo "$root" || echo "$pt/tree" )/coax/web/room/dev" > "$w/prev-$side.room.log" 2>&1 & pid=$!
    for n in $(seq 1 100); do curl -fs "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break; read -r -t 0.1 < /dev/zero || true; done
    (cd "$pt/tree" && OA_ROOM_TICKET_KEY="$key" coax/web/tool.sh coax/web/room/bots.coax run "ws://localhost:$port" "$players" "$secs" --direct --realm pair-e2-prev --reconnect 0 --travel 0) > "$w/prev-$side.bots" 2>&1
    curl -fs "http://127.0.0.1:$port/vitals" > "$w/prev-$side.vitals" 2>&1 || curl -fs "http://127.0.0.1:$port/health" > "$w/prev-$side.vitals" 2>&1
    kill "$pid" 2> /dev/null; wait "$pid" 2> /dev/null
  done
  e2_num() { grep '^bots {' "$w/prev-$1.bots" | tail -1 | grep -o "\"$2\":-\{0,1\}[0-9]*" | head -1 | cut -d: -f2; }
  e2_blows() { grep '^F ' "$w/prev-$1.bots" | grep -o '"blows":[0-9]*' | cut -d: -f2 | awk '{ n += $1 } END { print n + 0 }'; }
  local hs hv ht hk ps pk ptk hb pb
  hb="$(e2_blows head)"; pb="$(e2_blows prod)"
  hs="$(e2_num head seated)"; hv="$(e2_num head version_refused)"; ht="$(e2_num head taken)"; hk="$(e2_num head kills)"
  ps="$(e2_num prod seated)"; ptk="$(e2_num prod taken)"; pk="$(e2_num prod kills)"
  echo "e2 prev-wire: production's bots on this commit's room: seated ${hs:-none} refused-for-version ${hv:-none} casts-taken ${ht:-none} monsters' blows heard $hb kills ${hk:-none}; on production's room: seated ${ps:-none} casts-taken ${ptk:-none} monsters' blows heard $pb kills ${pk:-none}" >> "$log"
  [[ -n "$ps" && "$ps" -gt 0 ]] || { echo "e2 prev-wire: production's bots did not sit on production's own room on this host; not judged" >> "$log"; return 0; }
  if [[ -z "$hs" || "${hv:-0}" -gt 0 || "$hs" -lt "$ps" ]] || [[ "${ptk:-0}" -gt 0 && "${ht:-0}" -eq 0 ]] || [[ "${pk:-0}" -gt 0 && "${hk:-0}" -eq 0 ]] || [[ "$pb" -gt 0 && "$hb" -eq 0 ]]; then
    echo "e2 prev-wire" > "$w/prev.worse"
    echo "e2 prev-wire: this commit's room does not serve production's client (its wire or what it reads). See everglade/server/wire-prev.coax" >> "$log"
  fi
}
# The previous pins (#13408, everglade/shared/pins-prev.coax; docs/protocol/everglade-previous-pins.md):
# production's page says its engine, its rules and its content pin, and this commit's room must seat
# them: they are this build's own or the release before's that it names, over the same content tables.
# "e2 prev-pins" when coax/web/room/pins-pair.coax says it does not: a pin raised twice in one release,
# or without the value production runs kept. e2_prev's bots then show the same room refusing them.
e2_pins() {   # prod
  local prod="$1" tables="-" rc
  git show "$prod:everglade/shared/everglade-wire.coax" > "$w/prod-wire.coax" 2> /dev/null || { echo "e2 prev-pins: production's wire file could not be read; not judged" >> "$log"; return 0; }
  ! git show "$prod:everglade/content/everglade.content" > "$w/prod.content" 2> /dev/null || tables="$w/prod.content"
  coax/web/tool.sh coax/web/room/pins-pair.coax "$w/prod-wire.coax" "$tables" "${prod:0:10}" > "$w/pins.out" 2>&1; rc=$?
  sed 's/^/e2 prev-pins: /' "$w/pins.out" >> "$log"
  if [[ "$rc" -eq 1 ]]; then echo "e2 prev-pins" >> "$w/prev.worse"; elif [[ "$rc" -ne 0 ]]; then echo "e2 prev-pins: not judged" >> "$log"; fi
}
# The allowlist counts (coax/remote/counts.sh, #13266): none that a commit since BASE raised. Each
# raised count is a line "count NAME" in failed.
counts_since() {
  echo "== counts since ${1:0:10}" >> "$log"
  local raised; raised="$(coax/remote/counts.sh check "$1" HEAD 2>> "$log")"
  coax/remote/counts.sh show HEAD | sed 's/^/  /' >> "$log"
  [[ -n "$raised" ]] || return 0
  echo "$raised" >> "$log"; awk '{ print "count " $2 }' <<< "$raised" >> "$out/failed"; status=1
}
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
  if grep -q '^coax/remote/' "$out/changed"; then
    step remote-cleanup coax/remote/test-remote.sh || status=1
    # after.sh's comparing gates in a scratch repository (#13266)
    step after-gates coax/remote/test-after.sh || status=1
  fi
  # the release rules against a stand-in for the room VM (#13398)
  if grep -qE '^coax/web/(room/(vm-deploy|test-vm-deploy)\.sh|api/deploy\.sh|release-set)' "$out/changed"; then
    step release-rules coax/web/room/test-vm-deploy.sh || status=1
  fi
  if grep -qv '\.md$' "$out/changed"; then
    step tools-check coax/web/tool.sh tools-check || status=1
    step everglade-parity coax/web/tool.sh everglade-parity || status=1
    step splice-content coax/web/tool.sh metaverse/splice/tools/content-check.coax metaverse/splice/content/splice.content --check || status=1
    step everglade-content coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content || status=1   # and the kept tables before (#13408)
    # every wire shape registered (proto/registry.json, #13236); the allowlist against the base's copy
    step wire-check coax/web/tool.sh wire-check --base "$arg" || status=1
    # no load up the layers (metaverse-engine/README.md "Layers", #13264); the allowlist against the base's copy
    step layer-check coax/web/tool.sh layer-check --base "$arg" || status=1
  fi
  if grep -Eq '^coax/web/(api/(migrations/|migrations\.sha256|schema\.sql|migration-check\.coax)|tool\.sh)' "$out/changed"; then migrations "$arg"; fi
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
  # Its sources: the reader, its face and the compiler that writes the C.
  if grep -Eq '^(metaverse-engine/host/pack-table|metaverse/native/pack-table-gen\.h|coax/(src|lib/compiler|bootstrap)/|coax/VERSION)' "$out/changed"; then pack_reader_c; fi
  if grep -Eq '^(metaverse-engine/render/(frame-plan|passes|types)\.coax|metaverse-engine/host/frame-plan\.h|metaverse-engine/tools/frame-plan-header\.coax)' "$out/changed"; then frame_plan_h; fi
  if grep -Eq '^(metaverse-engine/render/(frame-plan|passes|types)\.coax|metaverse-engine/host/(frame-plan\.h|core/)|metaverse-engine/tools/frame-plan-fixture\.coax)' "$out/changed"; then frame_plan_walk; fi
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
    room) room "$arg" ;;
    fuzz) step fuzz coax/web/room/fuzz/fuzz.sh long "${arg:-4}" "${OA_FUZZ_WORKERS:-9}" || status=1 ;;
    soak)   # also: PLAYERS DAY. The self-check first: a soak that can't fail is no check.
        step room-build coax/web/room/build.sh || { status=1; return 0; }
        step soak-self-check coax/web/tool.sh coax/web/room/soak.coax self-check coax/web/room/bin/room $(( 21000 + RANDOM % 9000 )) || { status=1; return 0; }
        step soak coax/web/tool.sh coax/web/room/soak.coax run --origin "${OA_SOAK_ORIGIN:-https://staging.openagents.com}" \
          --vitals "${OA_SOAK_VITALS:-https://room-staging.openagents.com/api/room/s1/vitals}" --env "${OA_SOAK_ENV:-staging}" \
          --players "${also[0]:-32}" --seconds "${arg:-7200}" ${also[1]:+--day "${also[1]}"} --commit "$(git rev-parse HEAD)" --out "$out/capacity.txt" || status=1 ;;
    api) api "${arg:-1/1}" ;;
    tools) reports; room "$arg" ;;
    e2) [[ -n "$arg" ]] || return 2
        step room-build coax/web/room/build.sh || { status=1; return 0; }
        e2_gate "$arg" || status=1 ;;
    counts) [[ -n "$arg" ]] || return 2
        counts_since "$arg" ;;
    coder) coder ;;
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
  api-since)
    changed_since
    if grep -v '\.md$' "$out/changed" | grep -Eq "$api_inputs"; then api "${3:-1/1}"
    else echo "== api: skipped, the change doesn't reach the API suite" >> "$log"; fi ;;
  coder-since)
    changed_since
    if grep -qvE '(\.md$|^docs/)' "$out/changed" && coder_touched; then coder
    else echo "== coder: skipped, the change doesn't reach Coder" >> "$log"; fi ;;
  room-since)
    changed_since
    if grep -v '\.md$' "$out/changed" | grep -Eq "$web_inputs" && room_touched; then room "${also[0]:-}"
    else echo "== room-build: skipped, the change doesn't reach the room" >> "$log"; fi ;;
  *) run "$mode" "$arg" || { sed -n '8,45p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; } ;;
esac
limits; sizes
# A failure that says nothing of the commit: the host could not run the check
# (97), and the caller runs it again, with the slot's compiler installed again
# first, instead of bisecting it (#13255, #13256). Each failing item has to be
# one of these; one real failure beside them keeps the check judged.
#   - tool.sh suites refused the compiler: it doesn't match this tree in the
#     host's toolchain (nixpkgs moved under the slot, 2026-11-11)
#   - a suite whose compiler did not start
#   - the suites step failed with no suite failing for itself, or the
#     compiler's units with no unit failing (an empty log, a full or broken
#     host, a harness that stopped before any suite or unit ran)
#   - nothing failed by name: the check stopped outside any step
not_judged() {
  [[ $status != 0 ]] || return 1
  [[ -s "$log" ]] || { why="its log is empty"; return 0; }
  if grep -q 'was not built from this tree; run coax/build.sh' "$log"; then why="the host's compiler doesn't match this tree"; return 0; fi
  [[ -s "$out/failed" ]] || { why="it stopped outside any step"; return 0; }
  local item
  while IFS= read -r item; do
    case "$item" in
      # judged when a suite failed for itself, or a unit failed
      "step suites") grep -E '^FAIL ' "$log" | grep -qv ': the compiler did not start$' && return 1 ;;
      "step compiler") grep -Eq '^== [^ ]+: FAILED' "$log" && return 1 ;;
      suite\ *) grep -qxF "FAIL ${item#suite }: the compiler did not start" "$log" || return 1 ;;
      *) return 1 ;;
    esac
  done < "$out/failed"
  why="no suite or unit failed for itself"
  return 0
}
why=""
if not_judged; then
  echo 97 > "$out/status"
  echo "check $mode${arg:+ ${arg:0:60}}: not judged (97): $why"
  exit 97
fi
echo "$status" > "$out/status"
[[ ! -s "$out/warnings.txt" ]] || { echo "warnings:"; sed 's/^/  /' "$out/warnings.txt"; }
[[ ! -s "$out/failed" ]] || { echo "failed:"; sed 's/^/  /' "$out/failed"; }
echo "check $mode${arg:+ ${arg:0:60}}: $([[ $status == 0 ]] && echo passed || echo FAILED)"
exit "$status"
