#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around git, build.sh, PlistBuddy, codesign and launchctl; holds no logic)
# Keeps /Applications/OpenAgents.app at the newest origin/main.
#   apps/openagents-desktop/install.sh [--minimal|--full] [--force] [--local]
#                                                                  build origin/main (or current checkout with --local) and install it now
#                                                                  (defaults to --minimal); --force bypasses skip/up-to-date caches and active-reply deferral.
#                                                                  An OpenAgents (or legacy Coder) open from /Applications is quit first and opened again after.
#                                                                  Exits 4 when deferred (active reply, inside app, or lock held).
#   apps/openagents-desktop/install.sh [--minimal|--full] --agent  also check every 15 minutes (a LaunchAgent), and run once now
#   apps/openagents-desktop/install.sh --remove-agent              stop the checks
#   apps/openagents-desktop/install.sh [--minimal|--full] --check  what the LaunchAgent runs: build when main moved, install
#                                                                  only while OpenAgents is closed (the build waits beside it until then)
#   apps/openagents-desktop/install.sh [--minimal|--full] --status what is installed, what waits, what main is at
# --minimal (default; OA_RELEASE_MINIMAL=0 or --full selects the full developer build) builds the v1.0
# minimal release profile, watches only v1.0 Coax roots, and verifies the bundle with
# apps/openagents-desktop/v1-bundle-check.coax before promoting it to /Applications/OpenAgents.app.
# It builds in a checkout of its own under ~/.cache/openagents/coder-release, never in yours.
# Each build records what it read: the Coax sources of the page, the shader and the workers
# (coax deps), and the folders of the compiler, the C, Swift and Objective-C hosts and the
# assets. A main that moved without changing any of them is not built again, since the
# installed OpenAgents is already that commit's build. (A new #load needs an edit to a file the build
# already read, so the list can't miss one.)
# The installed app carries its commit (OACommit in Info.plist), profile (OAProfile) and commit count (CFBundleVersion).
# The resident daemon (`coder host serve`, the program behind ~/.openagents/bin/openagents-environment and
# the login service com.openagents.coax.environment) is kept at the newest built OpenAgents's coder-host: the
# installed one's, or a build waiting for OpenAgents to quit, so a paired phone never waits on that quit.
# One binary, one resident service. It is restarted only while it runs no work (GitHub #13757).
set -euo pipefail
export LC_ALL=C BROWSER=/usr/bin/true
state="${CODER_RELEASE_HOME:-$HOME/.cache/openagents/coder-release}"
src="$state/src"
stage="$state/stage"
dest="${CODER_APP_DIR:-/Applications}"
app="$dest/OpenAgents.app"
legacy_app="$dest/Coder.app"
label="com.openagents.desktop-update"
legacy_label="com.openagents.coder-update"
agent_plist="$HOME/Library/LaunchAgents/$label.plist"
legacy_agent_plist="$HOME/Library/LaunchAgents/$legacy_label.plist"
minimal=1
if [[ "${OA_RELEASE_MINIMAL:-1}" == "0" ]]; then minimal=0; fi
mode="now"
force_build=0
use_local=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    --force) export OA_INSTALL_FORCE=1; force_build=1 ;;
    --local) use_local=1 ;;
    now|--agent|--remove-agent|--check|--status) mode="$1" ;;
    *) echo "install.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done
want_profile="full"
[[ "$minimal" == 1 ]] && want_profile="minimal"
# Bump whenever fixed/roots tracking changes so stale $state/inputs files never skip a needed rebuild.
inputs_schema="4"
# What every build reads besides the Coax sources coax deps lists: the compiler's inputs (as
# coax/provenance.sh lists them), what coax/web/tool.sh links into the workers, and the app's
# hosts and assets.
fixed=(coax/src coax/bootstrap coax/lib/compiler coax/lib/proto ':(glob)coax/*' scripts/coax-release.sh coax/lib/crypto/sha256-kernel.coax
       coax/platform coax/web/tool.sh coax/lib/crypto/entropy.coax coax/lib/sys coax/lib/net
       coax/lib/nostr/nip06.coax pay metaverse/stage-resources/bip39-english.txt
       openagents/core
       apps/shared/auth apps/shared/host apps/shared/native components/chat components/fonts components/icons components/NOTICE apps/openagents-desktop/native psionic/platform psionic/lib/inkling)
