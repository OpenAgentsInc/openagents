#!/usr/bin/env bash
# Tests for os/bin/coder-close. Run it by hand:
#
#   os/tests/coder-close.sh
#
# Every tool the script calls is a stub: `coder-desk` prints the window a
# test sets and logs each call, `ps` prints a process table a test sets,
# `coder` answers `activity` as busy when it is asked about a PID in
# TEST_ACTIVE, and `notify-send` logs the notice.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/coder-close"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-close-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
check() {
    local name="$1"
    shift
    if "$@"; then
        printf 'ok   %s\n' "$name"
    else
        printf 'FAIL %s\n' "$name"
        printf '     desk: %s\n' "$(tr '\n' ';' < "$TEST_DESK_LOG")"
        printf '     coder: %s\n' "$(tr '\n' ';' < "$TEST_CLIENT_LOG")"
        printf '     notice: %s\n' "$(tr '\n' ';' < "$TEST_NOTIFY_LOG")"
        failures=$((failures + 1))
    fi
}

stubs="$scratch/stubs"
mkdir -p "$stubs"

cat > "$stubs/coder-desk" << 'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TEST_DESK_LOG"
if [ "$1" = focused ]; then printf '%s\n' "${TEST_WINDOW:-null}"; fi
STUB

cat > "$stubs/ps" << 'STUB'
#!/usr/bin/env bash
printf '%s\n' "${TEST_PROCESSES:-}"
STUB

cat > "$stubs/coder" << 'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TEST_CLIENT_LOG"
[ "${1:-}" = activity ] || exit 64
for pid in ${TEST_ACTIVE:-}; do
    for arg in "$@"; do
        if [ "$arg" = "$pid" ]; then
            printf '%s\n' "${TEST_SENTENCE:-A turn is streaming.}"
            exit 0
        fi
    done
done
printf 'Nothing is running.\n'
exit 1
STUB

cat > "$stubs/notify-send" << 'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TEST_NOTIFY_LOG"
STUB
chmod +x "$stubs"/*

export PATH="$stubs:$PATH"
export TEST_DESK_LOG="$scratch/desk.log"
export TEST_CLIENT_LOG="$scratch/client.log"
export TEST_NOTIFY_LOG="$scratch/notify.log"
export CODER_CLOSE_ARM="$scratch/arm"
export CODER_CLOSE_SECONDS=5

press() { "$script" > /dev/null 2>&1; }
reset() {
    : > "$TEST_DESK_LOG"
    : > "$TEST_CLIENT_LOG"
    : > "$TEST_NOTIFY_LOG"
    rm -f "$CODER_CLOSE_ARM"
    export TEST_ACTIVE=""
}
closed() { grep -q '^close ' "$TEST_DESK_LOG"; }
not_closed() { ! closed; }
no_notice() { [ ! -s "$TEST_NOTIFY_LOG" ]; }
asked_about() { grep -q -- "--pid $1\( \|$\)" "$TEST_CLIENT_LOG"; }
not_asked_about() { ! asked_about "$1"; }

# A terminal window (PID 100) with Coder as its child (140) and a grandchild
# (180), which is the shape of a Coder window on the desktop.
coder_window() {
    export TEST_WINDOW='{"handle":"0x1","pid":100,"app_id":"foot"}'
    export TEST_PROCESSES='  1 0
  100 1
  140 100
  180 140'
}

reset
coder_window
press
check "an idle window closes on the first press" closed
check "an idle window shows no notice" no_notice

reset
export TEST_WINDOW='{"handle":"0x2","pid":300,"app_id":"chromium"}'
export TEST_PROCESSES='  1 0
  300 1
  310 300'
export TEST_ACTIVE="140"
press
check "a window without a Coder session closes on the first press" closed

reset
coder_window
export TEST_ACTIVE="140"
export TEST_SENTENCE="A turn is streaming, and 2 delegations have not reported."
press
check "a busy window does not close on the first press" not_closed
check "the notice says what is running" grep -q "2 delegations have not reported" "$TEST_NOTIFY_LOG"
check "the window and its descendants are asked about" asked_about 100
check "a child is asked about" asked_about 140
check "a grandchild is asked about" asked_about 180
check "a process outside the window is not asked about" not_asked_about 1
press
check "the second press closes the busy window" grep -Fxq "close 0x1" "$TEST_DESK_LOG"

reset
coder_window
export TEST_ACTIVE="140"
printf '0x1 1\n' > "$CODER_CLOSE_ARM"
press
check "a press after the confirmation expires asks again" not_closed

reset
coder_window
export TEST_ACTIVE="140"
press
export TEST_WINDOW='{"handle":"0x9","pid":900,"app_id":"foot"}'
export TEST_PROCESSES='  1 0
  900 1
  940 900'
export TEST_ACTIVE="940"
press
check "a notice for one window does not close another" not_closed

reset
export TEST_WINDOW='{"handle":"0x3","app_id":"foot"}'
export TEST_PROCESSES='  1 0'
press
check "a window with no process closes on the first press" closed

reset
export TEST_WINDOW='null'
check "a press with no focused window succeeds" press

reset
coder_window
export TEST_ACTIVE="140"
CODER_CLOSE_CLIENT="a-client-that-is-not-installed" press
check "a missing client closes the window" closed

reset
coder_window
export TEST_ACTIVE="140"
cat > "$scratch/broken-coder" << 'STUB'
#!/usr/bin/env bash
exit 101
STUB
chmod +x "$scratch/broken-coder"
CODER_CLOSE_CLIENT="$scratch/broken-coder" press
check "a failing client closes the window" closed

if [ "$failures" -eq 0 ]; then
    printf '\nall coder-close tests passed\n'
else
    printf '\n%s coder-close tests failed\n' "$failures" >&2
    exit 1
fi
