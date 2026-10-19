#!/bin/sh
# Check that this machine's Rust builds use the shared remote compile cache
# (#12176, docs/coder/runtime/remote-build-cache.md). Prints one line per
# check and exits 1 when a required one fails.
#
# Usage: scripts/dev/check-build-cache.sh [--wrapper PATH]
#   --wrapper names the rustc wrapper a service passes in RUSTC_WRAPPER
#   (the land-queue integrator's drop-in), when this shell's Cargo
#   configuration isn't what that service runs.
# Environment: KACHE_CONFIG, KACHE (the kache program), CARGO_HOME.
set -u

want_version=0.28.1
wrapper=${RUSTC_WRAPPER:-}
while [ $# -gt 0 ]; do
  case $1 in
    --wrapper) wrapper=${2:?}; shift 2 ;;
    -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "check-build-cache: unknown option: $1" >&2; exit 2 ;;
  esac
done

fail=0
ok() { echo "ok    $1"; }
bad() { echo "FAIL  $1"; fail=1; }

cargo_config=${CARGO_HOME:-$HOME/.cargo}/config.toml
if [ -z "$wrapper" ]; then
  wrapper=$(sed -n 's/^rustc-wrapper *= *"\(.*\)"/\1/p' "$cargo_config" 2>/dev/null | head -n 1)
fi
case $wrapper in
  */kache|kache) ok "rustc wrapper is kache ($wrapper)" ;;
  */oa-rustc-wrapper)
    if grep -qs 'exec /usr/local/bin/kache' "$wrapper"; then ok "rustc wrapper $wrapper runs kache"
    else bad "rustc wrapper $wrapper doesn't run kache"; fi ;;
  '') bad "no rustc wrapper: set build.rustc-wrapper in $cargo_config" ;;
  *) bad "rustc wrapper is $wrapper, not kache" ;;
esac

k=${KACHE:-$(command -v kache 2>/dev/null || true)}
[ -n "$k" ] || for c in "$HOME/.local/bin/kache" "$HOME/.cargo/bin/kache" /usr/local/bin/kache; do
  [ -x "$c" ] && { k=$c; break; }
done
if [ -z "$k" ]; then
  bad "kache is not installed"
  exit 1
fi
version=$("$k" --version 2>/dev/null | awk '{print $2}')
if [ "$version" = "$want_version" ]; then ok "kache $version"
else bad "kache ${version:-unknown}, not $want_version: keys differ between versions"; fi

file=${KACHE_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/kache/config.toml}
source=$(sed -n 's/^# credentials: //p' "$file" 2>/dev/null | tail -n 1)
if [ -z "$source" ]; then
  bad "no shared remote in $file: run scripts/dev/remote-build-cache.sh enable"
  exit 1
fi
ok "remote configured in $file (credentials: $source)"

key=
case $source in
  key\ *)
    key=${source#key }
    if [ -r "$key" ]; then ok "key file readable"; else bad "key file $key is missing"; fi
    unit=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/kache.service.d/openagents-remote.conf
    plist=$HOME/Library/LaunchAgents/ninja.kunobi.kache.plist
    if grep -qs "GOOGLE_APPLICATION_CREDENTIALS=$key" "$unit" ||
      { [ -f "$plist" ] && plutil -extract EnvironmentVariables.GOOGLE_APPLICATION_CREDENTIALS raw "$plist" 2>/dev/null | grep -qxF "$key"; }; then
      ok "kache's daemon service has the key"
    else
      bad "kache's daemon service doesn't have the key: rerun enable --key $key"
    fi
    ;;
esac

# kache's doctor reaches the bucket with this process's credentials.
doctor=$(if [ -n "$key" ]; then GOOGLE_APPLICATION_CREDENTIALS=$key "$k" doctor --json; else "$k" doctor --json; fi 2>/dev/null)
access=$(printf '%s' "$doctor" | python3 -c '
import json, sys
try:
    checks = json.load(sys.stdin)["checks"]
except Exception:
    print("unreadable"); sys.exit()
for c in checks:
    if c["label"] == "Remote access":
        print(("pass " if c["pass"] else "fail ") + str(c["detail"])); break
else:
    print("missing")')
case $access in
  pass\ *) ok "bucket reachable: ${access#pass }" ;;
  *) bad "bucket not reachable: $access" ;;
esac

# What the last build got from the remote, for information.
"$k" report --last-build --format json 2>/dev/null | python3 -c '
import collections, json, sys
try:
    events = json.load(sys.stdin)["all_events"]
except Exception:
    sys.exit()
c = collections.Counter(e["result"] for e in events)
remote = c.get("remote_hit", 0) + c.get("prefetch_hit", 0)
local, miss = c.get("local_hit", 0), c.get("miss", 0)
print(f"info  last build: {sum(c.values())} crates, {remote} from the remote, {local} local hits, {miss} compiled")'

exit "$fail"
