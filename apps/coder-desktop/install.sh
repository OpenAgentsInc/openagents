#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around git, build.sh, PlistBuddy, codesign and launchctl; holds no logic)
# Keeps /Applications/Coder.app at the newest origin/main.
#   apps/coder-desktop/install.sh                 build origin/main and install it now; a Coder open from
#                                                 /Applications is quit first and opened again after
#   apps/coder-desktop/install.sh --agent         also check every 15 minutes (a LaunchAgent), and run once now
#   apps/coder-desktop/install.sh --remove-agent  stop the checks
#   apps/coder-desktop/install.sh --check         what the LaunchAgent runs: build when main moved, install
#                                                 only while Coder is closed (the build waits beside it until then)
#   apps/coder-desktop/install.sh --status        what is installed, what waits, what main is at
# It builds in a checkout of its own under ~/.cache/openagents/coder-release, never in yours.
# Each build records what it read: the Coax sources of the page, the shader and the two workers
# (coax deps), and the folders of the compiler, the C, Swift and Objective-C hosts and the
# assets. A main that moved without changing any of them is not built again, since the
# installed Coder is already that commit's Coder. (A new #load needs an edit to a file the build
# already read, so the list can't miss one.)
# The installed app carries its commit (OACommit in Info.plist) and commit count (CFBundleVersion).
# The resident daemon (`coder host serve`, the program behind ~/.openagents/bin/openagents-environment and
# the login service com.openagents.coax.environment) is kept at the installed Coder's coder-host too: one
# binary, one resident service. It is restarted only while it runs no work (GitHub #13757).
set -euo pipefail
export LC_ALL=C BROWSER=/usr/bin/true
state="${CODER_RELEASE_HOME:-$HOME/.cache/openagents/coder-release}"
src="$state/src"
stage="$state/stage"
dest="${CODER_APP_DIR:-/Applications}"
app="$dest/Coder.app"
label="com.openagents.coder-update"
agent_plist="$HOME/Library/LaunchAgents/$label.plist"
# What every build reads besides the Coax sources coax deps lists: the compiler's inputs (as
# coax/provenance.sh lists them), what coax/web/tool.sh links into the workers, and the app's
# hosts and assets.
fixed=(coax/src coax/bootstrap coax/lib/compiler coax/lib/proto ':(glob)coax/*' scripts/coax-release.sh coax/lib/crypto/sha256-kernel.coax
       coax/platform coax/web/tool.sh coax/lib/crypto/entropy.coax coax/lib/sys coax/lib/net
       apps/coder-desktop apps/shared/host apps/shared/native components/zeron apps/openagents-desktop/native)
roots=(apps/coder-desktop/app.coax components/backend/gpu-shader.coax coder/tui/mcworker.coax coder/tui/claudeworker.coax coder/tui/gitguard.coax)
mode="${1:-now}"
mkdir -p "$state"

say() { echo "$(date '+%Y-%m-%d %H:%M:%S') coder-release: $*"; }
commit_of() { /usr/libexec/PlistBuddy -c "Print :OACommit" "$1/Contents/Info.plist" 2>/dev/null || true; }
coder_pids() { pgrep -f "^$app/Contents/MacOS/Coder" || true; }