if [[ "$minimal" == 1 ]]; then
  roots=(components/backend/gpu-shader.coax components/backend/composition-shader.coax coder/tui/host.coax coder/tui/oa-worker.coax coder/tui/gitguard.coax)
else
  roots=(components/backend/gpu-shader.coax components/backend/composition-shader.coax coder/tui/host.coax coder/tui/oa-worker.coax coder/tui/agyworker.coax coder/tui/gitguard.coax coder/tui/claudeworker.coax coder/tui/oasim.coax coder/tui/lean-run.coax coder/tui/tbench-run.coax coder/tui/gbench-run.coax metaverse/rh-flythrough.coax)
fi
mkdir -p "$state"

say() { echo "$(date '+%Y-%m-%d %H:%M:%S') coder-release: $*"; }
commit_of() { /usr/libexec/PlistBuddy -c "Print :OACommit" "$1/Contents/Info.plist" 2>/dev/null || true; }
version_of() {
  local plist="$1/Contents/Info.plist"
  [[ -f "$plist" ]] || return 0
  local sv bv cm sh out
  sv="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$plist" 2>/dev/null || true)"
  bv="$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$plist" 2>/dev/null || true)"
  cm="$(/usr/libexec/PlistBuddy -c "Print :OACommit" "$plist" 2>/dev/null || true)"
  [[ -n "$sv" ]] || sv="1.0.0"
  sv="${sv#v}"; sv="${sv#V}"
  sh="$(printf '%s' "$cm" | tr 'A-F' 'a-f' | cut -c1-7)"
  out="$sv"
  if [[ "$bv" =~ ^[0-9]+$ && "$bv" -gt 0 ]]; then out="${out}.${bv}"; fi
  if [[ -n "$sh" ]]; then out="${out}.${sh}"; fi
  printf '%s' "$out"
}
profile_of() {
  [[ -f "$1/Contents/Info.plist" ]] || return 0
  local p
  p="$(/usr/libexec/PlistBuddy -c "Print :OAProfile" "$1/Contents/Info.plist" 2>/dev/null || true)"
  [[ -n "$p" ]] && echo "$p" || echo "full"
}
# The app's folder in a checkout: apps/openagents-desktop once the Phase 2 move lands (#13780), apps/coder-desktop
# before it. Checked per build, so this script keeps working across the move.
app_dir() { if [[ -f "$1/apps/openagents-desktop/app.coax" ]]; then echo apps/openagents-desktop; else echo apps/coder-desktop; fi; }
# True when this script runs inside OpenAgents or a daemon-owned agent worker (an agent in OpenAgents ran it):
# quitting the app or restarting the daemon would kill this run with it, half done (#14095).
inside_app() {
  # Workers always export OA_AGENT_ID; even a `nohup ... &` reparented to PID 1 inherits it.
  [[ -n "${OA_AGENT_ID:-}" ]] && return 0
  local p=$$ pids=" $* " comm
  while [[ -n "$p" && "$p" -gt 1 ]]; do
    [[ "$pids" == *" $p "* ]] && return 0
    comm="$(ps -o comm= -p "$p" 2>/dev/null || true)"
    case "${comm##*/}" in
      oa-worker|agyworker|claudeworker|inky|coder-host|openagents-environment) return 0 ;;
    esac
    p="$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')"
  done
  return 1
}
coder_pids() {
  { pgrep -f "^$app/Contents/MacOS/OpenAgents" || true;
    pgrep -f "^$legacy_app/Contents/MacOS/Coder" || true; } | sort -u
}

