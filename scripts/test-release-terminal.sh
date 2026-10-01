#!/bin/sh
# The release script defines the names this reads, and reads the ones it sets.
# shellcheck disable=SC2034,SC2154
# Tests for the OpenAgents Terminal release and its installer, with no
# bucket and no network: scripts/release/terminal.sh's channel coverage and
# version grammar, and scripts/install/openagents.sh against a local HTTP
# server holding fake artifacts.
#
#   scripts/test-release-terminal.sh
#
# Needs python3 (for the local server) and shasum or sha256sum.

set -eu

checkout=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/openagents-release-test.XXXXXX")
server_pid=''
cleanup() {
  [ -z "$server_pid" ] || kill "$server_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT INT TERM

failures=0
pass() { echo "ok   $1"; }
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

if command -v shasum >/dev/null 2>&1; then
  digest() { shasum -a 256 "$1" | awk '{ print $1 }'; }
else
  digest() { sha256sum "$1" | awk '{ print $1 }'; }
fi

# ---- the release script's functions --------------------------------------

OPENAGENTS_RELEASE_SCRIPT="$checkout/scripts/release/terminal.sh"
OPENAGENTS_RELEASE_LIBRARY=1
# shellcheck source=scripts/release/terminal.sh
. "$checkout/scripts/release/terminal.sh"
set -eu
# The release script defines the names this reads, and reads the ones it sets.
# shellcheck disable=SC2034,SC2154

version=1.0.0-rc.1
for good in 1.0.0 1.0.0-rc.1 1.0.0-rc.0 12.3.45-rc.10; do
  if (version=$good; check_version) 2>/dev/null; then pass "version $good is accepted"; else fail "version $good is accepted"; fi
done
for bad in 1.0.0-rc1 1.0.0-rc.01 v1.0.0 1.0 rc.1 1.0.0-beta.1; do
  if (version=$bad; check_version) 2>/dev/null; then fail "version $bad is refused"; else pass "version $bad is refused"; fi
done

# Every platform covered: both artifacts listed and both digests in the sums.
: >"$work/sums"
: >"$work/objects"
for platform in $all_platforms; do
  for name in "$product" "$engine"; do
    artifact=$(artifact_name "$name" "$platform")
    echo "$artifact" >>"$work/objects"
    printf '%064d  %s\n' 0 "$(sums_name_for "$artifact" "$platform")" >>"$work/sums"
  done
done
gap=$(uncovered_platforms "$work/sums" "$work/objects")
[ -z "$gap" ] && pass "a version on every platform covers the channel" || fail "a version on every platform covers the channel (gap:$gap)"

# The Windows sums entry carries `.exe`; the object does not.
grep -qxF "$(printf '%064d' 0)  openagents-1.0.0-rc.1-windows-x86_64.exe" "$work/sums" &&
  grep -qxF "openagents-1.0.0-rc.1-windows-x86_64" "$work/objects" &&
  pass "a Windows artifact is listed without .exe and summed with it" ||
  fail "a Windows artifact is listed without .exe and summed with it"

# A missing engine leaves its platform uncovered.
grep -vxF "microcoder-1.0.0-rc.1-linux-aarch64-musl" "$work/objects" >"$work/objects-no-engine"
gap=$(uncovered_platforms "$work/sums" "$work/objects-no-engine")
[ "$gap" = " linux-aarch64-musl" ] && pass "a platform without its engine is a gap" || fail "a platform without its engine is a gap (gap:$gap)"

# A missing sums entry leaves its platform uncovered.
grep -v 'openagents-1.0.0-rc.1-windows-x86_64.exe' "$work/sums" >"$work/sums-no-windows"
gap=$(uncovered_platforms "$work/sums-no-windows" "$work/objects")
[ "$gap" = " windows-x86_64" ] && pass "a platform without a checksum entry is a gap" || fail "a platform without a checksum entry is a gap (gap:$gap)"

# A longer version's artifacts do not cover this one.
sed 's/1\.0\.0-rc\.1-/1.0.0-rc.10-/' "$work/objects" >"$work/objects-longer"
gap=$(uncovered_platforms "$work/sums" "$work/objects-longer")
[ "$(echo "$gap" | wc -w | tr -d ' ')" = 7 ] && pass "rc.10's artifacts do not cover rc.1" || fail "rc.10's artifacts do not cover rc.1 (gap:$gap)"

# ---- the installer against a local server ---------------------------------

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform=macos-aarch64 ;;
  Darwin-x86_64)
    if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || :)" = 1 ]; then platform=macos-aarch64; else platform=macos-x86_64; fi
    ;;
  Linux-x86_64) platform=linux-x86_64 ;;
  Linux-aarch64) platform=linux-aarch64 ;;
  *) echo "skip installer tests on $(uname -sm)"; exit "$failures" ;;
