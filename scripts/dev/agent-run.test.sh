#!/usr/bin/env bash
# Tests for agent-run.py (#11433): a fake Claude Code run (a sleep tree in
# tool-shell process groups, plus a recorded VM) is reaped on SessionEnd and
# by `stop-agent-burn.sh --run`; bystanders and the root survive; the sweep
# picks only old launchd-parented agent processes and never a tty session.
# Only sleep processes this test starts are signalled; gcloud is a mock.
# Run: scripts/dev/agent-run.test.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ar=$here/agent-run.py
tmp=$(cd "$(mktemp -d)" && pwd -P)
started=()
cleanup() {
  for p in "${started[@]}"; do kill -KILL "$p" 2>/dev/null || true; done
  pkill -KILL -f "$tmp/" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT
export OA_RUNS_DIR=$tmp/runs OA_GCLOUD=$tmp/gcloud
unset OA_RUN_ID CLOUDSDK_CONFIG
mkdir -p "$tmp/bin"
cat >"$OA_GCLOUD" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$tmp/gcloud.log"
if [ "\$1 \$2 \$3" = "compute instances list" ]; then cat "$tmp/vm-list" 2>/dev/null; fi
exit 0
EOF
chmod +x "$OA_GCLOUD"
: >"$tmp/vm-list"
fail() { echo "FAIL: $*"; exit 1; }
alive() { kill -0 "$1" 2>/dev/null && [ "$(ps -o stat= -p "$1" | cut -c1)" != Z ]; }
# Start a command as its own session and process group, like a Bash tool
# shell (always called with &, so exec replaces only the job's subshell).
newsession() { exec python3 -c 'import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])' "$@"; }
pid_of() { # wait for a pid file
  for _ in $(seq 50); do [ -s "$1" ] && { cat "$1"; return; }; sleep 0.1; done
  fail "no pid in $1"
}

# The fake claude process (the run's root) and a bystander outside the run.
sleep 600 & root=$!; disown; started+=("$root")
sleep 600 & bystander=$!; disown; started+=("$bystander")

# SessionStart: the run and the env script every tool shell sources.
echo '{"session_id":"s-1","source":"startup"}' |
  CLAUDE_PID=$root CLAUDE_ENV_FILE=$tmp/env sh "$here/agent-run-hook.sh" session-start
rid=$(ls "$OA_RUNS_DIR")
[ -n "$rid" ] && [ -f "$OA_RUNS_DIR/$rid/run.json" ] || fail "session-start creates a run"
grep -q "export OA_RUN_ID=$rid" "$tmp/env" || fail "the env script exports OA_RUN_ID"
jq -e '.sessions == ["s-1"] and .kind == "claude"' "$OA_RUNS_DIR/$rid/run.json" >/dev/null || fail "run.json records the session"

# Tool shell 1: nohup's a sleep and exits (the orphan keeps the group).
newsession bash -c ". $tmp/env; nohup sleep 601 >/dev/null 2>&1 & echo \$! > $tmp/orphan" & disown
orphan=$(pid_of "$tmp/orphan"); started+=("$orphan")
# Tool shell 2: a long background shell with a child.
newsession bash -c ". $tmp/env; sleep 602 & echo \$! > $tmp/bgchild; wait" & shell2=$!; disown; started+=("$shell2")
bgchild=$(pid_of "$tmp/bgchild"); started+=("$bgchild")
# Tool shell 3: its child leaves the group with setsid (bash-post records it).
newsession bash -c ". $tmp/env; python3 -c 'import os; os.setsid(); os.execvp(\"sleep\", [\"sleep\", \"603\"])' & echo \$! > $tmp/escapee; wait" & shell3=$!; disown; started+=("$shell3")
escapee=$(pid_of "$tmp/escapee"); started+=("$escapee")
sleep 0.5
[ "$(wc -l <"$OA_RUNS_DIR/$rid/shells" | tr -d ' ')" -ge 3 ] || fail "each tool shell records its pid"
[ "$(ps -o ppid= -p "$orphan" | tr -d ' ')" = 1 ] || fail "the nohup'd sleep is orphaned to launchd (test setup)"

echo '{"tool_name":"Bash","tool_input":{"command":"x"}}' |
  CLAUDE_PID=$root sh "$here/agent-run-hook.sh" bash-post
grep -q "^$escapee " "$OA_RUNS_DIR/$rid/pids" && fail "bash-post ignores a foreground call"
echo '{"tool_name":"Bash","tool_input":{"command":"x","run_in_background":true}}' |
  CLAUDE_PID=$root sh "$here/agent-run-hook.sh" bash-post
grep -q "^$escapee " "$OA_RUNS_DIR/$rid/pids" || fail "bash-post records a child that left its shell's group"

# /clear does not reap.
echo '{"reason":"clear"}' | CLAUDE_PID=$root python3 "$ar" hook session-end
alive "$orphan" || fail "SessionEnd on /clear leaves the run alone"

# A recorded VM, stopped (detached) at SessionEnd.
python3 "$ar" add-vm "$rid" fake-vm us-central1-a

echo '{"reason":"prompt_input_exit"}' | CLAUDE_PID=$root sh "$here/agent-run-hook.sh" session-end >/dev/null
for p in "$orphan" "$shell2" "$bgchild" "$shell3" "$escapee"; do
  alive "$p" && fail "SessionEnd reaps pid $p"
done
alive "$root" || fail "SessionEnd leaves the session's own process"
alive "$bystander" || fail "SessionEnd leaves processes outside the run"
jq -e '.ended != null' "$OA_RUNS_DIR/$rid/run.json" >/dev/null || fail "the run is marked ended"
for _ in $(seq 50); do grep -q "instances stop fake-vm" "$tmp/gcloud.log" 2>/dev/null && break; sleep 0.1; done
grep -q "instances stop fake-vm --zone=us-central1-a" "$tmp/gcloud.log" || fail "SessionEnd stops the run's VMs"

# A bench run cancelled by hand: --run reaps the recorded tree, the root
# (--with-root via stop-agent-burn) and VMs found by label.
sleep 600 & bench=$!; disown; started+=("$bench")
rid2=$(python3 "$ar" new --kind bench --root "$bench" --id bench-test-1)
newsession bash -c "echo \"\$\$ \$(date +%s)\" >> $OA_RUNS_DIR/$rid2/shells; sleep 604 & echo \$! > $tmp/b1; wait" & bshell=$!; disown; started+=("$bshell")
b1=$(pid_of "$tmp/b1"); started+=("$b1")
sleep 605 & extra=$!; disown; started+=("$extra")
python3 "$ar" add-pid "$rid2" "$extra" worker
echo "labelled-vm us-east5-a $rid2 $(python3 "$ar" labels "$rid2" | sed 's/.*run-host=//')" >"$tmp/vm-list"
[ "$(python3 "$ar" labels "$rid2" | cut -d, -f1)" = "run-id=bench-test-1" ] || fail "labels name the run"
out=$(python3 "$ar" reap "$rid2" --dry-run)
alive "$b1" || fail "--dry-run signals nothing"
grep -q "labelled-vm" <<<"$out" || fail "--dry-run lists the labelled VM"
bash "$here/stop-agent-burn.sh" --run "$rid2" >/dev/null
for p in "$bshell" "$b1" "$extra" "$bench"; do
  alive "$p" && fail "--run reaps pid $p"
done
alive "$bystander" || fail "--run leaves processes outside the run"
grep -q "instances stop labelled-vm --zone=us-east5-a" "$tmp/gcloud.log" || fail "--run stops VMs labelled with the run id"

# A run's id label value is GCP-safe.
rid3=$(python3 "$ar" new --kind "Solver Run" --root "$root")
[[ $rid3 =~ ^[a-z0-9_-]{1,63}$ ]] || fail "run ids are valid GCP label values ($rid3)"

# Sweep selection from a fixture process table: only old, launchd-parented,
# terminal-less claude / coder-new / bench processes and orphaned Claude Code
# tool shells, judged by their executable.
old="Mon Oct  5 01:00:00 2026"
young=$(LC_ALL=C date "+%a %b %e %H:%M:%S %Y")
cat >"$tmp/ps-fixture" <<EOF
  9001     1  9001 ??       $old /Users/x/.local/bin/claude daemon run
  9002     1  9002 ??       $old /Users/x/work/target/debug/coder-new
  9003     1  9003 ttys001  $old /Users/x/.local/bin/claude --dangerously-skip-permissions
  9004     1  9004 ??       $young /Users/x/work/target/debug/coder-new
  9005     1  9005 ??       $old python3 ab.py batch --plan plans/x.json
  9006     1  9006 ??       $old /usr/sbin/cfprefsd agent
  9007   500  9007 ??       $old /Users/x/work/target/debug/coder-new
  9008     1  9008 ??       $old /bin/zsh -c source /Users/x/.claude/shell-snapshots/s.sh && eval 'until false; do sleep 15; done'
  9009     1  9009 ??       $old /usr/bin/grep coder-new
  9010     1  9010 ??       $old /Library/Frameworks/Python.framework/Versions/3.9/Resources/Python.app/Contents/MacOS/Python ab.py batch
EOF
printf '#!/bin/sh\ncat %s\n' "$tmp/ps-fixture" >"$tmp/fakeps"; chmod +x "$tmp/fakeps"
: >"$tmp/vm-list"
out=$(OA_PS_CMD=$tmp/fakeps python3 "$ar" sweep --hours 12)
for p in 9001 9002 9005 9008 9010; do grep -q "pid *$p " <<<"$out" || fail "sweep picks $p"; done
for p in 9003 9004 9006 9007 9009; do grep -q "pid *$p " <<<"$out" && fail "sweep skips $p"; done
grep -q "report only" <<<"$out" || fail "sweep without --kill only reports"

# Sweep kill on real processes this test owns: an old-enough (--hours 0)
# orphaned "coder-new" and the leftover of a run whose root is gone. VMs
# labelled for this host whose run is gone are stopped; a live run's are not.
ln -sf "$(command -v sleep)" "$tmp/bin/coder-new"
newsession bash -c "nohup $tmp/bin/coder-new 606 >/dev/null 2>&1 & echo \$! > $tmp/c1" & disown
c1=$(pid_of "$tmp/c1"); started+=("$c1")
deadroot=$(sh -c 'echo $$')  # a pid that has exited
rid4=$(python3 "$ar" new --kind bench --root "$deadroot" --id gone-run)
newsession bash -c "echo \"\$\$ \$(date +%s)\" >> $OA_RUNS_DIR/$rid4/shells; nohup $tmp/bin/coder-new 607 >/dev/null 2>&1 & echo \$! > $tmp/c2" & disown
c2=$(pid_of "$tmp/c2"); started+=("$c2")
h=$(python3 "$ar" labels "$rid4" | sed 's/.*run-host=//')
rid5=$(python3 "$ar" new --kind claude --root "$root" --id live-run)
printf 'gone-vm us-central1-b gone-run %s\nlive-vm us-central1-b live-run %s\nother-host-vm us-central1-b gone-run elsewhere\n' "$h" "$h" >"$tmp/vm-list"
: >"$tmp/gcloud.log"
OA_SWEEP_PREFIX=$tmp/ python3 "$ar" sweep --hours 0 --kill >/dev/null
sleep 0.5
alive "$c1" && fail "sweep --kill stops an orphaned coder-new"
alive "$c2" && fail "sweep --kill stops a gone run's leftovers"
alive "$root" || fail "sweep leaves a process with a parent"
alive "$bystander" || fail "sweep leaves unrelated processes"
grep -q "instances stop gone-vm" "$tmp/gcloud.log" || fail "sweep stops a gone run's VM"
grep -q "instances stop live-vm" "$tmp/gcloud.log" && fail "sweep leaves a live run's VM"
grep -q "labels.run-host=$h" "$tmp/gcloud.log" || fail "sweep only lists this host's VMs"

# label-vm labels an existing VM and records it.
: >"$tmp/gcloud.log"
OA_RUN_ID=$rid5 python3 "$ar" label-vm oa-dev-env-9 us-central1-b
grep -q "add-labels oa-dev-env-9 --zone=us-central1-b --project=openagentsgemini --labels=run-id=live-run,run-host=$h" "$tmp/gcloud.log" ||
  fail "label-vm adds the run labels"
grep -q "^oa-dev-env-9 us-central1-b" "$OA_RUNS_DIR/$rid5/vms" || fail "label-vm records the VM"

# A hook never fails the session, even with no run and junk input.
echo 'not json' | CLAUDE_PID=1 python3 "$ar" hook session-end || fail "a hook exits 0"
echo "agent-run tests passed"