# The resident daemon: one program (coder/tui/host.coax), shipped in Coder as coder-host and run at login
# by com.openagents.coax.environment from ~/.openagents/bin/openagents-environment.
env_label="${CODER_ENV_LABEL:-com.openagents.coax.environment}"
env_plist="$HOME/Library/LaunchAgents/$env_label.plist"
env_link="$HOME/.openagents/bin/openagents-environment"
host_sock="$HOME/.openagents/coder/host/service.sock"
host_discovery="$HOME/.openagents/coder/host/daemon.json"
# True when PID is a running openagents-environment / coder-host daemon process (never a leaked child/Python process).
is_daemon_pid() {
  local p="$1" comm exe
  [[ "$p" =~ ^[0-9]+$ && "$p" -gt 1 ]] || return 1
  kill -0 "$p" 2>/dev/null || return 1
  comm="$(ps -o comm= -p "$p" 2>/dev/null || true)"
  case "${comm##*/}" in
    openagents-environment|coder-host) return 0 ;;
  esac
  exe="$(lsof -a -p "$p" -d txt -Fn 2>/dev/null | awk 'NR > 0 && /^n/ && e == "" {e = substr($0, 2)} END {print e}' || true)"
  case "${exe##*/}" in
    openagents-environment|coder-host) return 0 ;;
  esac
  return 1
}
# The resident daemon process owning service.sock: prefer the verified PID in daemon.json, falling back to
# lsof -U filtered strictly to openagents-environment/coder-host (#14095).
# awk reads all of lsof's output (an early exit would end lsof by SIGPIPE, which pipefail makes fatal).
sock_pid() {
  if [[ -f "$host_discovery" ]]; then
    local dpid
    dpid="$(python3 -I - "$host_discovery" <<'PY' 2>/dev/null || true
import json, sys
try:
    d = json.load(open(sys.argv[1]))
    if isinstance(d, dict) and d.get("alive") is True:
        p = d.get("pid")
        if isinstance(p, int) and p > 1:
            print(p)
except Exception:
    pass
PY
)"
    if [[ -n "$dpid" ]] && is_daemon_pid "$dpid"; then
      echo "$dpid"
      return 0
    fi
  fi
  local all filtered="" p
  all="$(lsof -U 2>/dev/null | awk -v s="$host_sock" '$NF == s {print $2}' | sort -u)"
  for p in $all; do
    if is_daemon_pid "$p"; then
      filtered="${filtered}${filtered:+$'\n'}${p}"
    fi
  done
  [[ -n "$filtered" ]] || return 0
  for p in $filtered; do
    grep -qx "$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')" <<< "$filtered" || { echo "$p"; return 0; }
  done
}
# True while any daemon run in ~/.openagents/coder/host/runs/*.state is still running or stopping with a live worker PID.
runs_active() {
  python3 -I - "$HOME/.openagents/coder/host/runs" <<'PY'
import json, os, sys
d = sys.argv[1]
for f in (os.listdir(d) if os.path.isdir(d) else []):
    if not f.endswith(".state"): continue
    try:
        st = json.load(open(os.path.join(d, f)))
        if not isinstance(st, dict) or st.get("state") not in ("running", "stopping"): continue
        pid = st.get("pid")
        if isinstance(pid, int) and pid > 1:
            try:
                os.kill(pid, 0)
                sys.exit(0)
            except PermissionError:
                sys.exit(0)
            except OSError:
                continue
        else:
            sys.exit(0)
    except (OSError, ValueError):
        continue
sys.exit(1)
PY
}
# The daemon is busy while it has any child process OR any live run in runs/*.state; a restart then waits (#14095).
daemon_busy() {
  local p; p="$(sock_pid)"
  if [[ -n "$p" ]] && [[ -n "$(pgrep -P "$p" 2>/dev/null || true)" ]]; then return 0; fi
  runs_active && return 0
  replies_running && return 0
  return 1
}
# True while a worker run is mid-reply: a running run whose last line is neither an idle line
# (ready, journal, aware, ended, empty queue) nor an empty .out file (host-runs.coax dr_idle_line).
# Quitting OpenAgents then could cost that reply, so an install waits (#13810).
replies_running() {
  python3 -I - "$HOME/.openagents/coder/host/runs" <<'PY'
import json, os, sys
d = sys.argv[1]
IDLE_KINDS = ("ready", "journal", "aware", "ended")
for f in (os.listdir(d) if os.path.isdir(d) else []):
    if not f.endswith(".state"): continue
    try:
        st = json.load(open(os.path.join(d, f)))
        if not isinstance(st, dict) or st.get("state") != "running": continue
        outp = os.path.join(d, f[:-6] + ".out")
        with open(outp, "rb") as h:
            h.seek(0, 2); n = h.tell()
            if n == 0: continue
            win = 65536
            h.seek(max(0, n - win))
            lines = [l for l in h.read().decode("utf-8", "replace").split("\n") if l.strip()]
            if not lines: continue
            try:
                q = json.loads(lines[-1])
            except ValueError:
                if n > win:
                    h.seek(max(0, n - 1048576))
                    lines = [l for l in h.read().decode("utf-8", "replace").split("\n") if l.strip()]
                    q = json.loads(lines[-1]) if lines else {}
                else:
                    raise
        if isinstance(q, dict) and q.get("kind") in IDLE_KINDS: continue
        if not isinstance(q, dict) or not (q.get("kind") == "queue" and q.get("active") == "" and q.get("items") == []): sys.exit(0)
    except (OSError, ValueError): continue
sys.exit(1)
PY
}
# Syncs the given app's bundled worker binaries (~/.openagents/bin/*) and resident daemon
# (coder-host -> ~/.openagents/bin/openagents-environment) to the exact same version identifier
# ({version}.{build}.{hash7}). A newer build waiting for OpenAgents to quit hands its daemon and
# workers over first so paired clients and standalone invocations stay in lockstep.
sync_resident() {
  local from="${1:-$app}"
  local res="$from/Contents/Resources"
  local bundled="$res/coder-host" commit ver_id dir want bin_dir="$HOME/.openagents/bin"
  [[ -x "$bundled" ]] || return 0
  commit="$(commit_of "$from" | cut -c1-16)"
  ver_id="$(version_of "$from")"
  [[ -n "$ver_id" ]] || ver_id="${commit:-unknown}"
  mkdir -p "$bin_dir" "$HOME/.openagents/environment/bin"
  dir="$HOME/.openagents/environment/bin/$ver_id"
  mkdir -p "$dir"
  # Atomically sync all bundled worker and runtime helpers into ~/.openagents/bin/ AND the immutable
  # per-version directory ~/.openagents/environment/bin/<ver_id>/ so swapping OpenAgents.app never
  # replaces binaries underneath active daemon workers (#14095).
  local helper src_h dst_h vdst_h tmp_h mode
  for helper in oa-worker agyworker gitguard coder-host psionic-inkling-serve inkling-kernels.metal claudeworker oasim ple-run lean-run tbench-run gbench-run; do
    src_h="$res/$helper"
    [[ -f "$src_h" ]] || continue
    mode=0755; [[ "$helper" == *.metal ]] && mode=0644
    dst_h="$bin_dir/$helper"
    if ! cmp -s "$src_h" "$dst_h" 2>/dev/null; then
      tmp_h="$bin_dir/.${helper}.new.$$"
      cp "$src_h" "$tmp_h" && chmod "$mode" "$tmp_h" && mv -f "$tmp_h" "$dst_h"
    fi
    vdst_h="$dir/$helper"
    if ! cmp -s "$src_h" "$vdst_h" 2>/dev/null; then
      tmp_h="$dir/.${helper}.new.$$"
      cp "$src_h" "$tmp_h" && chmod "$mode" "$tmp_h" && mv -f "$tmp_h" "$vdst_h"
    fi
  done
  printf '%s\n' "$ver_id" > "$bin_dir/.worker-version.new.$$" && mv -f "$bin_dir/.worker-version.new.$$" "$bin_dir/.worker-version"
  if ! cmp -s "$bundled" "$dir/openagents-environment" 2>/dev/null; then
    cp "$bundled" "$dir/openagents-environment.new.$$" && chmod 0755 "$dir/openagents-environment.new.$$" && mv -f "$dir/openagents-environment.new.$$" "$dir/openagents-environment"
  fi
  printf '%s\n' "$ver_id" > "$HOME/.openagents/environment/version.new.$$" && mv -f "$HOME/.openagents/environment/version.new.$$" "$HOME/.openagents/environment/version"
  mkdir -p "$(dirname "$env_link")"
  [[ "$(readlink "$env_link" 2>/dev/null)" == "$dir/openagents-environment" ]] || ln -sfn "$dir/openagents-environment" "$env_link"
  # Which binary answers now: the login service's (through the link) or a coder-host Coder started.
  # The file it runs, from lsof: ps names the link it started through, which may point elsewhere now.
  local p exe; p="$(sock_pid)"
  exe="$( [[ -n "$p" ]] && lsof -a -p "$p" -d txt -Fn 2>/dev/null | awk 'NR > 0 && /^n/ && e == "" {e = substr($0, 2)} END {print e}' || true)"
  local stale=0; [[ -n "$p" ]] && ! cmp -s "$exe" "$bundled" && stale=1
  local restart=0
  if [[ -f "$env_plist" ]]; then
    # The login service runs the link, as a standby, so it never crash-loops while another serves.
    want="$env_link serve --standby"
    if [[ "$(/usr/libexec/PlistBuddy -c 'Print :ProgramArguments' "$env_plist" 2>/dev/null | sed -n '2,$p' | sed '$d' | xargs)" != "$want" ]]; then
      /usr/libexec/PlistBuddy -c 'Delete :ProgramArguments' "$env_plist"
      /usr/libexec/PlistBuddy -c 'Add :ProgramArguments array' -c "Add :ProgramArguments:0 string $env_link" \
        -c 'Add :ProgramArguments:1 string serve' -c 'Add :ProgramArguments:2 string --standby' "$env_plist"
      restart=1
    fi
  fi
  (( stale || restart )) || return 0
  if daemon_busy; then say "the resident daemon runs work; it moves to $ver_id at a later check (workers synced to $ver_id)"; return 0; fi
  if [[ -f "$env_plist" ]]; then
    # The login service stops (launchd would restart it otherwise), then whatever still holds the
    # socket (a coder-host Coder started; only that PID), then the service starts from the new plist.
    launchctl bootout "gui/$(id -u)/$env_label" 2>/dev/null || true
    [[ -n "$p" ]] && { kill "$p" 2>/dev/null || true; for _ in $(seq 1 40); do kill -0 "$p" 2>/dev/null || break; sleep 0.25; done; }
    launchctl bootstrap "gui/$(id -u)" "$env_plist"
    say "resident daemon & workers at $ver_id ($env_label)"
  elif [[ -n "$p" ]]; then
    # No login service: Coder starts its own coder-host when none answers, so the old one only stops.
    kill "$p" 2>/dev/null || true
    for _ in $(seq 1 40); do kill -0 "$p" 2>/dev/null || break; sleep 0.25; done
    say "stopped the old resident daemon; OpenAgents starts $ver_id's when it opens (workers synced to $ver_id)"
  fi
}

