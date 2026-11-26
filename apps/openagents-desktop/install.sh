#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around git, build.sh, PlistBuddy, codesign and launchctl; holds no logic)
# Keeps /Applications/OpenAgents.app at the newest origin/main.
#   apps/openagents-desktop/install.sh [--minimal|--full]          build origin/main and install it now (defaults to --minimal); an OpenAgents
#                                                                  (or legacy Coder) open from /Applications is quit first and opened again after.
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
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    now|--agent|--remove-agent|--check|--status) mode="$1" ;;
    *) echo "install.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done
want_profile="full"
[[ "$minimal" == 1 ]] && want_profile="minimal"
# What every build reads besides the Coax sources coax deps lists: the compiler's inputs (as
# coax/provenance.sh lists them), what coax/web/tool.sh links into the workers, and the app's
# hosts and assets.
fixed=(coax/src coax/bootstrap coax/lib/compiler coax/lib/proto ':(glob)coax/*' scripts/coax-release.sh coax/lib/crypto/sha256-kernel.coax
       coax/platform coax/web/tool.sh coax/lib/crypto/entropy.coax coax/lib/sys coax/lib/net
       apps/shared/host apps/shared/native components/chat components/fonts components/icons components/NOTICE apps/openagents-desktop/native psionic/platform psionic/lib/inkling)
if [[ "$minimal" == 1 ]]; then
  roots=(components/backend/gpu-shader.coax coder/tui/host.coax coder/tui/oa-worker.coax coder/tui/inky.coax coder/tui/agyworker.coax coder/tui/gitguard.coax)
else
  roots=(components/backend/gpu-shader.coax coder/tui/host.coax coder/tui/oa-worker.coax coder/tui/inky.coax coder/tui/mcworker.coax coder/tui/claudeworker.coax coder/tui/agyworker.coax coder/tui/gitguard.coax coder/tui/lean-run.coax metaverse/rh-flythrough.coax)
fi
mkdir -p "$state"

