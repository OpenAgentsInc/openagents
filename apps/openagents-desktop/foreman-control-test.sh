#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around swiftc and the app's own command line; holds no logic)
# Checks the Foreman control API (docs/coder/foreman-control-api.md).
#   apps/coder-desktop/foreman-control-test.sh          the credentials, route policy, descriptor and server
#   apps/coder-desktop/foreman-control-test.sh --live   also drives a built Coder.app through foreman-ctl
# --live opens a Coder window for a few seconds. It runs under a throwaway home folder, so it does not
# read or change your chats, and it sends no message to a model. Settings are not under that folder:
# the check changes the Foreman tier and puts back the one you had when it ends.
#   CODER_BUILD=DIR   where build.sh put Coder.app (default apps/coder-desktop/build)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d /tmp/foreman-control-test.XXXXXX)"
pid=""
app=""
domain=com.openagents.desktop
tier_was=skip   # skip: the live check did not run; unset: no tier was chosen; else the tier's number
cleanup() {
  if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; fi
  # the coder-host serve this Coder started outlives it on purpose: stop it by its own PID (#13752)
  local disc="$tmp/home/.openagents/coder/host/daemon.json" hp=""
  [[ -s "$disc" ]] && hp="$(python3 -c 'import json,sys; print(int(json.load(open(sys.argv[1])).get("pid") or 0))' "$disc" 2>/dev/null || true)"
  if [[ -n "$app" && -n "$hp" && "$hp" != 0 ]] && ps -o command= -p "$hp" 2>/dev/null | grep -qF "$app"; then kill "$hp" 2>/dev/null || true; fi
  if [[ "$tier_was" == unset ]]; then defaults delete "$domain" foremanTier 2>/dev/null || true
  elif [[ "$tier_was" != skip ]]; then defaults write "$domain" foremanTier -int "$tier_was"; fi
  rm -rf "$tmp"
}
trap cleanup EXIT

xcrun -sdk macosx swiftc -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 \
  "$here/Foreman.swift" "$here/ForemanOperator.swift" "$here/foreman-control-test.swift" -o "$tmp/test"
"$tmp/test"

[[ "${1:-}" == --live ]] || exit 0

app="${CODER_BUILD:-$here/build}/OpenAgents.app"
app="$(cd "$(dirname "$app")" 2>/dev/null && pwd)/OpenAgents.app"
[[ -x "$app/Contents/MacOS/OpenAgents" ]] || { echo "no $app: build it with apps/openagents-desktop/build.sh --full"; exit 1; }
export CODER_FOREMAN_CONTROL_FILE="$tmp/foreman-control.json"
unset CODER_FOREMAN_URL CODER_FOREMAN_TOKEN
mkdir -p "$tmp/home"
tier_was="$(defaults read "$domain" foremanTier 2>/dev/null || echo unset)"
# CODER_HOME keeps the daemon's state in the throwaway home; CODER_TEST_LAUNCH makes this copy stop its
# coder-host when it quits and quit itself if this script dies (TestLaunch, AppDelegate.swift)
CFFIXED_USER_HOME="$tmp/home" CODER_HOME="$tmp/home" CODER_TEST_LAUNCH=foreman-control-test CODER_TEST_IDLE_S=300 \
  BROWSER=/usr/bin/true "$app/Contents/MacOS/OpenAgents" > "$tmp/coder.log" 2>&1 &
pid=$!
for _ in $(seq 1 100); do [[ -s "$CODER_FOREMAN_CONTROL_FILE" ]] && break; sleep 0.1; done
[[ -s "$CODER_FOREMAN_CONTROL_FILE" ]] || { echo "FAIL Coder wrote no control file"; exit 1; }
ctl="$app/Contents/Resources/foreman-ctl"
fails=0
# expect WHAT NEEDLE COMMAND...: the command's output holds NEEDLE.
expect() {
  local what="$1" needle="$2"; shift 2
  local out; out="$("$@" 2>&1 || true)"
  if [[ "$out" == *"$needle"* ]]; then echo "ok   $what"; else fails=$((fails + 1)); echo "FAIL $what: $out"; fi
}
expect "the control file is 0600" "600" stat -f %Lp "$CODER_FOREMAN_CONTROL_FILE"
expect "info names the operator" '"principal":"operator"' "$ctl" info
expect "status answers" '"ok":true' "$ctl" status
expect "config reports the tier and the raise switch" '"operator_tier_raise":false' "$ctl" config
expect "Foreman starts out of view" '"shown":false' "$ctl" foreman status
expect "open shows Foreman" '"shown":true' "$ctl" foreman open
expect "the tier can be lowered" '"tier":"observe"' "$ctl" set-tier observe
expect "config shows the new tier" '"tier":"observe"' "$ctl" config
expect "the tier cannot be raised by default" '"operator_tier_raise":false' "$ctl" set-tier autonomous
expect "the tier is still observe" '"tier":"observe"' "$ctl" tier
expect "an unknown tier is refused" 'tier must be' "$ctl" set-tier bogus
expect "an empty prompt is refused" 'prompt is required' "$ctl" foreman prompt " "
expect "stop on an idle Foreman is a no-op" '"status":"already_idle"' "$ctl" foreman stop
expect "wait on an idle Foreman returns at once" '"timed_out":false' "$ctl" foreman wait --timeout 5
expect "reset clears the history" '"has_history":false' "$ctl" foreman reset
expect "chats can be listed" '"chats":[' "$ctl" list-chats
expect "the tier change is an event" 'tier.changed' "$ctl" events
expect "operator actions are audited" 'operator:set_tier' "$ctl" audit
token="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["token"])' "$CODER_FOREMAN_CONTROL_FILE")"
url="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["url"])' "$CODER_FOREMAN_CONTROL_FILE")"
expect "a wrong token is refused" '401' curl -s -o /dev/null -w '%{http_code}' -H "X-Foreman-Token: ${token}x" "$url/v1/status"
kill "$pid"; for _ in $(seq 1 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done; pid=""
expect "a Coder that has ended is reported, not called" 'Coder is not running' "$ctl" status
[[ "$fails" == 0 ]] && echo "foreman-control-test --live: all checks passed" || { echo "foreman-control-test --live: $fails failed"; exit 1; }