# The clone this checkout belongs to (the LaunchAgent's copy of this script reads it from $state/clone).
here="$(cd "$(dirname "$0")" && pwd)"
if git -C "$here" rev-parse --git-dir > /dev/null 2>&1; then
  clone="$(cd "$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)/.." && pwd)"
  echo "$clone" > "$state/clone"
else
  clone="$(cat "$state/clone")"
fi

write_agent_plist() {
  mkdir -p "$HOME/Library/LaunchAgents"
  local extra_arg="<string>--full</string>" extra_env=$'\n    <key>OA_RELEASE_MINIMAL</key><string>0</string>'
  if [[ "$minimal" == 1 ]]; then
    extra_arg="<string>--minimal</string>"
    extra_env=$'\n    <key>OA_RELEASE_MINIMAL</key><string>1</string>'
  fi
  cat > "$agent_plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>/bin/bash</string><string>$state/install.sh</string><string>--check</string>${extra_arg}</array>
  <key>StartInterval</key><integer>900</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
  <key>EnvironmentVariables</key><dict>
    <key>PATH</key><string>/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:$HOME/.nix-profile/bin</string>
    <key>HOME</key><string>$HOME</string>${extra_env}
  </dict>
  <key>StandardOutPath</key><string>$state/log</string>
  <key>StandardErrorPath</key><string>$state/log</string>
</dict></plist>
PLIST
}

