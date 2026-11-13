#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around coax, cc, javac and a local HTTP listener; the logic checked is
# crash-reports.c's and crash-dump.coax's)
# Checks the Android app's crash reporter on a desktop (#13379, docs/deployment/crash-reports.md):
# crash-reports.c, https.c and the Coax dump unit, built as the app builds them, run in a JVM against
# stand-ins for the Android classes they call (check/stubs) and a listener in place of the API.
#   apps/shared/host/android/crash-reports-check.sh
# What it holds:
#   - a native crash's tombstone becomes the profile's dump, byte for byte, and is posted once to
#     POST /api/playtest/app-crash;
#   - nothing is sent before a yes; the question is asked once and the answer is kept;
#   - a no sends nothing, then and after; a Java crash and a crash already handled are not sent;
#   - a server that does not answer leaves the crash for the next launch.
# It does not prove the real classes' signatures or a real tombstone: that is a device's check
# (NEEDS_OWNER.md). Needs a JDK (JAVA_HOME, or javac on the path), cc and python3.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
java_home="${JAVA_HOME:-}"
if [[ -z "$java_home" ]]; then j="$(command -v javac || true)"; [[ -n "$j" ]] && java_home="$(dirname "$(dirname "$(readlink -f "$j")")")"; fi
[[ -x "$java_home/bin/javac" ]] || { echo "crash-reports-check: needs a JDK (set JAVA_HOME)" >&2; exit 2; }
jvm="$(dirname "$(find -L "$java_home" -name 'libjvm.*' | head -1)")"
[[ -d "$jvm" ]] || { echo "crash-reports-check: no libjvm under $java_home" >&2; exit 2; }
work="$(mktemp -d)"; server=""
trap '[[ -n "$server" ]] && kill "$server" 2>/dev/null; rm -rf "${work:?}"' EXIT
mkdir -p "$work/classes" "$work/posts"
"$java_home/bin/javac" -d "$work/classes" $(find "$here/check/stubs" -name '*.java')
"$coax" c "$here/crash-dump.coax" --library --header "$work/crash-dump.h" > "$work/crash-dump.c"
os_inc=""; for d in "$java_home"/include/*/; do [[ -f "$d/jni_md.h" ]] && os_inc="${d%/}"; done   # jni_md.h: include/linux or include/darwin
[[ -n "$os_inc" ]] || { echo "crash-reports-check: no jni_md.h under $java_home/include" >&2; exit 2; }
"${CC:-cc}" -std=c11 -O1 -w -D_GNU_SOURCE -I"$work" -I"$here/check/include" -I"$java_home/include" -I"$os_inc" \
  "$here/check/crash-reports-check.c" "$here/crash-reports.c" "$here/https.c" "$work/crash-dump.c" \
  -L"$jvm" -ljvm -lpthread -lm -Wl,-rpath,"$jvm" -o "$work/check"

# The listener in place of the API: each POST to /api/playtest/app-crash is kept as posts/N; while
# posts/busy exists it answers 503.
cat > "$work/listen.py" <<'PY'
import http.server, os, sys
root = sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        busy = os.path.exists(os.path.join(root, "busy"))
        if self.path == "/api/playtest/app-crash" and not busy:
            n = len([f for f in os.listdir(root) if f.isdigit()])
            open(os.path.join(root, str(n)), "wb").write(body)
            open(os.path.join(root, "type"), "w").write(self.headers.get("Content-Type", ""))
        code = 503 if busy else 200 if self.path == "/api/playtest/app-crash" else 404
        out = b'{"kept":true,"bucket":"check"}' if code == 200 else b'{"error":"later"}'
        self.send_response(code); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
s = http.server.HTTPServer(("127.0.0.1", 0), H)
open(os.path.join(root, "port"), "w").write(str(s.server_address[1]))
s.serve_forever()
PY
python3 "$work/listen.py" "$work/posts" & server=$!
for _ in $(seq 100); do [[ -s "$work/posts/port" ]] && break; python3 -c 'import time; time.sleep(0.05)'; done
origin="http://127.0.0.1:$(cat "$work/posts/port")"

# The tombstone crash-dump.coax's check writes (acdt_tombstone), and the dump it makes (ACDT_DUMP).
python3 - "$work/tombstone.bin" <<'PY'
import sys
def var(v):
    out = b""
    while v >= 128: out += bytes([v & 127 | 128]); v >>= 7
    return out + bytes([v])
def num(n, v): return var(n * 8) + var(v)
def lit(n, b): return var(n * 8 + 2) + var(len(b)) + b
def frame(pc, name, build): return lit(4, num(1, pc) + num(2, 0x7a00001000 + pc) + lit(4, b"fn") + lit(6, name) + lit(8, build))
libc = b"/apex/com.android.runtime/lib64/bionic/libc.so"; app = b"/data/app/~~x==/com.openagents.everglade-y==/lib/arm64/libeverglade.so"
t = lit(2, b"google/sdk/emu:15/AB1/1:user/release-keys") + num(5, 4200) + num(6, 4242)
t += lit(10, num(1, 11) + lit(2, b"SIGSEGV") + num(3, 1) + lit(4, b"SEGV_MAPERR") + num(9, 16))
other = num(1, 4243) + lit(2, b"RenderThread") + frame(7777, libc, b"9c6f0d1e2a3b4c5d6e7f8091a2b3c4d5e6f70812")
t += lit(16, num(1, 4243) + lit(2, other))
main = num(1, 4242) + lit(2, b"main") + frame(73184, libc, b"9C6F0D1E2A3B4C5D6E7F8091A2B3C4D5E6F70812")
main += frame(165304, app, b"3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f40") + frame(4096, app, b"3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f40") + frame(24, b"", b"")
t += lit(16, num(1, 4242) + lit(2, main))
open(sys.argv[1], "wb").write(t)
PY
printf 'appdump 1\nt 1763000000000\napp android\nos Android 15 (API 35)\nversion 0.1.0\nbuildno 1\nexe libeverglade.so\nsignal 11\nreason SIGSEGV SEGV_MAPERR\na 9C6F0D1E2A3B4C5D6E7F8091A2B3C4D5E6F70812 73184 libc.so\na 3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f40 165304 libeverglade.so\na 3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f40 4096 libeverglade.so\na 0 24 ?\n' > "$work/want.dump"

fail() { echo "crash-reports-check: FAILED: $*" >&2; exit 1; }
posts() { find "$work/posts" -maxdepth 1 -type f -regex '.*/[0-9]+' | wc -l | tr -d ' '; }
run() { "$work/check" "$work/classes" "$@" 2> "$work/log"; }   # DIR ORIGIN RECORDS ANSWER WAIT_SEEN
crash="5:1763000000000:11:crash:$work/tombstone.bin"
java="4:1763000000500:0:java:"; old="5:1000:6:old:"

