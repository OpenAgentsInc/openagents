#!/usr/bin/env bash
# The release acceptance gate: the owner's real flows, end to end, on the
# exact build under test (#10080). docs/release/acceptance.md has the full
# runbook and the scenario list.
#
#   scripts/release/acceptance.sh [--app PATH | --bin-dir DIR] [--evidence DIR]
#                                 [--only NAME[,NAME...]] [--allow-missing-engine] [--no-engines]
#                                 [--keep] [--list]
#
# What it runs, never touching the real home or its stores:
#   1. A temporary HOME (short, under /tmp, for the control socket's path
#      limit), the owner's Codex, Claude Code, and Grok Build logins made
#      readable there
#      without copying or changing them (see "Engine logins" below), and a
#      scratch Git repository with a linked worktree of it, the shape of the
#      owner's ~/work/openagents-host-tasks, and a local bare repository as
#      its `origin` (push-main pushes there, never to a real remote).
#   2. The build's own `coder host serve --control` in that HOME, the
#      worktree registered as its project (`project_add`, as the app's
#      folder picker sends) and auto-start on (`autostart_set`, as the app's
#      switch sends).
#   3. The build's own desktop binary in its headless acceptance mode
#      (`--acceptance DIR`): the real window model, run inline against that
#      host's control socket, sending each scenario's message the way the
#      desktop sends it (the host adds the desktop surface context, #10077),
#      letting Coder start, run, and finish on real engines in the scratch
#      project, and asserting the window's view tree and captures. The phone
#      scenarios pair a phone-shaped NIP-HOST client with the host:
#      phone-claude presses Run Coder as the phone does; phone-start-at-once
#      runs the phone's own Coder tab and checks a coding reply starts Coder
#      with no tap; phone-agents asks "what coding agents are connected?"
#      and checks the reply names each agent the computer's presence says
#      is ready.
#   4. The gate's own scenarios, run with the build's binaries outside the
#      window: explain-error runs the Explain this error plugin on a planted
#      failure through `openagents plugin run`; plugins-chat asks the live
#      chat "which plugins can I test?" through `openagents chat --scratch`
#      and checks the reply names every plugin in deploy/eval-runner/catalog;
#      essays-chat asks the live chat two questions about our essays and to
#      summarize both, and checks each reply is grounded in them (the
#      summary names both essays and is not a dispatch); phone-sim-start
#      pairs the actual iOS app, in a simulator the gate creates, with the
#      host and checks a coding question from its Chat tab starts Coder
#      with no tap (OPENAGENTS_ACCEPTANCE_IOS_APP: a built simulator app).
#   5. A PASS/FAIL line per scenario, a summary table, and the evidence
#      directory. Exit status 1 when any scenario fails, 2 on a setup error.
#
# Options:
#   --app PATH         An assembled OpenAgents.app: Contents/MacOS/{OpenAgents,
#                      coder,microcoder} and Contents/Helpers/openagents.
#                      Default: $CARGO_TARGET_DIR/desktop-release/OpenAgents.app
#                      (or target/desktop-release/OpenAgents.app), the .app
#                      scripts/desktop/package-macos.sh just packaged.
#   --bin-dir DIR      Take openagents-desktop, coder, microcoder, and
#                      openagents from DIR instead (for example
#                      $CARGO_TARGET_DIR/debug after `cargo build -p coder -p
#                      microcoder -p openagents-cli -p openagents-desktop`).
#   --evidence DIR     Where results, captures, transcripts, and logs go
#                      (default: a new directory under $TMPDIR).
#   --only NAMES       Run only these scenarios (comma-separated); --list
#                      prints them.
#   --allow-missing-engine
#                      A missing Codex, Claude Code, or Grok Build login
#                      skips the scenarios that need it instead of failing
#                      them.
#   --no-engines       Read no engine login at all (no Codex copy, no
#                      Keychain link) and skip the scenarios that need one:
#                      for the UI and chat scenarios alone.
#   --keep             Keep the temporary HOME (its path is printed).
#   -h, --help         This text.
#
# Engine logins (read-only, the owner-local smoke approach):
#   Codex       CODEX_HOME is the temporary HOME's .codex, holding a private
#               copy (mode 0600) of ~/.codex/auth.json, deleted with the
#               temporary HOME. Coder's Codex transport reads the login and
#               never refreshes it, so the real login is never changed. It
#               is a copy, not a link, so nothing a run does reaches the
#               real file (a take removes only a run's own copy and refuses
#               a link, #10083).
#   Claude Code The temporary HOME's Library/Keychains is a symbolic link to
#               the real one, so `claude` reads its `Claude Code-credentials`
#               item as it always does, and ~/.claude.json gets only the
#               account metadata (oauthAccount, userID, onboarding), never a
#               credential.
#   Grok Build  GROK_BIN names the owner's installed `grok`, and the
#               temporary HOME's .grok holds a private copy (mode 0600) of
#               ~/.grok/auth.json (or $GROK_HOME/auth.json), deleted with the
#               temporary HOME; XAI_API_KEY, when set, is used as it is
#               instead. Grok Build refreshes its sign-in only near its
#               expiry, and a refresh could retire the owner's refresh
#               token, so the copy is made only while the login has more
#               than an hour left (the gate takes under half that); with
#               less, run `grok` once to refresh it, then the gate. Only
#               that file is copied; the real one is never changed.
#   Only the Codex and Grok Build login files are copied, and nothing is
#   printed.
#
# Spend: each Coder run is a tiny task in the scratch repository that Coder
# finishes in a few steps (runs have no step limit, #10103); the chat router
# answers a fixed handful of messages. A full run costs a few cents of engine usage.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"