verify_minimal_bundle() {
  local bundle="$1"
  local check_repo="${build_src:-$src}"
  [[ -f "$check_repo/apps/openagents-desktop/v1-bundle-check.coax" ]] || check_repo="$repo_here"
  [[ -f "$check_repo/apps/openagents-desktop/v1-bundle-check.coax" ]] || check_repo="$clone"
  local v1_stage="${OA_V1_STAGE:-1}"
  if ! (cd "$check_repo" && coax/web/tool.sh apps/openagents-desktop/v1-bundle-check.coax --platform macos --stage "$v1_stage" "$bundle") >> "$state/build.log" 2>&1; then
    say "v1-bundle-check failed (stage $v1_stage): $state/build.log"
    return 1
  fi
}

retire_legacy_agent() {
  [[ -f "$legacy_agent_plist" ]] || return 0
  if [[ ! -f "$agent_plist" ]]; then
    write_agent_plist
    launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
    launchctl bootstrap "gui/$(id -u)" "$agent_plist" 2>/dev/null || true
  fi
  rm -f "$legacy_agent_plist"
  launchctl bootout "gui/$(id -u)/$legacy_label" 2>/dev/null || true
}

if [[ "$mode" == --remove-agent ]]; then
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootout "gui/$(id -u)/$legacy_label" 2>/dev/null || true
  rm -f "$agent_plist" "$legacy_agent_plist"
  say "checks stopped; $app stays"
  exit 0