# 1. A first crash, no answer yet: asked, and nothing leaves.
d="$work/a"; mkdir "$d"
got="$(run "$d" "$origin" "$java;$crash;$old" none -)"
[[ "$got" == $'asked\nseen -\nconsent -' ]] || fail "a first crash should ask and keep nothing: $got"
[[ "$(posts)" == 0 ]] || fail "a dump left before the answer"
# 2. The same launch again, and Send: the two native crashes are posted, the first byte for byte the
#    profile's dump; the answer and the newest crash's time are kept. The Java crash is not sent.
got="$(run "$d" "$origin" "$java;$crash;$old" send 1763000000000)"
[[ "$got" == $'asked\nseen 1763000000000\nconsent yes' ]] || fail "Send should post and keep the answer: $got"
[[ "$(posts)" == 2 ]] || fail "two native crashes, $(posts) posts"
cmp -s "$work/posts/0" "$work/want.dump" || fail "the posted dump is not the profile's: $(head -c 300 "$work/posts/0")"
[[ "$(cat "$work/posts/1")" == $'appdump 1\nt 1000\napp android\nos Android 15 (API 35)\nversion 0.1.0\nbuildno 1\nexe libeverglade.so\nsignal 6\nreason old' ]] || fail "a crash with no tombstone: $(cat "$work/posts/1")"
[[ "$(cat "$work/posts/type")" == "text/plain; charset=utf-8" ]] || fail "the post's content type"
grep -q "appdump\|libeverglade" "$work/log" && fail "the log holds a dump"
# 3. The next launch, the same records: nothing new, not asked, nothing sent.
got="$(run "$d" "$origin" "$java;$crash;$old" none -)"
[[ "$got" == $'not asked\nseen 1763000000000\nconsent yes' && "$(posts)" == 2 ]] || fail "a crash was handled twice: $got, $(posts) posts"
# 4. A newer crash, the answer kept: sent with no question.
got="$(run "$d" "$origin" "5:1763000009000:7:bus:;$java;$crash" none 1763000009000)"
[[ "$got" == $'not asked\nseen 1763000009000\nconsent yes' && "$(posts)" == 3 ]] || fail "a later crash should go with no question: $got, $(posts) posts"
grep -q '^signal 7$' "$work/posts/2" || fail "the later crash's signal"
# 5. The server answers 503: the crash stays for the next launch, which sends it.
touch "$work/posts/busy"
got="$(run "$d" "$origin" "5:1763000010000:11:again:;$crash" none -)"
[[ "$got" == $'not asked\nseen 1763000009000\nconsent yes' && "$(posts)" == 3 ]] || fail "an unanswered post must stay: $got"
rm "$work/posts/busy"
got="$(run "$d" "$origin" "5:1763000010000:11:again:;$crash" none 1763000010000)"
[[ "$got" == $'not asked\nseen 1763000010000\nconsent yes' && "$(posts)" == 4 ]] || fail "the next launch should send it: $got, $(posts) posts"
# 6. No server at all: the same.
got="$(run "$d" "http://127.0.0.1:9" "5:1763000011000:11:x:" none -)"
[[ "$got" == $'not asked\nseen 1763000010000\nconsent yes' ]] || fail "no server: $got"
# 7. Another phone: Don't Send. Nothing leaves, then or on a later crash, and it is not asked again.
d="$work/b"; mkdir "$d"
got="$(run "$d" "$origin" "$crash" dont 1763000000000)"
[[ "$got" == $'asked\nseen 1763000000000\nconsent no' && "$(posts)" == 4 ]] || fail "Don't Send should send nothing: $got, $(posts) posts"
got="$(run "$d" "$origin" "5:1763000020000:11:later:$work/tombstone.bin;$crash" none 1763000020000)"
[[ "$got" == $'not asked\nseen 1763000020000\nconsent no' && "$(posts)" == 4 ]] || fail "after a no: $got, $(posts) posts"
# 8. No crash at all, only other exits: nothing asked, nothing kept.
d="$work/c"; mkdir "$d"
got="$(run "$d" "$origin" "$java;10:5:0:user:;6:7:0:anr:" none -)"
[[ "$got" == $'not asked\nseen -\nconsent -' && "$(posts)" == 4 ]] || fail "no native crash: $got"
# 9. A tombstone that cannot be read: the crash is still sent, from its exit record.
d="$work/e"; mkdir "$d"
got="$(run "$d" "$origin" "5:1763000030000:11:unreadable:$work/tombstone.bin" send 1763000030000)"
[[ "$got" == $'asked\nseen 1763000030000\nconsent yes' && "$(posts)" == 5 ]] || fail "an unreadable tombstone: $got, $(posts) posts"
[[ "$(cat "$work/posts/4")" == $'appdump 1\nt 1763000030000\napp android\nos Android 15 (API 35)\nversion 0.1.0\nbuildno 1\nexe libeverglade.so\nsignal 11\nreason unreadable' ]] || fail "its dump: $(cat "$work/posts/4")"
echo "crash-reports-check: ok (9 launches: asked once, nothing before a yes, a no sends nothing, each crash once, an unanswered post waits)"