app=""
bin_dir=""
evidence=""
only=""
allow_missing=0
keep=0

usage() { sed -n '2,/^set -uo/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//'; }
die() { echo "acceptance: $*" >&2; exit 2; }
say() { echo "==> $*" >&2; }

# The desktop driver's scenarios, then the gate's own: ones this script
# runs itself with the build's binaries, outside the desktop window.
desktop_scenarios="who-are-you working-directory delegate-who delegate-now followup-chat followup-coder delegate-claude delegate-grok push-main ui-stop-coder ui-no-attach open-deck phone-claude phone-start-at-once phone-closed-loop phone-dead-task phone-agents ui-no-verse ui-placeholder ui-starter-chips ui-engines-sidebar ui-new-chat-top ui-filter-sessions ui-chips route-map route-map-chat"
gate_scenarios="explain-error plugins-chat essays-chat phone-sim-start"
scenarios="$desktop_scenarios $gate_scenarios"

while [ $# -gt 0 ]; do
  case "$1" in
    --app) app="${2:?--app needs a path}"; shift 2 ;;
    --bin-dir) bin_dir="${2:?--bin-dir needs a directory}"; shift 2 ;;
    --evidence) evidence="${2:?--evidence needs a directory}"; shift 2 ;;
    --only) only="${2:?--only needs scenario names}"; shift 2 ;;
    --allow-missing-engine) allow_missing=1; shift ;;
    --no-engines) no_engines=1; allow_missing=1; shift ;;
    --keep) keep=1; shift ;;
    --list) for s in $scenarios; do echo "$s"; done; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option $1 (see --help)" ;;
  esac
done

[ "$(uname -s)" = "Darwin" ] || die "the gate runs on a Mac (the desktop app's platform)"
command -v python3 >/dev/null || die "python3 is required"
command -v git >/dev/null || die "git is required"

if [ -n "$only" ]; then
  for name in ${only//,/ }; do
    case " $scenarios " in *" $name "*) ;; *) die "unknown scenario $name (see --list)" ;; esac
  done
fi

# The binaries under test.
if [ -n "$app" ] && [ -n "$bin_dir" ]; then
  die "pass --app or --bin-dir, not both"
fi
if [ -z "$app" ] && [ -z "$bin_dir" ]; then
  app="$target/desktop-release/OpenAgents.app"
  [ -d "$app" ] || die "no --app or --bin-dir, and no packaged app at $app"
fi
if [ -n "$app" ]; then
  app="$(cd "$app" && pwd)"
  desktop="$app/Contents/MacOS/OpenAgents"
  coder="$app/Contents/MacOS/coder"
  microcoder="$app/Contents/MacOS/microcoder"
  openagents="$app/Contents/Helpers/openagents"
else
  bin_dir="$(cd "$bin_dir" && pwd)"
  desktop="$bin_dir/openagents-desktop"
  coder="$bin_dir/coder"
  microcoder="$bin_dir/microcoder"
  openagents="$bin_dir/openagents"
fi
for binary in "$desktop" "$coder" "$microcoder" "$openagents"; do
  [ -x "$binary" ] || die "missing executable $binary"
done

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
if [ -z "$evidence" ]; then
  evidence="${TMPDIR:-/tmp}/openagents-acceptance-$stamp"
fi
mkdir -p "$evidence" || die "cannot create $evidence"
evidence="$(cd "$evidence" && pwd)"
: > "$evidence/results.jsonl"

real_home="$HOME"
# launchd's default soft limit for a login agent or a Finder-opened app.
file_limit=256
# Short, for the control socket's 104-byte path limit:
# $HOME/Library/Application Support/OpenAgents/control.sock.
scratch="$(mktemp -d /tmp/oaacc.XXXXXX)" || die "cannot make a temporary HOME"
scratch="$(cd "$scratch" && pwd -P)"
host_pid=""