fi

if [[ "$mode" == --status ]]; then
  git -C "$clone" fetch -q origin main 2>/dev/null || true
  main_rev="$((28830 + $(git -C "$clone" rev-list --count origin/main 2>/dev/null || echo 0)))"
  main_sha="$(git -C "$clone" rev-parse --short=7 origin/main 2>/dev/null || true)"
  echo "main:      1.0.0.${main_rev}.${main_sha}"
  echo "profile:   $want_profile"
  inst_id="$(version_of "$app")"
  inst_p="$(profile_of "$app")"
  inst_d="$(/usr/libexec/PlistBuddy -c 'Print :OABuildDate' "$app/Contents/Info.plist" 2>/dev/null || true)"
  echo "installed: ${inst_id}${inst_id:+ [$inst_p${inst_d:+, $inst_d}]} ($app)"
  wait_id="$(version_of "$stage/OpenAgents.app")"
  wait_p="$(profile_of "$stage/OpenAgents.app")"
  echo "waiting:   ${wait_id}${wait_id:+ [$wait_p]}"
  worker_v="$(cat "$HOME/.openagents/bin/.worker-version" 2>/dev/null || true)"
  echo "workers:   ${worker_v:-untracked}"
  echo "checked:   $(cut -c1-10 "$state/current" 2>/dev/null || true)"
  launchctl print "gui/$(id -u)/$label" > /dev/null 2>&1 && echo "checks:    every 15 minutes (log: $state/log)" || echo "checks:    off"
  exit 0
fi

if [[ "$mode" == --agent ]]; then
  cp "$0" "$state/install.sh.new" && mv "$state/install.sh.new" "$state/install.sh"
  write_agent_plist
  launchctl bootout "gui/$(id -u)/$legacy_label" 2>/dev/null || true
  rm -f "$legacy_agent_plist"
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$agent_plist"
  say "checks every 15 minutes ($want_profile); log at $state/log"
  exit 0
fi

# One run at a time, with dead-PID stale lock recovery.
if ! mkdir "$state/lock" 2>/dev/null; then
  lock_pid="$(cat "$state/lock/pid" 2>/dev/null || true)"
  if [[ -n "$lock_pid" && "$lock_pid" =~ ^[0-9]+$ ]] && ! kill -0 "$lock_pid" 2>/dev/null; then
    say "clearing stale lock from dead pid $lock_pid"
    rm -rf "$state/lock"
  fi
  if ! mkdir "$state/lock" 2>/dev/null; then
    say "another run is going (pid ${lock_pid:-unknown})"
    [[ "$mode" == --check ]] && exit 0 || exit 4
  fi
fi
printf '%s\n' "$$" > "$state/lock/pid"
trap 'rm -rf "$state/lock"' EXIT

if [[ "$mode" == --check ]]; then
  # A background check gives way to a severely overloaded Mac and to a nearly full disk.
  load="$(sysctl -n vm.loadavg | awk '{print int($2)}')"; cpus="$(sysctl -n hw.ncpu)"
  if (( load > cpus * 2 )); then say "load $load on $cpus cores: next check"; exit 0; fi
  free_gb="$(df -g "$state" | awk 'NR==2 {print $4}')"
  if (( free_gb < 10 )); then say "only ${free_gb} GB free: next check"; exit 0; fi
fi

repo_here="$(cd "$here/../.." && pwd)"
if [[ "$use_local" == 1 ]]; then
  target="$(git -C "$repo_here" rev-parse HEAD)"
else
  git -C "$clone" fetch -q origin main
  target="$(git -C "$clone" rev-parse origin/main)"
fi
installed="$(commit_of "$app")"
installed_profile="$(profile_of "$app")"
staged="$(commit_of "$stage/OpenAgents.app")"
staged_profile="$(profile_of "$stage/OpenAgents.app")"

