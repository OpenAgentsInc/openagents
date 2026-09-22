#!/usr/bin/env bash
# Run the local Rust gate.
#
# A bare run is the full gate. Scope it for iteration — never to claim a
# full pass:
#
#   --phases a,b     run only these slugs (--list prints them)
#   --crates a,b     narrow cargo phases to these packages
#   --changed[=REF]  derive the crate set from paths changed since REF
#                    (default origin/main), including uncommitted edits
#   --keep-going     run later phases after one fails; the record names all
#   --print          show the resolved phases and commands without running
#   --record-dir D   run records (default .coder/verification); --no-record
#   --no-retry       do not retry a phase whose log shows resource
#                    exhaustion (fd or address pressure) once
#
# Existing coverage flags: --skip-postgres, --with-metal, --with-soak.
set -euo pipefail
cd "$(dirname "$0")/.."

ALL_SLUGS="preflight gate-tooling artifacts delegation backup fmt clippy clippy-features tests tests-features deps postgres metal-clippy metal-tests soak"
CARGO_SCOPED="clippy clippy-features tests tests-features"
features='kev/serve,lev/serve,gym/tui,jev/blocking,oak/mcp-http'

postgres=1
metal=0
soak=0
keep_going=0
print_only=0
record=1
retry=1
record_dir=".coder/verification"
WANTED=""
CRATES=""
CRATES_SET=0
WORKSPACE_SCOPE=0
CHANGED=""
UNCOVERED=""

usage() {
  cat <<'EOF' >&2
Usage: verify-rust.sh [--list] [--print]
       [--phases a,b] [--crates a,b | --changed[=REF]]
       [--keep-going] [--no-retry] [--record-dir DIR | --no-record]
       [--skip-postgres] [--with-metal] [--with-soak]
EOF
  exit "${1:-64}"
}

list_phases() {
  cat <<'EOF'
preflight        Environment preflight: descriptors, tools, toolchains, disk
gate-tooling     Gate tooling tests (verify-changed, gate-record)
artifacts        Artifact acquisition tests
delegation       Delegation evidence checks
backup           Backup collection tests
fmt              Workspace formatting
clippy           Default workspace Clippy
clippy-features  Feature workspace Clippy
tests            Default workspace tests
tests-features   Feature workspace tests
deps             Dependency policy
postgres         PostgreSQL acceptance
metal-clippy     Metal Clippy (with --with-metal)
metal-tests      Metal feature tests (with --with-metal)
soak             Relay soak (with --with-soak)
EOF
}

while (( $# )); do
  case "$1" in
    --skip-postgres) postgres=0 ;;
    --with-metal) metal=1 ;;
    --with-soak) soak=1 ;;
    --keep-going) keep_going=1 ;;
    --print) print_only=1 ;;
    --no-record) record=0 ;;
    --no-retry) retry=0 ;;
    --list) list_phases; exit 0 ;;
    --help|-h) usage 0 ;;
    --phases) shift; [[ $# -gt 0 ]] || usage; WANTED="${1//,/ }" ;;
    --phases=*) WANTED="${1#--phases=}"; WANTED="${WANTED//,/ }" ;;
    --crates) shift; [[ $# -gt 0 ]] || usage; CRATES="${1//,/ }"; CRATES_SET=1 ;;
    --crates=*) CRATES="${1#--crates=}"; CRATES="${CRATES//,/ }"; CRATES_SET=1 ;;
    --changed) CHANGED="origin/main" ;;
    --changed=*) CHANGED="${1#--changed=}" ;;
    --record-dir) shift; [[ $# -gt 0 ]] || usage; record_dir=$1 ;;
    --record-dir=*) record_dir="${1#--record-dir=}" ;;
    *) usage ;;
  esac
  shift
done

for slug in $WANTED; do
  case " $ALL_SLUGS " in
    *" $slug "*) ;;
    *) echo "unknown phase slug: $slug" >&2; list_phases >&2; exit 64 ;;
  esac
done

# A low descriptor limit is the most common environmental failure in the
# worktree fan-out tests, and raising the soft limit needs no authority —
# it may go as high as the hard limit. Do it here, in the shell the phases
# inherit, before preflight verifies what it found.
fds_now=$(ulimit -n)
if (( fds_now < 8192 )); then
  ulimit -n 8192 2>/dev/null || true
fi

phase_wanted() {
  [[ -z $WANTED || " $WANTED " == *" $1 "* ]]
}