cleanup() {
  # Coder runs the scenarios started (the phone's runs the host started
  # detached) end on their own; give them a bounded while, then stop them.
  local waited=0
  while pgrep -f "$scratch/" >/dev/null 2>&1 && [ "$waited" -lt 240 ]; do
    others="$(pgrep -f "$scratch/" | grep -v "^${host_pid:-x}\$" || true)"
    [ -z "$others" ] && break
    sleep 2; waited=$((waited + 2))
  done
  pgrep -f "$scratch/" | grep -v "^${host_pid:-x}\$" | xargs kill 2>/dev/null || true
  if [ -n "$host_pid" ] && kill -0 "$host_pid" 2>/dev/null; then
    kill "$host_pid" 2>/dev/null
    for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$host_pid" 2>/dev/null || break; sleep 0.5; done
    kill -9 "$host_pid" 2>/dev/null
  fi
  # Keep what a reader needs from the scratch stores, then drop them.
  if [ -d "$scratch/.openagents/tasks" ]; then
    mkdir -p "$evidence/tasks"
    find "$scratch/.openagents/tasks" -maxdepth 2 -type f \
      \( -name '*.jsonl' -o -name 'tasks.json' -o -name '*.json' \) \
      ! -name '*.grant.json' -size -4M -exec cp {} "$evidence/tasks/" \; 2>/dev/null
  fi
  [ -f "$scratch/.openagents/host/autostart.jsonl" ] && cp "$scratch/.openagents/host/autostart.jsonl" "$evidence/" 2>/dev/null
  if [ "$keep" = 1 ]; then
    rm -f "$scratch/.codex/auth.json" "$scratch/.grok/auth.json"
    echo "acceptance: kept the temporary HOME at $scratch (without the Codex and Grok Build login copies)" >&2
  else
    rm -rf "$scratch"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

record() { # name status detail
  python3 - "$evidence/results.jsonl" "$1" "$2" "$3" <<'PY'
import json, sys
path, name, status, detail = sys.argv[1:5]
with open(path, "a") as out:
    out.write(json.dumps({"scenario": name, "status": status, "detail": detail}) + "\n")
PY
  echo "$2 $1: $3"
}

H="$scratch"
export_env() {
  # Every process of the gate sees only the scratch home.
  export HOME="$H"
  export TMPDIR="$H/tmp"
  export OPENAGENTS_ACCEPTANCE_EVIDENCE="$evidence"
  unset OPENAGENTS_SETTINGS OPENAGENTS_TASKS OPENAGENTS_CHAT_HOME OPENAGENTS_CHAT_RELAY OPENAGENTS_CHAT_WORKER
  export GIT_CONFIG_NOSYSTEM=1
}
mkdir -p "$H/tmp" "$H/work"

say "build under test: $desktop"
{
  echo "desktop: $desktop"
  echo "coder: $coder"
  echo "microcoder: $microcoder"
  echo "openagents: $openagents"
  for binary in "$desktop" "$coder" "$microcoder" "$openagents"; do
    shasum -a 256 "$binary"
  done
  "$coder" --version 2>/dev/null || true
} > "$evidence/build.txt" 2>&1

# Engine logins, read-only.
codex_ok=0
claude_ok=0
if [ "${no_engines:-0}" = 1 ]; then
  say "no engine logins read (--no-engines); scenarios needing one are skipped"
elif [ -f "$real_home/.codex/auth.json" ]; then
  # A private copy, never a link, so nothing a run does reaches the real
  # login. Nothing refreshes it; it goes with the scratch home.
  mkdir -p "$H/.codex"
  chmod 700 "$H/.codex"
  (umask 077 && cp "$real_home/.codex/auth.json" "$H/.codex/auth.json") || die "cannot copy the Codex login"
  export CODEX_HOME="$H/.codex"
  codex_ok=1
fi
claude_bin="$(command -v claude || true)"
if [ "${no_engines:-0}" != 1 ] && [ -n "$claude_bin" ] && [ -f "$real_home/.claude.json" ] \
  && /usr/bin/security find-generic-password -s "Claude Code-credentials" >/dev/null 2>&1; then
  mkdir -p "$H/Library"
  ln -s "$real_home/Library/Keychains" "$H/Library/Keychains"
  if python3 - "$real_home/.claude.json" "$H/.claude.json" <<'PY'
import json, sys
state = json.load(open(sys.argv[1]))
keep = {k: state[k] for k in ("oauthAccount", "userID", "hasCompletedOnboarding") if k in state}
if "oauthAccount" not in keep:
    sys.exit(1)
json.dump(keep, open(sys.argv[2], "w"))
PY
  then
    export CLAUDE_BIN="$claude_bin"
    claude_ok=1
  fi
fi
# Grok Build (#10091): the owner's installed binary, and a private copy of
# its login while that login has more than an hour left, so nothing the
# gate's runs do refreshes it (a refresh could retire the owner's refresh
# token). The real file is only read.
grok_ok=0
grok_why="not installed"
grok_bin="${GROK_BIN:-}"
if [ -z "$grok_bin" ]; then
  for candidate in "$(command -v grok || true)" "$real_home/.local/bin/grok" "$real_home/.grok/bin/grok"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then grok_bin="$candidate"; break; fi
  done
fi
grok_auth="${GROK_HOME:-$real_home/.grok}/auth.json"
if [ "${no_engines:-0}" = 1 ]; then
  grok_why="--no-engines"
elif [ -n "$grok_bin" ] && [ -x "$grok_bin" ]; then
  if [ -n "${XAI_API_KEY:-}" ]; then
    grok_ok=1
  elif [ -s "$grok_auth" ]; then
    # The seconds the login has left: the earliest expires_at it holds.
    grok_left="$(python3 - "$grok_auth" <<'PYGROK'
import datetime, json, sys
try:
    logins = json.load(open(sys.argv[1]))
    now = datetime.datetime.now(datetime.timezone.utc)
    left = []
    for login in logins.values():
        at = login.get("expires_at")
        if at:
            left.append((datetime.datetime.fromisoformat(at.replace("Z", "+00:00")) - now).total_seconds())
    print(int(min(left)) if left else 0)
except Exception:
    print(0)
PYGROK
)"
    if [ "${grok_left:-0}" -gt 3600 ]; then
      mkdir -p "$H/.grok"
      chmod 700 "$H/.grok"
      (umask 077 && cp "$grok_auth" "$H/.grok/auth.json") || die "cannot copy the Grok Build login"
      grok_ok=1
    else
      grok_why="its login has $(( ${grok_left:-0} / 60 )) minutes left; run grok once to refresh it, then the gate"
    fi
  else
    grok_why="not signed in"
  fi
fi
if [ "$grok_ok" = 1 ]; then
  export GROK_BIN="$grok_bin"
  unset GROK_HOME