# Built already, or nothing OpenAgents's build reads changed since the build we have.
have=""
if [[ -n "$staged" && "$staged_profile" == "$want_profile" ]]; then
  have="$staged"
elif [[ -n "$installed" && "$installed_profile" == "$want_profile" ]]; then
  have="$installed"
fi
cached_profile="$(cat "$state/profile" 2>/dev/null || echo full)"
cached_inputs_schema="$(cat "$state/inputs.version" 2>/dev/null || true)"
if [[ "$use_local" == 0 && "$force_build" == 0 && "$have" != "$target" && -n "$have" && "$cached_profile" == "$want_profile" && "$cached_inputs_schema" == "$inputs_schema" && -s "$state/inputs" ]] \
   && git -C "$clone" cat-file -e "$have^{commit}" 2>/dev/null \
   && xargs git -C "$clone" diff --quiet "$have" "$target" -- < "$state/inputs"; then
  echo "$target" > "$state/current"
  target="$have"
fi

need_build=1
if [[ "$use_local" == 0 && "$force_build" == 0 ]]; then
  if [[ "$staged" == "$target" && "$staged_profile" == "$want_profile" ]]; then need_build=0; fi
  if [[ "$installed" == "$target" && "$installed_profile" == "$want_profile" ]]; then need_build=0; fi
fi

