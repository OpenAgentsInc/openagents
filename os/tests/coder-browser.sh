#!/usr/bin/env bash
#
# Tests for the browser launcher on a CoderOS desktop, `os/bin/coder-browser`,
# which `SUPER + B` runs and which every `xdg-open` on the host resolves to.
#
# Run it by hand:
#
#   os/tests/coder-browser.sh
#
# There is no desk or browser here, so `coder-desk`, `chromium`, and `curl`
# are files under a scratch directory that record what they were given, and
# every assertion reads an exit code or those records.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/os/bin/coder-browser"

if ! command -v jq > /dev/null 2>&1; then
  echo "FAIL jq is missing, and these tests need it" >&2
  exit 1
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/coder-browser-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }

runner="$scratch/coder-browser"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$script"
} > "$runner"
chmod +x "$runner"

bin="$scratch/bin"
clients="$scratch/clients.json"
chromium_args="$scratch/chromium-args"
curled="$scratch/curled"
dispatched="$scratch/dispatched"

reset_host() {
  rm -rf "$bin" "$chromium_args" "$curled" "$dispatched"
  mkdir -p "$bin"
  printf '[]\n' > "$clients"
  cat > "$bin/coder-desk" << DESK
#!/usr/bin/env bash
case "\$1" in
  list) cat "$clients" ;;
  *) printf '%s\n' "\$*" >> "$dispatched" ;;
esac
exit 0
DESK
  chmod +x "$bin/coder-desk"
  cat > "$bin/chromium" << CHROMIUM
#!/usr/bin/env bash
printf '%s\n' "\$@" > "$chromium_args"
exit 0
CHROMIUM
  chmod +x "$bin/chromium"
  cat > "$bin/curl" << CURL
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$curled"
exit 0
CURL
  chmod +x "$bin/curl"
}

# With no window open, Chromium starts with the port the host set and the
# URL on its command line.
reset_host
PATH="$bin:$PATH" CODER_BROWSER_PORT=9333 "$runner" 'https://x.test/?a=1&b=2'
if grep -Fxq -- '--remote-debugging-port=9333' "$chromium_args" \
  && grep -Fxq -- '--remote-debugging-address=127.0.0.1' "$chromium_args" \
  && grep -Fxq 'https://x.test/?a=1&b=2' "$chromium_args" \
  && [ ! -e "$curled" ]; then
  pass "a cold start passes the URL and the host's port to chromium"
else
  fail "a cold start lost the URL or the port: $(cat "$chromium_args" 2>/dev/null)"
fi

# With no port set, the launcher uses the module's default.
reset_host
(unset CODER_BROWSER_PORT; PATH="$bin:$PATH" "$runner")
if grep -Fxq -- '--remote-debugging-port=9222' "$chromium_args"; then
  pass "a launcher with no port set listens on 9222"
else
  fail "a launcher with no port set listened elsewhere: $(cat "$chromium_args" 2>/dev/null)"
fi

# With the window open, a URL opens as a new tab over the DevTools port and
# the window is focused; no second Chromium starts.
reset_host
printf '[{"app_id":"chromium-browser","title":"Chromium"}]\n' > "$clients"
PATH="$bin:$PATH" CODER_BROWSER_PORT=9222 "$runner" 'https://x.test/?a=1&b=2'
if grep -Fxq -- '-s -X PUT http://127.0.0.1:9222/json/new?https://x.test/?a=1&b=2' "$curled" \
  && grep -Fxq 'focus class:chromium-browser' "$dispatched" \
  && [ ! -e "$chromium_args" ]; then
  pass "an open window takes the URL as a new tab over DevTools and is focused"
else
  fail "an open window did not take the URL: curl=$(cat "$curled" 2>/dev/null) desk=$(cat "$dispatched" 2>/dev/null) chromium=$(cat "$chromium_args" 2>/dev/null)"
fi

# With the window open and no URL, the launcher only focuses.
reset_host
printf '[{"app_id":"chromium-browser","title":"Chromium"}]\n' > "$clients"
PATH="$bin:$PATH" CODER_BROWSER_PORT=9222 "$runner"
if [ ! -e "$curled" ] && grep -q '^focus ' "$dispatched" && [ ! -e "$chromium_args" ]; then
  pass "an open window with no URL is focused and nothing else"
else
  fail "focus without a URL did something else: curl=$(cat "$curled" 2>/dev/null) chromium=$(cat "$chromium_args" 2>/dev/null)"
fi

# A flag is not a URL.
reset_host
printf '[{"app_id":"chromium-browser","title":"Chromium"}]\n' > "$clients"
PATH="$bin:$PATH" CODER_BROWSER_PORT=9222 "$runner" --incognito
if [ ! -e "$curled" ]; then
  pass "a flag with the window open opens no tab"
else
  fail "a flag was sent to DevTools as a URL: $(cat "$curled")"
fi

# `os/bin/coder-open-url` hands a URL from a sandbox to the desk, which runs
# the launcher in the host's session. Wine passes the URL inside a longer
# argument, and a quote in an argument that holds no web address survives
# the shell the session runs.
open_url="$scratch/coder-open-url"
{
  printf '#!/usr/bin/env bash\nset -o errexit\nset -o nounset\nset -o pipefail\n'
  cat "$root/os/bin/coder-open-url"
} > "$open_url"
chmod +x "$open_url"

reset_host
PATH="$bin:$PATH" "$open_url" 'start "" https://x.test/sign-in?next=/a'
if grep -Fxq "open -- coder-browser 'https://x.test/sign-in?next=/a'" "$dispatched"; then
  pass "a URL inside a Wine argument reaches the launcher through the desk"
else
  fail "the URL did not reach the desk as asked: $(cat "$dispatched" 2>/dev/null)"
fi

reset_host
PATH="$bin:$PATH" "$open_url" "x.test/it's"
if grep -Fxq "open -- coder-browser 'x.test/it'\\''s'" "$dispatched"; then
  pass "a quote inside the URL is escaped for the session's shell"
else
  fail "a quote inside the URL was not escaped: $(cat "$dispatched" 2>/dev/null)"
fi

status=0
PATH="$bin:$PATH" "$open_url" > /dev/null 2>&1 || status=$?
if [ "$status" = 2 ]; then
  pass "coder-open-url with no URL exits 2"
else
  fail "coder-open-url with no URL exited $status"
fi

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)" >&2
  exit 1
fi
