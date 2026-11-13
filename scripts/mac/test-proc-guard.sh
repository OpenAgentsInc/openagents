#!/bin/bash
# Firm reason: 5 (the check of proc-guard.sh, a shell LaunchAgent script).
# Starts a few dozen sleeping copies of `sleep` under other names (symbolic
# links: a copy of a system binary is killed by code signing) and runs the guard
# once with its own settings and log folder: a runaway that a stop= pattern
# matches is stopped and named; a runaway nothing matches is only named; a
# program under the copy limit is left alone, even when a pattern matches.
# It stops only the processes it started.
#   scripts/mac/test-proc-guard.sh     exit 0 when every case passes
set -u
here="$(cd "$(dirname "$0")" && pwd -P)"
w="$(mktemp -d "${TMPDIR:-/tmp}/proc-guard-test.XXXXXXXX")" || exit 2
started=()
finish() { (( ${#started[@]} )) && kill -KILL "${started[@]}" 2>/dev/null; rm -rf "${w:?}"; }
trap finish EXIT
fails=0
ok() { echo "ok: $*"; }
bad() { echo "FAIL: $*"; fails=$((fails + 1)); }

tag="pgt$$"
for n in run keep few; do ln -s /bin/sleep "$w/$tag-$n" || exit 2; done
spawn() { local i; for ((i = 0; i < $2; i++)); do "$w/$tag-$1" 600 & started+=($!); disown $!; done; }
spawn run 30; spawn keep 30; spawn few 5
alive() { local n=0 p; for p in "${started[@]}"; do ps -o comm= -p "$p" 2>/dev/null | grep -q "$tag-$1\$" && n=$((n + 1)); done; echo "$n"; }
sleep 0.5
[[ "$(alive run)" == 30 && "$(alive keep)" == 30 ]] || { bad "the copies did not start"; exit 1; }

printf 'warn_total=999999\ncopies=20\nstop=%s-run 600$\nstop=%s-few 600$\n' "$tag" "$tag" > "$w/conf"
PROC_GUARD_CONF="$w/conf" PROC_GUARD_LOGS="$w/logs" PROC_GUARD_NOTIFY=0 PROC_GUARD_KILL_AFTER=1 bash "$here/proc-guard.sh"
log="$(cat "$w/logs/guard.log" 2>/dev/null)"

[[ "$(alive run)" == 0 ]] && ok "a runaway a stop= pattern matches is stopped" || bad "the runaway: $(alive run) still running"
grep -q "runaway: 30 copies of '$tag-run'" <<< "$log" && grep -q "runaway '$tag-run': stopped 30" <<< "$log" \
  && ok "the log names it, its count and what was stopped" || bad "the log: $log"
grep -q "runaway: 30 copies of '$tag-keep' (more than 20): 30 in group" <<< "$log" && ok "the log names the process group that holds the copies" || bad "no group: $log"
[[ "$(alive keep)" == 30 ]] && ok "a runaway no pattern matches is only logged" || bad "keep: $(alive keep) of 30 left"
grep -q "runaway '$tag-keep': no copy matches a stop= pattern; not stopped" <<< "$log" && ok "and the log says it was not stopped" || bad "keep's line: $log"
[[ "$(alive few)" == 5 ]] && ! grep -q "$tag-few" <<< "$log" && ok "a program under the copy limit is left alone" || bad "few: $(alive few) of 5 left"

# No stop= line: only logs.
printf 'copies=20\n' > "$w/conf2"
PROC_GUARD_CONF="$w/conf2" PROC_GUARD_LOGS="$w/logs2" PROC_GUARD_NOTIFY=0 bash "$here/proc-guard.sh"
[[ "$(alive keep)" == 30 ]] && grep -q "copies of '$tag-keep'" "$w/logs2/guard.log" && ok "with no stop= line it only logs" || bad "no stop= line: $(alive keep) left"

if (( fails )); then echo "test-proc-guard.sh: $fails failed"; exit 1; fi
echo "test-proc-guard.sh: every case passed"