fi
engines_missing=""
[ "$codex_ok" = 1 ] || engines_missing="$engines_missing codex"
[ "$claude_ok" = 1 ] || engines_missing="$engines_missing claude"
[ "$grok_ok" = 1 ] || engines_missing="$engines_missing grok ($grok_why)"
if [ -n "$engines_missing" ]; then
  if [ "$allow_missing" = 1 ]; then
    echo "acceptance: no login for:$engines_missing; scenarios needing it are skipped (--allow-missing-engine)" >&2
  else
    record engine-logins FAIL "no usable login for:$engines_missing (pass --allow-missing-engine to skip those scenarios)"
  fi
fi
export OPENAGENTS_ACCEPTANCE_ENGINES="codex=$codex_ok,claude=$claude_ok,grok=$grok_ok,allow_missing=$allow_missing"

export_env
git config --global user.name "OpenAgents Acceptance" >/dev/null
git config --global user.email "acceptance@example.invalid" >/dev/null
git config --global init.defaultBranch main >/dev/null

# The scratch project: a repository and a linked worktree of it, the
# owner's ~/work/openagents + ~/work/openagents-host-tasks shape.
# Big enough to matter: a few hundred directories and a few thousand
# files in the repository, and hundreds of sibling folders beside it, as
# in ~/work (#10078 exhausted the host's 256 open files on such a tree).
repo="$H/work/acceptance-repo"
worktree="$H/work/acceptance-repo-host-tasks"
remote="$H/work/acceptance-remote.git"
mkdir -p "$repo"
python3 - "$repo" "$H/work" <<'PY' || die "cannot create the scratch tree"
import os, sys
repo, work = sys.argv[1], sys.argv[2]
for d in range(400):
    folder = os.path.join(repo, "src", f"module{d:03}", "parts")
    os.makedirs(folder, exist_ok=True)
    for f in range(8):
        with open(os.path.join(folder, f"part{f}.py"), "w") as out:
            out.write(f"VALUE = {d * 8 + f}\n")
for s in range(650):
    sibling = os.path.join(work, f"sibling-{s:03}")
    os.makedirs(os.path.join(sibling, "inner"), exist_ok=True)
    with open(os.path.join(sibling, "README"), "w") as out:
        out.write("a sibling folder\n")
PY
(
  cd "$repo" &&
  git init -q &&
  printf '# Acceptance notes\n\nA scratch repository for the release acceptance gate.\n' > NOTES.md &&
  printf 'def add(a, b):\n    return a + b\n' > calc.py &&
  git add . && git commit -q -m "Scratch repository" &&
  git worktree add -q -b host-tasks "$worktree" &&
  git init -q --bare "$remote" &&
  git remote add origin "$remote" &&
  git push -q origin main
) || die "cannot create the scratch repository"
export OPENAGENTS_ACCEPTANCE_PROJECT="$worktree"
export OPENAGENTS_ACCEPTANCE_REPO="$repo"
# push-main (#10104): Coder, asked to commit and push to main, must do it
# without asking; the scenario reads this remote's main before and after.
export OPENAGENTS_ACCEPTANCE_REMOTE="$remote"

# phone-closed-loop (#10118): a scratch clone of this repository, its
# `origin` a local bare repository (both share the real repository's
# objects through hard links), which the scenario makes the host's project;
# and what the owner's login shell gives Coder's full-access commands
# (Coder reads the login shell's environment): the toolchains, the build
# cache, where releases keep their state, and the App Store Connect key's
# env file. Signing reads the real login keychain and provisioning
# profiles through links; nothing in them is copied or changed.
loop_selected=0
case " ${only//,/ } " in *" phone-closed-loop "*) loop_selected=1 ;; esac
[ -z "$only" ] && loop_selected=1
if [ "$loop_selected" = 1 ]; then
  loop_remote="$H/work/openagents-remote.git"
  loop_project="$H/work/openagents"
  git clone -q --bare --local "$root" "$loop_remote" \
    && git -C "$loop_remote" update-ref refs/heads/main "$(git -C "$root" rev-parse HEAD)" \
    && git clone -q --local "$loop_remote" "$loop_project" \
    || die "cannot make the scratch clone of $root"
  export OPENAGENTS_ACCEPTANCE_LOOP_PROJECT="$loop_project"
  export OPENAGENTS_ACCEPTANCE_LOOP_REMOTE="$loop_remote"
  export OPENAGENTS_SHIP_DIR="$evidence/ship"
  export OPENAGENTS_ACCEPTANCE_CODER="$coder"
  export OPENAGENTS_ACCEPTANCE_MICROCODER="$microcoder"
  mkdir -p "$OPENAGENTS_SHIP_DIR" "$H/Library/Developer/Xcode"
  [ -e "$H/Library/Keychains" ] || ln -s "$real_home/Library/Keychains" "$H/Library/Keychains"
  [ -d "$real_home/Library/MobileDevice" ] && ln -s "$real_home/Library/MobileDevice" "$H/Library/MobileDevice"
  [ -d "$real_home/Library/Developer/Xcode/UserData" ] \
    && ln -s "$real_home/Library/Developer/Xcode/UserData" "$H/Library/Developer/Xcode/UserData"
  asc_env="${OPENAGENTS_ASC_ENV:-$real_home/work/.secrets/appstoreconnect.env}"
  [ -f "$asc_env" ] && export OPENAGENTS_ACCEPTANCE_ASC="$asc_env"
  {
    echo "export PATH=\"$real_home/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:\$PATH\""
    echo "export CARGO_HOME=\"${CARGO_HOME:-$real_home/.cargo}\""
    echo "export RUSTUP_HOME=\"${RUSTUP_HOME:-$real_home/.rustup}\""
    echo "export CARGO_TARGET_DIR=\"$target\""
    echo "export OPENAGENTS_SHIP_DIR=\"$OPENAGENTS_SHIP_DIR\""
    echo "export OPENAGENTS_ASC_ENV=\"$asc_env\""
  } > "$H/.zshenv"
  cp "$H/.zshenv" "$H/.bash_profile"