esac
if [ "${platform#linux}" != "$platform" ] && ! { [ -e /lib64/ld-linux-x86-64.so.2 ] || [ -e /lib/ld-linux-aarch64.so.1 ] || [ -e /lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 ] || [ -e /lib/ld-linux-x86-64.so.2 ] || [ -e /lib64/ld-linux-aarch64.so.1 ] || [ -e /lib/aarch64-linux-gnu/ld-linux-aarch64.so.1 ]; }; then
  platform="$platform-musl"
fi

serve="$work/serve"
mkdir -p "$serve"
make_release() { # $1 version
  printf '#!/bin/sh\necho "openagents %s"\n' "$1" >"$serve/openagents-$1-$platform"
  printf '#!/bin/sh\necho "microcoder %s"\n' "$1" >"$serve/microcoder-$1-$platform"
  : >"$serve/SHA256SUMS-openagents-$1"
  for name in openagents microcoder; do
    printf '%s  %s\n' "$(digest "$serve/$name-$1-$platform")" "$name-$1-$platform" >>"$serve/SHA256SUMS-openagents-$1"
  done
}
make_release 1.0.0-rc.1
make_release 1.0.0-rc.2
echo 1.0.0-rc.1 >"$serve/openagents.rc"
# rc.2's engine bytes do not match its sums file.
echo tampered >>"$serve/microcoder-1.0.0-rc.2-$platform"

port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
(cd "$serve" && exec python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server_pid=$!
i=0
until curl -fsS "http://127.0.0.1:$port/openagents.rc" >/dev/null 2>&1; do
  i=$((i + 1))
  [ "$i" -lt 50 ] || { fail "the local server starts"; exit 1; }
  sleep 0.1
done

run_install() { # $1 bin dir, rest: args to the installer
  _bin=$1
  shift
  OPENAGENTS_BASE_URL="http://127.0.0.1:$port" OPENAGENTS_BIN_DIR="$_bin" OPENAGENTS_NO_LAUNCH=1 \
    HOME="$work/home" sh "$checkout/scripts/install/openagents.sh" "$@" </dev/null >"$work/install.log" 2>&1
}

# No stable pointer: it follows rc.
if run_install "$work/bin1" && [ "$("$work/bin1/openagents")" = "openagents 1.0.0-rc.1" ] &&
  [ "$("$work/bin1/microcoder")" = "microcoder 1.0.0-rc.1" ] &&
  grep -q 'No stable release yet; following the rc channel.' "$work/install.log"; then
  pass "with no stable release it installs rc, both programs side by side"
else
  fail "with no stable release it installs rc, both programs side by side"
  sed 's/^/    /' "$work/install.log"
fi

# A named version.
if run_install "$work/bin2" 1.0.0-rc.1 && [ -x "$work/bin2/openagents" ] && [ -x "$work/bin2/microcoder" ]; then
  pass "a named version installs"
else
  fail "a named version installs"
  sed 's/^/    /' "$work/install.log"
fi

# A checksum mismatch installs nothing, and leaves what was there.
if ! run_install "$work/bin1" 1.0.0-rc.2 && grep -q 'Checksum mismatch for microcoder-1.0.0-rc.2' "$work/install.log" &&
  [ "$("$work/bin1/openagents")" = "openagents 1.0.0-rc.1" ] &&
  [ -z "$(find "$work/bin1" -name '.install.*')" ]; then
  pass "a checksum mismatch installs nothing and keeps the installed version"
else
  fail "a checksum mismatch installs nothing and keeps the installed version"
  sed 's/^/    /' "$work/install.log"
fi

# An unpublished version and a bad version string are refused.
if ! run_install "$work/bin3" 9.9.9 && grep -q 'Version 9.9.9 is not published' "$work/install.log"; then
  pass "an unpublished version is refused"
else
  fail "an unpublished version is refused"
fi
if ! run_install "$work/bin3" stable && grep -q 'Not a version: stable' "$work/install.log"; then
  pass "a channel name passed as a version is refused"
else
  fail "a channel name passed as a version is refused"
fi

# A version with no build for this platform says so.
printf '%064d  openagents-2.0.0-some-other-platform\n' 0 >"$serve/SHA256SUMS-openagents-2.0.0"
if ! run_install "$work/bin3" 2.0.0 && grep -q "Version 2.0.0 has no $platform build" "$work/install.log"; then
  pass "a version without this platform says so"
else
  fail "a version without this platform says so"
fi

echo
[ "$failures" = 0 ] && echo "all passed" || echo "$failures failed"
exit "$failures"
