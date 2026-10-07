#!/usr/bin/env bash
#
# Tests for `os/bin/coder-pane`, the window Super+Return, Super+T, and the
# session's first window open on a CoderOS desktop. Run it by hand:
#
#   os/tests/coder-pane.sh
#
# There is no terminal emulator here, so the emulator is a stub that prints
# the arguments it was run with.
#
# What the tests hold:
#
# - Super+Return opens the emulator running Coder, and Super+T opens it on a
#   bare shell.
# - A pane carries the app-id and the title it was given.
# - The emulator reads the Coder palette and the font from the file the
#   session writes, and opens on its own configuration when the file is not
#   there.
set -euo pipefail

# A CoderOS host sets these for every session, through `coderos.desktop` in
# `os/`, so a test run there inherits them. Each case names what it grants,
# and the cases that grant nothing read the script's own defaults.
unset CODER_PANE_TERMINAL CODER_PANE_CLIENT CODER_PANE_DIRECTORY
unset CODER_PANE_TERMINAL_CONFIG

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/coder-pane"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-pane-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

stubs="$scratch/stubs"
mkdir -p "$stubs"
for emulator in foot kitty; do
  cat > "$stubs/$emulator" << 'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*"
STUB
  chmod +x "$stubs/$emulator"
done

# The tools a run needs, linked into one directory. A run gives the stub
# directory and this one as the whole PATH, so nothing on this machine
# answers by accident. The stubs start with `#!/usr/bin/env bash`, so `env`
# looks `bash` up on that PATH, and a PATH of `/usr/bin:/bin` finds none on
# a NixOS host, where `bash` sits in the system profile.
tools="$scratch/tools"
mkdir -p "$tools"
for tool in bash env; do
  resolved=$(command -v "$tool" || true)
  if [ -z "$resolved" ]; then
    echo "FAIL $tool is missing, and these tests need it" >&2
    exit 1
  fi
  ln -s "$resolved" "$tools/$tool"
done

# The palette file the session writes, and a path where none is. A CoderOS
# host has `/etc/coderos/foot.ini`, so every run names one or the other
# rather than reading the host's.
palette="$scratch/foot.ini"
printf 'font=Paper Mono:size=14\n' > "$palette"
no_palette="$scratch/absent/foot.ini"

# One run of the script on a host that wrote no palette file.
ran() {
  PATH="$stubs:$tools" CODER_PANE_TERMINAL=foot CODER_PANE_TERMINAL_CONFIG="$no_palette" \
    bash -euo pipefail "$script" "$@"
}

# The same run on a host that wrote the palette file.
ran_with_palette() {
  PATH="$stubs:$tools" CODER_PANE_TERMINAL=foot CODER_PANE_TERMINAL_CONFIG="$palette" \
    bash -euo pipefail "$script" "$@"
}

said=$(ran)
if [ "$said" = "-- coder" ]; then
  pass "Super+Return opens the emulator running Coder"
else
  fail "Super+Return opens the emulator running Coder: $said"
fi

said=$(ran --shell)
if [ "$said" = "" ]; then
  pass "Super+T opens the emulator on a bare shell"
else
  fail "Super+T opens the emulator on a bare shell: $said"
fi

said=$(ran --app-id=coder-work --title='coder · read the audit')
if [ "$said" = "--app-id=coder-work --title=coder · read the audit -- coder" ]; then
  pass "a pane carries the app-id and the title it was given"
else
  fail "a pane carries the app-id and the title it was given: $said"
fi

said=$(ran --app-id coder-work --title 'two words')
if [ "$said" = "--app-id=coder-work --title=two words -- coder" ]; then
  pass "the flags take their value as the next argument too"
else
  fail "the flags take their value as the next argument too: $said"
fi

said=$(PATH="$stubs:$tools" CODER_PANE_TERMINAL=foot CODER_PANE_TERMINAL_CONFIG="$no_palette" \
  CODER_PANE_CLIENT=/opt/coder/bin/coder bash -euo pipefail "$script")
if [ "$said" = "-- /opt/coder/bin/coder" ]; then
  pass "the client the session names is the one a pane runs"
else
  fail "the client the session names is the one a pane runs: $said"
fi

# --- The palette and the font -------------------------------------------
said=$(ran_with_palette)
if [ "$said" = "--config=$palette -- coder" ]; then
  pass "a pane reads the palette file the session wrote"
else
  fail "a pane reads the palette file the session wrote: $said"
fi

said=$(ran_with_palette --shell)
if [ "$said" = "--config=$palette" ]; then
  pass "a shell pane opens in the Coder palette"
else
  fail "a shell pane opens in the Coder palette: $said"
fi

# The flag is foot's. Another emulator the session names gets the line
# without it, whether or not the file is there.
said=$(PATH="$stubs:$tools" CODER_PANE_TERMINAL=kitty \
  CODER_PANE_TERMINAL_CONFIG="$palette" bash -euo pipefail "$script")
if [ "$said" = "-- coder" ]; then
  pass "an emulator that is not foot gets no palette flag"
else
  fail "an emulator that is not foot gets no palette flag: $said"
fi

# --- The directory the session names -------------------------------------
said=$(PATH="$stubs:$tools" CODER_PANE_TERMINAL=foot \
  CODER_PANE_TERMINAL_CONFIG="$no_palette" \
  CODER_PANE_DIRECTORY=/srv/checkouts/openagents bash -euo pipefail "$script")
if [ "$said" = "--working-directory=/srv/checkouts/openagents -- coder" ]; then
  pass "the directory the session names opens the window there"
else
  fail "the directory the session names opens the window there: $said"
fi

# --- A flag this does not take is refused ---------------------------------
for flag in --wat --attach; do
  status=0
  PATH="$stubs:$tools" bash -euo pipefail "$script" "$flag" > /dev/null 2>&1 || status=$?
  if [ "$status" = "2" ]; then
    pass "$flag is refused"
  else
    fail "$flag is refused: exit $status"
  fi
done

if [ "$failures" -gt 0 ]; then
  printf '\n%s coder-pane test(s) failed\n' "$failures" >&2
  exit 1
fi
printf '\nall passed\n'