phase_scoped() {
  case " $CARGO_SCOPED " in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

if [[ -n $CHANGED ]]; then
  if ! git rev-parse --verify "$CHANGED" >/dev/null 2>&1; then
    echo "--changed: cannot resolve ref '$CHANGED'; pass --changed=REF" >&2
    exit 64
  fi
  scope_json=$(python3 scripts/verify-changed.py --ref "$CHANGED")
  CRATES=$(python3 -c 'import json,sys; print(" ".join(json.load(sys.stdin)["crates"]))' <<<"$scope_json")
  WORKSPACE_SCOPE=$(python3 -c 'import json,sys; print(1 if json.load(sys.stdin)["workspace"] else 0)' <<<"$scope_json")
  UNCOVERED=$(python3 -c 'import json,sys; print(" ".join(json.load(sys.stdin)["uncovered"]))' <<<"$scope_json")
  CRATES_SET=1
  if (( WORKSPACE_SCOPE )); then
    echo "Changed paths touch workspace-wide files; cargo phases cover the whole workspace."
  fi
fi

# The package selection cargo phases run under: empty means the workspace.
# Built directly rather than through a function so the script still runs on
# the bash 3.2 macOS ships.
scope=()
if (( CRATES_SET )) && (( ! WORKSPACE_SCOPE )) && [[ -n $CRATES ]]; then
  for c in $CRATES; do scope+=(-p "$c"); done
fi

# Feature flags name packages; under a crate scope only the features whose
# package was selected may be enabled, or cargo refuses the whole command.
feature_scope() {
  local wanted_features=$1 kept="" f pkg
  if (( ! CRATES_SET )) || (( WORKSPACE_SCOPE )); then
    printf '%s' "$wanted_features"
    return
  fi
  for f in ${wanted_features//,/ }; do
    pkg=${f%%/*}
    case " $CRATES " in
      *" $pkg "*) kept+="${kept:+,}$f" ;;
    esac
  done
  printf '%s' "$kept"
}
feature_args=()
feature_selected=$(feature_scope "$features")
if [[ -n $feature_selected ]]; then
  feature_args=(--features "$feature_selected")
fi

preflight_checks() {
  local ok=0
  local fds
  fds=$(ulimit -n)
  if (( fds < 2048 )); then
    echo "FAIL: file descriptor limit is $fds and could not be raised; worktree fan-out tests need at least 2048."
    echo "      Raise it for this run:  ulimit -n 8192"
    ok=1
  else
    echo "ok: file descriptor limit $fds"
  fi
  local tool
  for tool in cargo python3 git rustup; do
    if command -v "$tool" >/dev/null 2>&1; then
      echo "ok: $tool present"
    else
      echo "FAIL: $tool is not on PATH"
      ok=1
    fi
  done
  local free_kib
  free_kib=$(df -k . | awk 'NR==2 {print $4}')
  if (( free_kib < 5242880 )); then
    echo "warn: $((free_kib / 1024)) MiB free; a cold target directory needs several GiB"
  else
    echo "ok: $((free_kib / 1024 / 1024)) GiB free"
  fi
  return $ok
}

# A phase failure whose log shows resource exhaustion is environmental,
# not evidence against the change: fd pressure (24), address reuse (48),
# and EAGAIN (11/35) retry once, with the retry recorded.
environmental() {
  local log=$1
  [[ -f $log ]] && grep -Eq 'Too many open files|os error 24|os error 35|os error 11|os error 48|os error 98|Address already in use|Resource temporarily unavailable' "$log"
}

RUN_ID=""
RESULT=""
declare -a FAILED_PHASES=()

record_call() {
  [[ -n $RUN_ID ]] || return 0
  python3 scripts/gate-record.py "$@"
}

finish_record() {
  local result=${RESULT:-aborted}
  [[ -n $RUN_ID ]] || return 0
  python3 scripts/gate-record.py finish --dir "$record_dir" --run-id "$RUN_ID" --result "$result" >/dev/null 2>&1 || true
}
trap finish_record EXIT

record_phase() {
  local slug=$1 name=$2 status=$3 exit_code=$4 elapsed=$5 attempts=$6 log=$7
  shift 7
  local cmd_json
  cmd_json=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "$@")
  local -a rec=(phase --dir "$record_dir" --run-id "$RUN_ID"
    --slug "$slug" --name "$name" --status "$status"
    --attempts "$attempts" --command "$cmd_json")
  [[ -n $exit_code ]] && rec+=(--exit "$exit_code")
  [[ -n $elapsed ]] && rec+=(--elapsed "$elapsed")
  [[ -n $log && -f $log ]] && rec+=(--log "$log")
  record_call "${rec[@]}"
}

record_skip() {
  record_call skip --dir "$record_dir" --run-id "$RUN_ID" --slug "$1" --reason "$2"
  [[ $2 == "not in --phases" ]] || echo "SKIPPED: $2"
}

run_phase() {
  local slug=$1 name=$2
  shift 2
  if ! phase_wanted "$slug"; then
    record_skip "$slug" "not in --phases"
    return 0
  fi
  if phase_scoped "$slug" && (( CRATES_SET )) && [[ -z $CRATES ]] && (( ! WORKSPACE_SCOPE )); then
    record_phase "$slug" "$name" "scoped-out" "" 0 0 "" "$@"
    echo "SCOPED OUT: $name; no affected crates"
    return 0
  fi
  if (( print_only )); then
    printf '%-16s %s\n' "$slug" "$*"
    return 0
  fi
  local log
  if [[ -n $RUN_ID ]]; then
    log="$record_dir/$RUN_ID/$slug.log"
  else
    log=$(mktemp -t "gate-$slug" 2>/dev/null || echo "/tmp/gate-$slug.$$.log")
  fi
  local started code=0 attempts=1
  started=$(python3 -c 'import time; print(time.monotonic())')
  python3 scripts/run-verification-phase.py --log "$log" "$name" -- "$@" || code=$?
  if (( code != 0 && retry )) && environmental "$log"; then
    echo "RETRY: $name failed with a resource-exhaustion signature; retrying once."
    attempts=2
    code=0
    python3 scripts/run-verification-phase.py --log "$log" "$name" -- "$@" || code=$?
  fi
  local elapsed
  elapsed=$(python3 -c "import time; print(round(time.monotonic() - $started, 1))")
  if (( code == 0 )); then
    record_phase "$slug" "$name" "passed" 0 "$elapsed" "$attempts" "$log" "$@"
  else
    record_phase "$slug" "$name" "failed" "$code" "$elapsed" "$attempts" "$log" "$@"
    FAILED_PHASES+=("$slug")
    if (( ! keep_going )); then
      RESULT=failed
      echo "FAILED: $name; stopping. --keep-going runs the remaining phases." >&2
      exit "$code"
    fi
  fi
  return 0
}

if (( print_only )); then
  echo "Phases for this run (crates: ${CRATES:-workspace}):"
fi

if (( record )) && (( ! print_only )); then
  mkdir -p "$record_dir"
  crate_label=all
  if (( CRATES_SET )); then
    if (( WORKSPACE_SCOPE )); then crate_label=workspace; else crate_label=${CRATES:-none}; fi
  fi
  requested=$(WANTED="$WANTED" CRATE_LABEL="$crate_label" \
    WORKSPACE_SCOPE=$WORKSPACE_SCOPE CHANGED="$CHANGED" UNCOVERED="$UNCOVERED" \
    KEEP_GOING=$keep_going RETRY=$retry POSTGRES=$postgres METAL=$metal SOAK=$soak \
    python3 -c '
import json, os
print(json.dumps({
    "phases": os.environ.get("WANTED") or "all",
    "crates": os.environ.get("CRATE_LABEL"),
    "workspace_scope": bool(int(os.environ.get("WORKSPACE_SCOPE", "0"))),
    "changed_ref": os.environ.get("CHANGED") or None,
    "uncovered": os.environ.get("UNCOVERED") or "",
    "keep_going": bool(int(os.environ.get("KEEP_GOING", "0"))),
    "retry": bool(int(os.environ.get("RETRY", "1"))),
    "postgres": bool(int(os.environ.get("POSTGRES", "1"))),
    "metal": bool(int(os.environ.get("METAL", "0"))),
    "soak": bool(int(os.environ.get("SOAK", "0"))),
}))')
  begin_out=$(python3 scripts/gate-record.py begin --dir "$record_dir" --requested "$requested")
  RUN_ID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["run_id"])' "$begin_out")
  mkdir -p "$record_dir/$RUN_ID"
  echo "Gate run: $RUN_ID ($record_dir/$RUN_ID/run.json)"
fi

echo 'Gate: Rust 1.97.1, rustfmt style edition 2024.'

# The preflight is cheap and always runs when its phase is selected:
# environmental prerequisites fail here, not inside a twenty-minute test.
if phase_wanted preflight; then
  if (( print_only )); then
    printf '%-16s %s\n' "preflight" "environment prerequisites"
  else
    preflight_log=""
    [[ -n $RUN_ID ]] && preflight_log="$record_dir/$RUN_ID/preflight.log"
    pf_code=0
    if [[ -n $preflight_log ]]; then
      preflight_checks 2>&1 | tee "$preflight_log" || pf_code=$?
    else
      preflight_checks || pf_code=$?
    fi
    if (( pf_code != 0 )); then
      record_phase preflight "Environment preflight" "failed" "$pf_code" 0 1 "$preflight_log" "environment preflight"
      RESULT=failed
      echo "FAILED: environment preflight; fix the prerequisites above and rerun." >&2
      exit "$pf_code"
    fi
    record_phase preflight "Environment preflight" "passed" 0 0 1 "$preflight_log" "environment preflight"
  fi
else
  record_skip preflight "not in --phases"
fi

run_phase gate-tooling "Gate tooling tests" \
  bash -c 'python3 -B -m unittest discover -s scripts/tests -p "test_gate_*.py" \
    && python3 -B -m unittest discover -s scripts -p "test_verification_phase.py"'
run_phase artifacts "Artifact acquisition tests" \
  python3 scripts/test_fetch_kev_artifacts.py
run_phase delegation "Delegation evidence checks" \
  python3 -B -m unittest discover -s scripts/tests -p 'test_check_coder_delegation_run.py'
run_phase backup "Backup collection tests" \
  python3 -B -m unittest discover -s scripts/tests -p 'test_backup_media.py'
run_phase fmt "Workspace formatting" cargo fmt --all --check
run_phase clippy "Default workspace Clippy" \
  cargo clippy --locked "${scope[@]+"${scope[@]}"}" --all-targets -- -D warnings
run_phase clippy-features "Feature workspace Clippy" \
  cargo clippy --locked "${scope[@]+"${scope[@]}"}" --all-targets "${feature_args[@]+"${feature_args[@]}"}" -- -D warnings
run_phase tests "Default workspace tests" \
  cargo test --locked "${scope[@]+"${scope[@]}"}" -- --nocapture
run_phase tests-features "Feature workspace tests" \
  cargo test --locked "${scope[@]+"${scope[@]}"}" "${feature_args[@]+"${feature_args[@]}"}" -- --nocapture
if ! phase_wanted deps; then
  record_skip deps "not in --phases"
elif cargo +1.97.1 deny --version >/dev/null 2>&1; then
  run_phase deps "Dependency policy" ./scripts/check-dependencies.sh
else
  record_skip deps "cargo-deny is not installed, so this is a partial gate"
fi

if ! phase_wanted postgres; then
  record_skip postgres "not in --phases"
elif (( postgres )); then
  run_phase postgres "PostgreSQL acceptance" ./scripts/test-postgres.sh
else
  record_skip postgres "--skip-postgres; this is a partial gate"
fi

if ! phase_wanted metal-clippy && ! phase_wanted metal-tests; then
  record_skip metal-clippy "not in --phases"
  record_skip metal-tests "not in --phases"
elif (( metal )); then
  run_phase metal-clippy "Metal Clippy" \
    cargo clippy --locked -p kev --all-targets --features serve,metal -- -D warnings
  run_phase metal-tests "Metal feature tests" \
    cargo test --locked -p kev --features serve,metal -- --nocapture
else
  record_skip metal-clippy "Metal checks need --with-metal on a supported Apple host"
  record_skip metal-tests "Metal checks need --with-metal on a supported Apple host"
fi

if ! phase_wanted soak; then
  record_skip soak "not in --phases"
elif (( soak )); then
  run_phase soak "Relay soak" ./scripts/test-soak.sh
else
  record_skip soak "long-running relay soak; use --with-soak"
fi

if (( print_only )); then
  exit 0
fi

if (( ${#FAILED_PHASES[@]} )); then
  RESULT=failed
  echo "FAILED phases: ${FAILED_PHASES[*]}" >&2
  exit 1
fi

# A run that skipped anything the full gate covers is partial evidence:
# the record, not a bare "passed", is what the tree earned.
if [[ -n $WANTED || $postgres -eq 0 || $metal -eq 0 || $soak -eq 0 ]] \
  || { (( CRATES_SET )) && [[ -z $CRATES ]] && (( ! WORKSPACE_SCOPE )); }; then
  RESULT=partial
else
  RESULT=passed
fi
if (( record )) && [[ -n $RUN_ID ]]; then
  python3 scripts/gate-record.py finish --dir "$record_dir" --run-id "$RUN_ID" --result "$RESULT"
  RUN_ID=""
fi
echo 'External model and paid live-door measurements require their documented separate runs.'
echo 'Only completed commands above constitute verification evidence.'