fi

# Coder's start setting, as the app's Settings page writes it.
"$openagents" settings set coder.start at_once >/dev/null 2>"$evidence/settings.log" \
  || die "openagents settings failed: $(cat "$evidence/settings.log")"

socket="$H/Library/Application Support/OpenAgents/control.sock"
control() { # JSON op -> JSON reply, framed as the app frames it
  python3 - "$socket" "$1" <<'PY'
import json, socket, struct, sys
path, op = sys.argv[1], json.loads(sys.argv[2])
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(60)
s.connect(path)
body = json.dumps({"v": "openagents.control.v1", "id": 1, "op": op}).encode()
s.sendall(struct.pack(">I", len(body)) + body)
def read(n):
    data = b""
    while len(data) < n:
        chunk = s.recv(n - len(data))
        if not chunk:
            raise SystemExit("closed")
        data += chunk
    return data
(size,) = struct.unpack(">I", read(4))
print(read(size).decode())
PY
}
wait_host() {
  for _ in $(seq 1 120); do
    if [ -S "$socket" ] && control '{"kind":"status"}' >/dev/null 2>&1; then
      return 0
    fi
    kill -0 "$host_pid" 2>/dev/null || return 1
    sleep 0.5
  done
  return 1
}

say "starting the build's host in $H"
# As the app starts it (`coder host serve --keychain --iroh --control`),
# with file keys in the scratch home instead of the login keychain, and
# under launchd's limits: a login agent and an app opened from the Finder
# get 256 open files (#10078).
ulimit -n "$file_limit" || die "cannot set the open-file limit to $file_limit"
"$coder" host serve --keys "$H/host-keys" --iroh --control --label "Acceptance Mac" \
  > "$evidence/host.log" 2>&1 &
host_pid=$!
wait_host || die "the host did not answer on its control socket (see $evidence/host.log)"
reply="$(control "{\"kind\":\"project_add\",\"path\":\"$worktree\"}")" \
  || die "project_add failed"
echo "$reply" > "$evidence/project_add.json"
sleep 1
wait_host || die "the host did not come back after project_add (see $evidence/host.log)"
label="$(control '{"kind":"project_list"}' | python3 -c '
import json, sys
reply = json.load(sys.stdin)
def walk(value):
    if isinstance(value, dict):
        if "label" in value and isinstance(value["label"], str):
            yield value["label"]
        for v in value.values():
            yield from walk(v)
    elif isinstance(value, list):
        for v in value:
            yield from walk(v)
print(next(walk(reply), ""))')"
[ -n "$label" ] || die "the host lists no project after project_add (see $evidence/project_add.json)"
export OPENAGENTS_ACCEPTANCE_LABEL="$label"
control "{\"kind\":\"autostart_set\",\"policy\":{\"enabled\":true,\"projects\":[\"$label\"],\"max_running\":1}}" \
  > "$evidence/autostart_set.json" || die "autostart_set failed"
sleep 1
wait_host || die "the host did not answer after autostart_set (see $evidence/host.log)"
"$coder" host autostart show > "$evidence/autostart.txt" 2>&1 || true
say "host ready: project $label at $worktree"

names="$scenarios"
[ -n "$only" ] && names="${only//,/ }"
desktop_names=""
gate_names=""
for name in $names; do
  case " $gate_scenarios " in
    *" $name "*) gate_names="$gate_names $name" ;;
    *) desktop_names="$desktop_names $name" ;;
  esac
done
desktop_names="${desktop_names# }"
say "running: $names"