# The resident daemon: one program (coder/tui/host.coax), shipped in Coder as coder-host and run at login
# by com.openagents.coax.environment from ~/.openagents/bin/openagents-environment.
env_label="${CODER_ENV_LABEL:-com.openagents.coax.environment}"
env_plist="$HOME/Library/LaunchAgents/$env_label.plist"
env_link="$HOME/.openagents/bin/openagents-environment"
host_sock="$HOME/.openagents/coder/host/service.sock"
sock_pid() { lsof -U 2>/dev/null | awk -v s="$host_sock" '$NF == s {print $2; exit}'; }
# The daemon is busy while it has a child (a worker run); a restart then waits for a later check.
daemon_busy() { local p; p="$(sock_pid)"; [[ -n "$p" ]] && [[ -n "$(pgrep -P "$p" || true)" ]]; }
sync_resident() {
  local bundled="$app/Contents/Resources/coder-host" commit dir want
  [[ -x "$bundled" ]] || return 0
  commit="$(commit_of "$app" | cut -c1-16)"; [[ -n "$commit" ]] || return 0
  dir="$HOME/.openagents/environment/bin/$commit"
  if ! cmp -s "$bundled" "$dir/openagents-environment" 2>/dev/null; then
    mkdir -p "$dir" && cp "$bundled" "$dir/openagents-environment.new" && mv "$dir/openagents-environment.new" "$dir/openagents-environment"
  fi
  mkdir -p "$(dirname "$env_link")"
  [[ "$(readlink "$env_link" 2>/dev/null)" == "$dir/openagents-environment" ]] || ln -sfn "$dir/openagents-environment" "$env_link"
  # Which binary answers now: the login service's (through the link) or a coder-host Coder started.
  local p exe; p="$(sock_pid)"; exe="$( [[ -n "$p" ]] && ps -o comm= -p "$p" || true)"
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

if [[ "$mode" == --remove-agent ]]; then
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  rm -f "$agent_plist"
  say "checks stopped; $app stays"
  exit 0
fi

if [[ "$mode" == --status ]]; then
  git -C "$clone" fetch -q origin main 2>/dev/null || true
  echo "main:      $(git -C "$clone" rev-parse --short origin/main)"
  echo "installed: $(commit_of "$app" | cut -c1-10) ($app)"
  echo "waiting:   $(commit_of "$stage/Coder.app" | cut -c1-10)"
  echo "checked:   $(cut -c1-10 "$state/current" 2>/dev/null || true)"
  launchctl print "gui/$(id -u)/$label" > /dev/null 2>&1 && echo "checks:    every 15 minutes (log: $state/log)" || echo "checks:    off"
  exit 0
fi

if [[ "$mode" == --agent ]]; then
  cp "$0" "$state/install.sh.new" && mv "$state/install.sh.new" "$state/install.sh"
  mkdir -p "$HOME/Library/LaunchAgents"
  cat > "$agent_plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>/bin/bash</string><string>$state/install.sh</string><string>--check</string></array>
  <key>StartInterval</key><integer>900</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
  <key>EnvironmentVariables</key><dict>
    <key>PATH</key><string>/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:$HOME/.nix-profile/bin</string>
    <key>HOME</key><string>$HOME</string>
  </dict>
  <key>StandardOutPath</key><string>$state/log</string>
  <key>StandardErrorPath</key><string>$state/log</string>
</dict></plist>
PLIST
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$agent_plist"
  say "checks every 15 minutes; log at $state/log"
  exit 0
fi

# One run at a time.
if ! mkdir "$state/lock" 2>/dev/null; then say "another run is going"; exit 0; fi
trap 'rmdir "$state/lock"' EXIT

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
staged="$(commit_of "$stage/Coder.app")"

# Built already, or nothing Coder's build reads changed since the build we have.
have="$staged"; [[ -n "$have" ]] || have="$installed"
if [[ "$have" != "$target" && -n "$have" && -s "$state/inputs" ]] && git -C "$clone" cat-file -e "$have^{commit}" 2>/dev/null \
   && xargs git -C "$clone" diff --quiet "$have" "$target" -- < "$state/inputs"; then
  echo "$target" > "$state/current"
  target="$have"
fi

if [[ "$staged" != "$target" && "$installed" != "$target" ]]; then
  say "building ${target:0:10}"
  if [[ ! -e "$src/.git" ]]; then
    git -C "$clone" worktree prune
    git -C "$clone" worktree add -q --no-checkout --detach "$src" "$target"
  fi
  git -C "$src" checkout -q --force --detach "$target"
  (cd "$src" && coax/build.sh) > "$state/build.log" 2>&1 || { say "coax build failed: $state/build.log"; exit 1; }
  CODER_BUILD="$stage" "$src/apps/coder-desktop/build.sh" >> "$state/build.log" 2>&1 || { say "Coder build failed: $state/build.log"; exit 1; }
  plist="$stage/Coder.app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $(git -C "$src" rev-list --count HEAD)" "$plist"
  /usr/libexec/PlistBuddy -c "Delete :OACommit" "$plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OACommit string $target" "$plist"
  codesign --force --sign - "$stage/Coder.app" 2> /dev/null
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
  # The LaunchAgent runs the newest copy of this script from here on.
  [[ ! -f "$src/apps/coder-desktop/install.sh" ]] || { cp "$src/apps/coder-desktop/install.sh" "$state/install.sh.new" && mv "$state/install.sh.new" "$state/install.sh"; }
  staged="$target"
fi

[[ "$installed" == "$target" ]] && { say "up to date at ${target:0:10}"; sync_resident; exit 0; }

pids="$(coder_pids)"
if [[ -n "$pids" ]]; then
  if [[ "$mode" == --check ]]; then say "${target:0:10} is built and installs when you quit Coder"; exit 0; fi
  kill $pids 2>/dev/null || true
  for _ in $(seq 1 40); do [[ -z "$(coder_pids)" ]] && break; sleep 0.25; done
  if [[ -n "$(coder_pids)" ]]; then
    say "Coder is still running; quit it before installing so its bundle is not removed out from under it"
    exit 1
  fi
fi

# Swap it in whole: the new copy lands beside the old one, then takes its name.
ditto "$stage/Coder.app" "$dest/Coder.app.new"
[[ -e "$app" ]] && mv "$app" "$dest/.Coder.app.old"
mv "$dest/Coder.app.new" "$app"
rm -rf "$dest/.Coder.app.old"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" 2>/dev/null || true
say "installed ${target:0:10} at $app"
sync_resident
[[ -n "$pids" || "$mode" == now ]] && open "$app"
exit 0