say() { echo "$(date '+%Y-%m-%d %H:%M:%S') coder-release: $*"; }
commit_of() { /usr/libexec/PlistBuddy -c "Print :OACommit" "$1/Contents/Info.plist" 2>/dev/null || true; }
profile_of() {
  [[ -f "$1/Contents/Info.plist" ]] || return 0
  local p
  p="$(/usr/libexec/PlistBuddy -c "Print :OAProfile" "$1/Contents/Info.plist" 2>/dev/null || true)"
  [[ -n "$p" ]] && echo "$p" || echo "full"
}
# The app's folder in a checkout: apps/openagents-desktop once the Phase 2 move lands (#13780), apps/coder-desktop
# before it. Checked per build, so this script keeps working across the move.
app_dir() { if [[ -f "$1/apps/openagents-desktop/app.coax" ]]; then echo apps/openagents-desktop; else echo apps/coder-desktop; fi; }
# True when this script runs inside one of the given app processes (an agent in OpenAgents ran it): quitting the
# app would kill this run with it, half done.
inside_app() {
  local p=$$ pids=" $* "
  while [[ -n "$p" && "$p" -gt 1 ]]; do
    [[ "$pids" == *" $p "* ]] && return 0
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
# The process that holds the socket and whose parent does not: a worker run inherits the descriptor.
# awk reads all of lsof's output (an early exit would end lsof by SIGPIPE, which pipefail makes fatal).
sock_pid() {
  local all p
  all="$(lsof -U 2>/dev/null | awk -v s="$host_sock" '$NF == s {print $2}' | sort -u)"
  for p in $all; do
    grep -qx "$(ps -o ppid= -p "$p" | tr -d ' ')" <<< "$all" || { echo "$p"; return 0; }
  done
}
# The daemon is busy while it has a child (a worker run); a restart then waits for a later check.
daemon_busy() { local p; p="$(sock_pid)"; [[ -n "$p" ]] && [[ -n "$(pgrep -P "$p" || true)" ]]; }
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
# It runs the given app's coder-host (the installed app's by default): a newer build waiting for
# OpenAgents to quit hands its daemon over first, so a paired phone gets it without that quit.
sync_resident() {
  local from="${1:-$app}"
  local bundled="$from/Contents/Resources/coder-host" commit dir want
  [[ -x "$bundled" ]] || return 0
  commit="$(commit_of "$from" | cut -c1-16)"; [[ -n "$commit" ]] || return 0
  dir="$HOME/.openagents/environment/bin/$commit"
  if ! cmp -s "$bundled" "$dir/openagents-environment" 2>/dev/null; then
    mkdir -p "$dir" && cp "$bundled" "$dir/openagents-environment.new" && mv "$dir/openagents-environment.new" "$dir/openagents-environment"
  fi
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
  if daemon_busy; then say "the resident daemon runs work; it moves to ${commit:0:10} at a later check"; return 0; fi
  if [[ -f "$env_plist" ]]; then
    # The login service stops (launchd would restart it otherwise), then whatever still holds the
    # socket (a coder-host Coder started; only that PID), then the service starts from the new plist.
    launchctl bootout "gui/$(id -u)/$env_label" 2>/dev/null || true
    [[ -n "$p" ]] && { kill "$p" 2>/dev/null || true; for _ in $(seq 1 40); do kill -0 "$p" 2>/dev/null || break; sleep 0.25; done; }
    launchctl bootstrap "gui/$(id -u)" "$env_plist"
    say "resident daemon at ${commit:0:10} ($env_label)"
  elif [[ -n "$p" ]]; then
    # No login service: Coder starts its own coder-host when none answers, so the old one only stops.
    kill "$p" 2>/dev/null || true
    for _ in $(seq 1 40); do kill -0 "$p" 2>/dev/null || break; sleep 0.25; done
    say "stopped the old resident daemon; Coder starts ${commit:0:10}'s when it opens"
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
  local check_repo="$src"
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
  echo "main:      $(git -C "$clone" rev-parse --short origin/main)"
  echo "profile:   $want_profile"
  inst_c="$(commit_of "$app" | cut -c1-10)"
  inst_p="$(profile_of "$app")"
  echo "installed: ${inst_c}${inst_c:+ [$inst_p]} ($app)"
  wait_c="$(commit_of "$stage/OpenAgents.app" | cut -c1-10)"
  wait_p="$(profile_of "$stage/OpenAgents.app")"
  echo "waiting:   ${wait_c}${wait_c:+ [$wait_p]}"
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
  # A background check gives way to a busy Mac (a build is heavy) and to a nearly full disk.
  load="$(sysctl -n vm.loadavg | awk '{print int($2)}')"; cpus="$(sysctl -n hw.ncpu)"
  if (( load * 4 > cpus * 3 )); then say "load $load on $cpus cores: next check"; exit 0; fi
  free_gb="$(df -g "$state" | awk 'NR==2 {print $4}')"
  if (( free_gb < 10 )); then say "only ${free_gb} GB free: next check"; exit 0; fi
fi

git -C "$clone" fetch -q origin main
target="$(git -C "$clone" rev-parse origin/main)"
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
if [[ "$have" != "$target" && -n "$have" && "$cached_profile" == "$want_profile" && -s "$state/inputs" ]] \
   && git -C "$clone" cat-file -e "$have^{commit}" 2>/dev/null \
   && xargs git -C "$clone" diff --quiet "$have" "$target" -- < "$state/inputs"; then
  echo "$target" > "$state/current"
  target="$have"
fi

need_build=1
if [[ "$staged" == "$target" && "$staged_profile" == "$want_profile" ]]; then need_build=0; fi
if [[ "$installed" == "$target" && "$installed_profile" == "$want_profile" ]]; then need_build=0; fi

if [[ "$need_build" == 1 ]]; then
  say "building ${target:0:10} ($want_profile)"
  if [[ ! -e "$src/.git" ]]; then
    git -C "$clone" worktree prune
    git -C "$clone" worktree add -q --no-checkout --detach "$src" "$target"
  fi
  git -C "$src" checkout -q --force --detach "$target"
  appdir="$(app_dir "$src")"
  fixed+=("$appdir")
  if [[ "$minimal" == 1 && -f "$src/$appdir/app-v1.coax" ]]; then
    roots+=("$appdir/app-v1.coax")
  else
    roots+=("$appdir/app.coax")
  fi
  (cd "$src" && coax/build.sh) > "$state/build.log" 2>&1 || { say "coax build failed: $state/build.log"; exit 1; }
  build_env=(CODER_BUILD="$stage")
  build_args=()
  if [[ "$minimal" == 1 ]]; then
    build_env+=(OA_RELEASE_MINIMAL=1)
    if grep -q -- '--minimal' "$src/$appdir/build.sh" 2>/dev/null; then
      build_args+=(--minimal)
    fi
  else
    build_env+=(OA_RELEASE_MINIMAL=0)
    if grep -q -- '--full' "$src/$appdir/build.sh" 2>/dev/null; then
      build_args+=(--full)
    fi
  fi
  env "${build_env[@]}" "$src/$appdir/build.sh" ${build_args[@]+"${build_args[@]}"} >> "$state/build.log" 2>&1 || { say "OpenAgents build failed: $state/build.log"; exit 1; }
  plist="$stage/OpenAgents.app/Contents/Info.plist"
  rev_count="$((28830 + $(git -C "$src" rev-list --count HEAD)))"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $rev_count" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OACommit" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OACommit string $target" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OAProfile" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OAProfile string $want_profile" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OABuildDate" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OABuildDate string $(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$plist"
  codesign --force --sign - "$stage/OpenAgents.app" 2> /dev/null
  if [[ "$minimal" == 1 ]]; then
    if ! verify_minimal_bundle "$stage/OpenAgents.app"; then
      rm -rf "$stage/OpenAgents.app"
      exit 1
    fi
  fi
  # What this build read, relative to the checkout, for the next check.
  { printf '%s\n' "${fixed[@]}"
    for r in "${roots[@]}"; do "$src/coax/bin/coax" deps "$src/$r"; done \
      | python3 -I -c 'import json, sys, os
root = os.path.realpath(sys.argv[1]) + "/"
for line in sys.stdin:
    for f in json.loads(line).get("sources", []):
        f = os.path.realpath(f)
        if f.startswith(root): print(f[len(root):])' "$src"
  } | sort -u > "$state/inputs.new" && mv "$state/inputs.new" "$state/inputs"
  echo "$target" > "$state/current"
  echo "$want_profile" > "$state/profile"
  # The LaunchAgent runs the newest copy of this script from here on.
  [[ ! -f "$src/$appdir/install.sh" ]] || { cp "$src/$appdir/install.sh" "$state/install.sh.new" && mv "$state/install.sh.new" "$state/install.sh"; }
  staged="$target"
  staged_profile="$want_profile"
fi

[[ "$installed" == "$target" && "$installed_profile" == "$want_profile" && ! -e "$legacy_app" ]] && { say "up to date at ${target:0:10} ($want_profile)"; sync_resident; retire_legacy_agent; exit 0; }

pids="$(coder_pids)"
if [[ -n "$pids" ]]; then
  if [[ "$mode" == --check ]]; then say "${target:0:10} is built and installs when you quit OpenAgents"; sync_resident "$stage/OpenAgents.app"; retire_legacy_agent; exit 0; fi
  if inside_app $pids; then say "DEFERRED: ${target:0:10} is built, but this run is inside OpenAgents; quit OpenAgents and re-run install.sh (or wait for the next check)"; retire_legacy_agent; exit 4; fi
  if [[ -z "${OA_INSTALL_FORCE:-}" ]] && replies_running; then say "DEFERRED: ${target:0:10} is built, but a reply is running in OpenAgents; re-run with OA_INSTALL_FORCE=1 to quit and install now"; retire_legacy_agent; exit 4; fi
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

# Swap it in whole: the new copy lands beside the old one, then takes its name.
ditto "$stage/OpenAgents.app" "$dest/OpenAgents.app.new"
[[ -e "$app" ]] && mv "$app" "$dest/.OpenAgents.app.old"
mv "$dest/OpenAgents.app.new" "$app"
rm -rf "$dest/.OpenAgents.app.old"
# Cleanly remove legacy /Applications/Coder.app after OpenAgents.app is in place.
if [[ -e "$legacy_app" ]]; then
  mv "$legacy_app" "$dest/.Coder.app.old"
  rm -rf "$dest/.Coder.app.old"
fi
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" 2>/dev/null || true
installed_now="$(commit_of "$app")"
if [[ "$installed_now" != "$target" ]]; then
  say "ERROR: post-install OACommit mismatch (got ${installed_now:-none}, expected $target)"
  exit 1
fi
say "installed ${target:0:10} ($want_profile, build $(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Contents/Info.plist" 2>/dev/null || echo ?)) at $app"
sync_resident
[[ -n "$pids" || "$mode" == now ]] && open "$app"
retire_legacy_agent
exit 0