# explain-error: the Explain this error plugin
# (docs/plugins/examples/explain-this-error.md) on a planted failure,
# through the build's own `openagents plugin run`, which runs the plugin's
# workflow in Coder's program runtime with reads only. A Python file with
# a wrong dictionary key is planted in a scratch project, the build runs
# it, and the plugin must name the file and line, show the code, and
# suggest the key the dictionary has.
explain_error() {
  local dir="$evidence/explain-error"
  local project="$H/work/planted-failure"
  mkdir -p "$dir" "$project"
  cat > "$project/billing.py" <<'PY'
"""Tax for an invoice line."""
RATES = {"standard": 0.2, "reduced": 0.05}

def tax(amount, band):
    return amount * RATES[band]

if __name__ == "__main__":
    print(tax(100, "reduce"))
PY
  (cd "$project" && python3 billing.py) > "$dir/failure.txt" 2>&1 && {
    record explain-error FAIL "the planted failure didn't fail"
    return
  }
  "$openagents" --json plugin run "$root/crates/plugin-explain-error" --in "$project" \
    --request-file "$dir/failure.txt" > "$dir/run.json" 2> "$dir/run.err" || {
    record explain-error FAIL "openagents plugin run failed: $(tail -1 "$dir/run.err")"
    return
  }
  local verdict
  verdict="$(python3 - "$dir/run.json" <<'PY'
import json, sys
run = json.load(open(sys.argv[1]))
value = run["steps"][0]["output"]["value"]
problems = []
if value.get("location", {}).get("file") != "billing.py" or value["location"].get("line") != 5:
    problems.append(f"location {value.get('location')}")
if not any(s.get("name") == "reduced" for s in value.get("suggestions", [])):
    problems.append(f"suggestions {value.get('suggestions')}")
if "billing.py:5" not in run.get("reply", "") or "RATES[band]" not in run.get("reply", ""):
    problems.append("the reply doesn't show the line")
print("; ".join(problems) if problems else "ok")
PY
)"
  if [ "$verdict" = "ok" ]; then
    record explain-error PASS "named billing.py:5, showed the line, and suggested the key 'reduced' (run.json)"
  else
    record explain-error FAIL "$verdict (run.json)"
  fi
}
# plugins-chat: the chat knows every plugin in the Gym (#10090). The
# build's `openagents chat --scratch` (a throwaway identity) asks the live
# chat worker "which plugins can I test?", and the reply must name every
# plugin deploy/eval-runner/catalog lists, by its package.json name, the
# catalog the hosted runner and the Gym's chips use.
plugins_chat() {
  local dir="$evidence/plugins-chat"
  mkdir -p "$dir"
  "$openagents" chat --scratch --no-run --json "which plugins can I test?" \
    > "$dir/chat.ndjson" 2> "$dir/chat.err" || {
    record plugins-chat FAIL "openagents chat failed: $(tail -1 "$dir/chat.err")"
    return
  }
  local verdict
  verdict="$(python3 - "$dir/chat.ndjson" "$root" <<'PY'
import json, os, sys
events = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
root = sys.argv[2]
names = []
for line in open(os.path.join(root, "deploy/eval-runner/catalog")):
    line = line.strip()
    if line and not line.startswith("#"):
        names.append(json.load(open(os.path.join(root, line, "package.json")))["name"])
result = next((e for e in events if e.get("event") == "result"), None)
if result is None:
    print("no result (chat.ndjson)")
    sys.exit()
text = result.get("text", "")
route = next((e.get("route") for e in events if e.get("event") == "route"), None)
missing = [name for name in names if name.lower() not in text.lower()]
if missing:
    print(f"route {route}: the reply leaves out {', '.join(missing)}: {text!r}")
else:
    print(f"ok route {route}, all {len(names)} named")
PY
)"
  case "$verdict" in
    ok*) record plugins-chat PASS "${verdict#ok } (chat.ndjson)" ;;
    *) record plugins-chat FAIL "$verdict" ;;
  esac
}
# essays-chat: the chat answers from our two essays (#10099). The build's
# `openagents chat --scratch` (a throwaway identity) asks the live chat
# worker one question about each essay, and each reply must carry that
# essay's own idea and not the "no documented answer" reply.
essays_chat() {
  local dir="$evidence/essays-chat" n=0 failed=""
  mkdir -p "$dir"
  local questions=(
    "what is a capability claim?|with-and-without,marginal effect,claim key,evidence"
    "what is your thesis about general agents?|composition"
    "summarize both of the essays, please|Test-Time Capabilities;General Agent"
  )
  local entry question words
  for entry in "${questions[@]}"; do
    n=$((n + 1))
    question="${entry%%|*}"
    words="${entry#*|}"
    "$openagents" chat --scratch --no-run --json "$question" \
      > "$dir/chat$n.ndjson" 2> "$dir/chat$n.err" || {
      failed="$failed [$question: openagents chat failed: $(tail -1 "$dir/chat$n.err")]"
      continue
    }
    local verdict
    verdict="$(python3 - "$dir/chat$n.ndjson" "$words" <<'PY'
import json, sys
events = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
result = next((e for e in events if e.get("event") == "result"), None)
if result is None:
    print("no result")
    sys.exit()
text = result.get("text", "")
route = next((e.get("route") for e in events if e.get("event") == "route"), None)
if "no documented answer" in text.lower() or "don't have that documented" in text.lower():
    print(f"route {route}: the chat has no documented answer: {text!r}")
elif route == "work.dispatch":
    print(f"route {route}: the question was dispatched to Coder: {text!r}")
elif missing := [g for g in sys.argv[2].split(";")
                 if not any(w.lower() in text.lower() for w in g.split(","))]:
    print(f"route {route}: the reply names none of {missing[0]}: {text!r}")
else:
    print(f"ok route {route}")
PY
)"
    case "$verdict" in
      ok*) ;;
      *) failed="$failed [$question: $verdict]" ;;
    esac
  done
  if [ -z "$failed" ]; then
    record essays-chat PASS "the essay questions answered from our essays, and both essays summarized (chat1.ndjson to chat3.ndjson)"
  else
    record essays-chat FAIL "$failed"
  fi
}
# phone-sim-start: the actual iOS app in an iOS simulator (#10118). The
# gate makes its own simulator (never the owner's), installs the app
# (OPENAGENTS_ACCEPTANCE_IOS_APP names a built OpenAgents.app for the
# simulator; otherwise bins/openagents-ios/build.sh sim builds one), and
# opens it with the host's invitation (`--connect-link`) as the phone's
# camera would. Once the host lists the phone, the app is opened again,
# paired, and sends a coding question from its Chat tab's composer
# (`--coder-tap send:`). The live chat worker routes it; the reply must
# start exactly one Coder task on the host with no tap. Screenshots before
# the send, after the reply, and after the start. Skipped where Xcode or
# an iOS simulator runtime is missing.
phone_sim_start() {
  local dir="$evidence/phone-sim-start" udid=""
  mkdir -p "$dir"
  # The simulator and Xcode live in the real home, never the gate's.
  local real_tmp
  real_tmp="$(getconf DARWIN_USER_TEMP_DIR 2>/dev/null || echo /tmp)"
  sim() { HOME="$real_home" TMPDIR="$real_tmp" xcrun simctl "$@"; }
  if [ "$codex_ok" != 1 ]; then
    if [ "$allow_missing" = 1 ]; then
      record phone-sim-start SKIP "no Codex login for the computer's Coder run"
    else
      record phone-sim-start FAIL "no Codex login for the computer's Coder run"
    fi
    return
  fi
  local picked
  picked="$(sim list -j 2>/dev/null | python3 -c '
import json, sys
listed = json.load(sys.stdin)
runtimes = [r for r in listed.get("runtimes", [])
            if r.get("isAvailable") and r.get("platform") == "iOS"]
types = [t["identifier"] for t in listed.get("devicetypes", [])
         if t.get("productFamily") == "iPhone"]
if runtimes and types:
    pro = [t for t in types if t.endswith("iPhone-17-Pro")]
    print(runtimes[-1]["identifier"], (pro or types)[-1])
' 2>/dev/null)"
  if [ -z "$picked" ] || ! HOME="$real_home" xcodebuild -version >/dev/null 2>&1; then
    record phone-sim-start SKIP "no Xcode or iOS simulator runtime on this Mac"
    return
  fi
  udid="$(sim create oa-loop-gate "${picked#* }" "${picked%% *}" 2>"$dir/simctl.err")" || {
    record phone-sim-start SKIP "cannot create a simulator: $(tail -1 "$dir/simctl.err")"
    return
  }
  echo "$udid $picked" > "$dir/simulator.txt"
  phone_sim_start_run "$dir" "$udid"
  sim shutdown "$udid" >/dev/null 2>&1
  sim delete "$udid" >/dev/null 2>&1
}
phone_sim_start_run() { # dir udid
  local dir="$1" udid="$2" bundle=com.openagents.app
  local ask="Can you look through the code in my project and summarize what it implements?"
  sim boot "$udid" 2>>"$dir/simctl.err"
  sim bootstatus "$udid" -b >/dev/null 2>>"$dir/simctl.err" || {
    record phone-sim-start FAIL "the simulator did not boot (simctl.err)"
    return
  }
  local built="${OPENAGENTS_ACCEPTANCE_IOS_APP:-}"
  if [ -z "$built" ]; then
    say "phone-sim-start: building the iOS app for the simulator (ios-build.log)"
    local output="${OPENAGENTS_IOS_OUTPUT:-$target/openagents-ios}"
    HOME="$real_home" TMPDIR="$real_tmp" CARGO_TARGET_DIR="$target" OPENAGENTS_IOS_OUTPUT="$output" \
      OPENAGENTS_IOS_DEVICE="$udid" "$root/bins/openagents-ios/build.sh" sim \
      > "$dir/ios-build.log" 2>&1 || {
      record phone-sim-start FAIL "the iOS app did not build (ios-build.log: $(tail -1 "$dir/ios-build.log"))"
      return
    }
    built="$output/DerivedData/Build/Products/Release-iphonesimulator/OpenAgents.app"
  fi
  [ -d "$built" ] || { record phone-sim-start FAIL "no iOS app at $built"; return; }
  shasum -a 256 "$built/OpenAgents" > "$dir/app.txt" 2>/dev/null
  sim terminate "$udid" "$bundle" >/dev/null 2>&1
  sim install "$udid" "$built" 2>>"$dir/simctl.err" || {
    record phone-sim-start FAIL "the app did not install (simctl.err)"
    return
  }
  # The host's invitation, as the desktop app shows it.
  local code
  code="$(control '{"kind":"invite_create"}' | python3 -c '
import json, sys
def walk(value):
    if isinstance(value, dict):
        for v in value.values():
            yield from walk(v)
    elif isinstance(value, list):
        for v in value:
            yield from walk(v)
    elif isinstance(value, str) and value.startswith("openagents-connect:"):
        yield value
print(next(walk(json.load(sys.stdin)), ""))')"
  [ -n "$code" ] || { record phone-sim-start FAIL "the host made no invitation"; return; }
  local devices_before
  devices_before="$(control '{"kind":"device_list"}' | python3 -c 'import sys; print(sys.stdin.read().count("\"device\""))')"
  # Pairing: the app opened with the code, as the camera opens it.
  sim launch --terminate-running-process "$udid" "$bundle" --connect-link "$code" --tab coder \
    > "$dir/launch-pair.txt" 2>&1 || {
    record phone-sim-start FAIL "the app did not launch ($(tail -1 "$dir/launch-pair.txt"))"
    return
  }
  local paired=0
  for _ in $(seq 1 90); do
    sleep 2
    if [ "$(control '{"kind":"device_list"}' | python3 -c 'import sys; print(sys.stdin.read().count("\"device\""))')" -gt "$devices_before" ]; then
      paired=1; break
    fi
  done
  control '{"kind":"device_list"}' > "$dir/device_list.json" 2>&1
  sleep 3
  sim io "$udid" screenshot "$dir/1-paired.png" >/dev/null 2>&1
  [ "$paired" = 1 ] || { record phone-sim-start FAIL "the host lists no new device after the app opened its invitation (1-paired.png)"; return; }
  # The question, from the Chat tab's composer, on a fresh launch: the
  # pairing must hold, as on a phone opened again later.
  local journal="$H/.openagents/host/autostart.jsonl"
  local before
  before="$(python3 - "$journal" <<'PY'