if [[ "$need_build" == 1 ]]; then
  if [[ "$use_local" == 1 ]]; then
    build_src="$repo_here"
    say "building local ${target:0:10} ($want_profile) from $build_src"
  else
    say "building ${target:0:10} ($want_profile)"
    if [[ ! -e "$src/.git" ]]; then
      git -C "$clone" worktree prune
      git -C "$clone" worktree add -q --no-checkout --detach "$src" "$target"
    fi
    git -C "$src" checkout -q --force --detach "$target"
    git -C "$src" clean -q -fd -e coax/bin -e coax/web/build/tools || true
    build_src="$src"
  fi
  appdir="$(app_dir "$build_src")"
  fixed+=("$appdir")
  if [[ -f "$build_src/$appdir/desktop-core.coax" ]]; then
    roots+=("$appdir/desktop-core.coax")
  fi
  if [[ "$minimal" == 1 && -f "$build_src/$appdir/app-v1.coax" ]]; then
    roots+=("$appdir/app-v1.coax")
  else
    roots+=("$appdir/app.coax")
  fi
  (cd "$build_src" && coax/build.sh) > "$state/build.log" 2>&1 || { say "coax build failed: $state/build.log"; exit 1; }
  # Always remove any prior staged .app bundle before building so stale workers/resources from older
  # commits can never leak across builds even if building a commit with an older build.sh.
  rm -rf "$stage/OpenAgents.app"
  build_env=(CODER_BUILD="$stage")
  build_args=()
  if [[ "$minimal" == 1 ]]; then
    build_env+=(OA_RELEASE_MINIMAL=1)
    if grep -q -- '--minimal' "$build_src/$appdir/build.sh" 2>/dev/null; then
      build_args+=(--minimal)
    fi
  else
    build_env+=(OA_RELEASE_MINIMAL=0)
    if grep -q -- '--full' "$build_src/$appdir/build.sh" 2>/dev/null; then
      build_args+=(--full)
    fi
  fi
  env "${build_env[@]}" "$build_src/$appdir/build.sh" ${build_args[@]+"${build_args[@]}"} >> "$state/build.log" 2>&1 || { say "OpenAgents build failed: $state/build.log"; exit 1; }
  plist="$stage/OpenAgents.app/Contents/Info.plist"
  rev_count="$((28830 + $(git -C "$build_src" rev-list --count HEAD)))"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $rev_count" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OACommit" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OACommit string $target" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OAProfile" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OAProfile string $want_profile" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OABuildDate" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OABuildDate string $(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$plist"
  codesign --force --sign - "$stage/OpenAgents.app" >> "$state/build.log" 2>&1
  if [[ "$minimal" == 1 ]]; then
    if ! verify_minimal_bundle "$stage/OpenAgents.app"; then
      rm -rf "$stage/OpenAgents.app"
      exit 1
    fi
  fi
  # What this build read, relative to the checkout, for the next check.
  if [[ "$use_local" == 0 ]]; then
    { printf '%s\n' "${fixed[@]}"
      for r in "${roots[@]}"; do [[ -f "$build_src/$r" ]] && "$build_src/coax/bin/coax" deps "$build_src/$r"; done \
        | python3 -I -c 'import json, sys, os
root = os.path.realpath(sys.argv[1]) + "/"
for line in sys.stdin:
    for f in json.loads(line).get("sources", []):
        f = os.path.realpath(f)
        if f.startswith(root): print(f[len(root):])' "$build_src"
    } | sort -u > "$state/inputs.new" && mv "$state/inputs.new" "$state/inputs"
    printf '%s\n' "$inputs_schema" > "$state/inputs.version"
    echo "$target" > "$state/current"
    echo "$want_profile" > "$state/profile"
  fi
  # The LaunchAgent runs the newest copy of this script from here on.
  [[ ! -f "$build_src/$appdir/install.sh" ]] || { cp "$build_src/$appdir/install.sh" "$state/install.sh.new" && mv "$state/install.sh.new" "$state/install.sh"; }
  staged="$target"
  staged_profile="$want_profile"
fi

target_id="$(version_of "$stage/OpenAgents.app")"
[[ -n "$target_id" ]] || target_id="$(version_of "$app")"
[[ -n "$target_id" ]] || target_id="${target:0:10}"

[[ "$use_local" == 0 && "$force_build" == 0 && "$installed" == "$target" && "$installed_profile" == "$want_profile" && ! -e "$legacy_app" ]] && { say "up to date at $target_id ($want_profile)"; sync_resident; retire_legacy_agent; exit 0; }

pids="$(coder_pids)"
if [[ -n "$pids" ]]; then
  if [[ "$mode" == --check ]]; then say "$target_id is built and installs when you quit OpenAgents"; sync_resident "$stage/OpenAgents.app"; retire_legacy_agent; exit 0; fi
  if inside_app $pids; then say "DEFERRED: $target_id is built and staged at $stage/OpenAgents.app (click 'Restart to update' in OpenAgents, or quit OpenAgents and re-run install.sh)"; sync_resident "$stage/OpenAgents.app"; retire_legacy_agent; exit 4; fi
  if [[ -z "${OA_INSTALL_FORCE:-}" ]] && replies_running; then say "DEFERRED: $target_id is built, but a reply is running in OpenAgents; re-run with OA_INSTALL_FORCE=1 to quit and install now"; retire_legacy_agent; exit 4; fi
  kill $pids 2>/dev/null || true
  for _ in $(seq 1 40); do [[ -z "$(coder_pids)" ]] && break; sleep 0.25; done
  if [[ -n "$(coder_pids)" ]]; then
    say "OpenAgents is still running; quit it before installing so its bundle is not removed out from under it"
    exit 1
  fi
fi

if [[ "$minimal" == 1 ]]; then
  verify_minimal_bundle "$stage/OpenAgents.app" || exit 1
fi

# Swap it in whole: always clean any leftover staging/backup paths first so ditto never merges into stale files.
rm -rf "$dest/OpenAgents.app.new" "$dest/.OpenAgents.app.old"
ditto "$stage/OpenAgents.app" "$dest/OpenAgents.app.new"
[[ -e "$app" ]] && mv "$app" "$dest/.OpenAgents.app.old"
mv "$dest/OpenAgents.app.new" "$app"
rm -rf "$dest/.OpenAgents.app.old"
# Cleanly remove legacy /Applications/Coder.app after OpenAgents.app is in place.
if [[ -e "$legacy_app" ]]; then
  rm -rf "$dest/.Coder.app.old"
  mv "$legacy_app" "$dest/.Coder.app.old"
  rm -rf "$dest/.Coder.app.old"
fi
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" 2>/dev/null || true
installed_now="$(commit_of "$app")"
installed_profile_now="$(profile_of "$app")"
if [[ "$installed_now" != "$target" || "$installed_profile_now" != "$want_profile" ]]; then
  say "ERROR: post-install metadata mismatch (got ${installed_now:-none} [${installed_profile_now:-none}], expected $target [$want_profile])"
  exit 1
fi
say "installed $(version_of "$app") ($want_profile) at $app"
sync_resident
[[ -n "$pids" || "$mode" == now ]] && open "$app"
retire_legacy_agent
exit 0