import json, sys
try:
    lines = open(sys.argv[1]).read().splitlines()
except OSError:
    lines = []
print(len(lines))
PY
)"
  sim launch --terminate-running-process "$udid" "$bundle" --tab coder \
    --coder-tap "sleep:20,send:$ask" > "$dir/launch-send.txt" 2>&1 || {
    record phone-sim-start FAIL "the app did not launch again ($(tail -1 "$dir/launch-send.txt"))"
    return
  }
  sleep 15
  sim io "$udid" screenshot "$dir/2-before-send.png" >/dev/null 2>&1
  local sent_at verdict="" shot_reply=0 started_at=0 n=0
  sent_at=$(date +%s)
  while :; do
    sleep 2; n=$((n + 1))
    verdict="$(python3 - "$journal" "$before" <<'PY'
import json, sys
try:
    lines = open(sys.argv[1]).read().splitlines()[int(sys.argv[2]):]
except OSError:
    lines = []
entries = []
for line in lines:
    try:
        entries.append(json.loads(line))
    except ValueError:
        pass
tasks = {e["task"] for e in entries if e.get("task")}
started = [e for e in entries if e.get("event") == "started"]
bad = [e for e in entries if e.get("event") in ("refused", "no_capacity", "not_started", "skipped", "unadmitted")]
if bad:
    print(f"bad {bad[0].get('event')} {bad[0].get('detail', '')}")
elif started:
    print(f"started {len(started)} {len(tasks)} {started[0].get('task')}")
elif tasks:
    print("task")
else:
    print("none")
PY
)"
    case "$verdict" in
      task|started*)
        if [ "$shot_reply" = 0 ]; then
          sim io "$udid" screenshot "$dir/3-after-reply.png" >/dev/null 2>&1
          shot_reply=1
        fi ;;
    esac
    case "$verdict" in
      bad*) break ;;
      started*)
        [ "$started_at" = 0 ] && started_at=$(date +%s)
        # A while longer, so a second start for the same reply shows.
        if [ $(( $(date +%s) - started_at )) -ge 20 ]; then break; fi ;;
    esac
    [ $(( $(date +%s) - sent_at )) -ge 360 ] && break
  done
  sim io "$udid" screenshot "$dir/4-after-start.png" >/dev/null 2>&1
  [ -f "$journal" ] && tail -n "+$((before + 1))" "$journal" > "$dir/autostart.jsonl"
  case "$verdict" in
    "started 1 1 "*)
      [ "$shot_reply" = 1 ] || sim io "$udid" screenshot "$dir/3-after-reply.png" >/dev/null 2>&1
      record phone-sim-start PASS "the iOS app in a simulator paired by its invitation and asked from its Chat tab; the reply started task ${verdict##* } on the host with no tap, $(( started_at - sent_at ))s after the send (autostart.jsonl, 1-paired.png to 4-after-start.png)" ;;
    started*)
      set -- $verdict
      record phone-sim-start FAIL "one question started $2 runs over $3 tasks (autostart.jsonl)" ;;
    bad*) record phone-sim-start FAIL "the computer did not start the task: ${verdict#bad } (autostart.jsonl)" ;;
    task) record phone-sim-start FAIL "a task was created but Coder never started within 6 minutes (autostart.jsonl, 4-after-start.png)" ;;
    *) record phone-sim-start FAIL "no Coder task on the host within 6 minutes of the send (3-after-reply.png absent, 4-after-start.png)" ;;
  esac
}
for name in $gate_names; do
  case "$name" in
    explain-error) explain_error ;;
    plugins-chat) plugins_chat ;;
    essays-chat) essays_chat ;;
    phone-sim-start) phone_sim_start ;;
  esac
done

# The scenarios, through the build's desktop binary.
if [ -n "$desktop_names" ]; then
  "$desktop" --acceptance "$evidence" --only "${desktop_names// /,}" 2>"$evidence/desktop.log" \
    | tee "$evidence/desktop.out"
  driver=${PIPESTATUS[0]}
  if [ "$driver" -ne 0 ] && [ "$driver" -ne 1 ]; then
    record desktop-driver FAIL "the desktop binary's acceptance mode exited $driver (see desktop.log)"
  fi
fi
for name in $names; do
  grep -Eq "\"scenario\": ?\"$name\"" "$evidence/results.jsonl" \
    || record "$name" FAIL "no result: the driver stopped before it (see desktop.log)"
done

# The summary.
python3 - "$evidence/results.jsonl" > "$evidence/summary.md" <<'PY'
import json, sys
rows = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
print("| Scenario | Result | Evidence |")
print("| --- | --- | --- |")
for row in rows:
    detail = row["detail"].replace("|", "\\|").replace("\n", " ")
    print(f"| {row['scenario']} | {row['status']} | {detail} |")
PY
echo
cat "$evidence/summary.md"
echo
echo "evidence: $evidence"
if grep -Eq '"status": ?"FAIL"' "$evidence/results.jsonl"; then
  exit 1
fi
exit 0
