#!/usr/bin/env bash
# Firm reason: 5. The check that a remote.sh call leaves nothing behind (#13181): no
# scratch folder, no ref, nothing new in the checkout, whether the call ends
# normally, fails, falls back to this computer or is interrupted (INT, TERM, HUP),
# and with calls side by side, each removing only its own. It also checks that the
# snapshot leaves out build-remote/ and that a TMPDIR inside the checkout is not
# used. No host, no network: a scratch checkout, a fake gcloud, OA_REMOTE_DRY_RUN.
#   coax/remote/test-remote.sh        exit 0 when every case passes
set -uo pipefail
set -m   # background calls get their own process groups and keep SIGINT (a script's & ignores it)
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
w="$(mktemp -d "${TMPDIR:-/tmp}/oa-remote-test.XXXXXXXX")" || exit 2
trap 'rm -rf "${w:?}"' EXIT
fails=0
ok() { echo "ok: $*"; }
bad() { echo "FAIL: $*"; fails=$((fails + 1)); }

# A checkout with remote.sh in it and an origin/main, untracked files, and
# build-remote/ folders (at the top and deeper) that must never be sent.
repo="$w/repo"; mkdir -p "$repo/coax/remote" "$repo/a/build-remote/old" "$repo/build-remote/oa-remote.x"
cp "$here/remote.sh" "$here/host-job.sh" "$here/shared-cache.sh" "$repo/coax/remote/"
(cd "$repo" && git init -q && git -c user.name=t -c user.email=t@t add -A && git -c user.name=t -c user.email=t@t commit -qm base && git update-ref refs/remotes/origin/main HEAD) || exit 2
echo new > "$repo/a/untracked.txt"
echo junk > "$repo/build-remote/oa-remote.x/bundle"; echo junk > "$repo/a/build-remote/old/bundle"
# The state folder with a key already made, a fake gcloud that waits until it is
# stopped, and a HOME with no build slots (the fallback runs the command itself).
export OA_REMOTE_STATE="$w/state" COAX_SHARED_CACHE=0 HOME="$w/home" OA_REMOTE_DAILY_USD=100
unset OA_REMOTE OA_REMOTE_HOST OA_REMOTE_DRY_RUN CLOUDSDK_CONFIG
mkdir -p "$OA_REMOTE_STATE" "$HOME" "$w/bin" "$w/pids"
: > "$OA_REMOTE_STATE/ssh_ed25519"; : > "$OA_REMOTE_STATE/ssh_ed25519.pub"
cat > "$w/bin/gcloud" <<'EOF'
#!/usr/bin/env bash
echo $$ > "$FAKE_PIDS/${FAKE_TAG:-gcloud}.pid"
exec sleep 120
EOF
chmod +x "$w/bin/gcloud"
export PATH="$w/bin:$PATH" FAKE_PIDS="$w/pids"
tmp="$w/tmp"; mkdir -p "$tmp"
before="$(cd "$repo" && git status --porcelain --ignored | sort)"

clean() {   # case: nothing left in the scratch folder, no ref, the checkout unchanged
  local left; left="$(ls -A "$tmp" | grep -v '^oa-remote\.other$')"
  [[ -z "$left" ]] || bad "$1: left in TMPDIR: $left"
  [[ -z "$(git -C "$repo" for-each-ref refs/oa-remote)" ]] || bad "$1: left a ref: $(git -C "$repo" for-each-ref refs/oa-remote)"
  [[ "$(cd "$repo" && git status --porcelain --ignored | sort)" == "$before" ]] || bad "$1: the checkout changed"
}
call() { (cd "$repo/a" && TMPDIR="$tmp" bash "$repo/coax/remote/remote.sh" "$@"); }

# 1. The snapshot: the untracked file goes, build-remote/ does not.
out="$(OA_REMOTE_DRY_RUN=1 call run -- true 2>&1)"; s=$?
tree="$(sed -n 's/^snapshot \([0-9a-f]*\) .*/\1/p' <<< "$out")"
if [[ $s == 0 && -n "$tree" ]]; then
  files="$(git -C "$repo" ls-tree -r --name-only "$tree")"
  grep -qx 'a/untracked.txt' <<< "$files" && ! grep -q 'build-remote' <<< "$files" \
    && ok "the snapshot has the untracked file and no build-remote/" || bad "snapshot files: $files"
else bad "dry run: status $s: $out"; fi
clean "a dry run"

# 2. Failure, with and without the fallback.
OA_REMOTE_DAILY_USD=0 call run --no-fallback -- true > /dev/null 2>&1; s=$?
[[ $s == 97 ]] && ok "over budget with --no-fallback exits 97" || bad "over budget with --no-fallback: status $s"
clean "a failed call"
OA_REMOTE_DAILY_USD=0 call run -- true > /dev/null 2>&1; s=$?
[[ $s == 0 ]] && ok "over budget falls back to this computer" || bad "the fallback: status $s"
clean "a call that fell back"

# 3. Interrupted while it waits for a host: the scratch folder goes with it.
interrupt() {   # signal status
  rm -f "$w/pids"/*.pid
  (cd "$repo/a" && FAKE_TAG=one OA_REMOTE_HOST=fake TMPDIR="$tmp" exec bash "$repo/coax/remote/remote.sh" run --no-fallback -- true) > /dev/null 2>&1 &
  local pid=$! n=0
  while [[ ! -s "$w/pids/one.pid" ]] && (( n < 200 )); do sleep 0.05; n=$((n + 1)); done
  [[ -s "$w/pids/one.pid" ]] || { bad "SIG$1: the call never asked for a host"; kill "$pid" 2> /dev/null; wait "$pid"; return; }
  [[ -n "$(ls -A "$tmp" | grep -v '^oa-remote\.other$')" ]] || bad "SIG$1: no scratch folder while the call ran"
  kill -s "$1" "$pid"; kill "$(cat "$w/pids/one.pid")" 2> /dev/null
  wait "$pid"; local s=$?
  [[ $s == "$2" ]] && ok "SIG$1 ends the call with status $2" || bad "SIG$1: status $s, not $2"
  clean "SIG$1"
}
interrupt INT 130; interrupt TERM 143; interrupt HUP 129

# 4. Side by side: each call removes only its own folder. Another call's folder
# (oa-remote.other) stays; a waiting call's folder stays until it ends.
mkdir -p "$tmp/oa-remote.other"; echo theirs > "$tmp/oa-remote.other/spec"
rm -f "$w/pids"/*.pid
(cd "$repo/a" && FAKE_TAG=held OA_REMOTE_HOST=fake TMPDIR="$tmp" exec bash "$repo/coax/remote/remote.sh" run --no-fallback -- true) > /dev/null 2>&1 &
held=$!; n=0
while [[ ! -s "$w/pids/held.pid" ]] && (( n < 200 )); do sleep 0.05; n=$((n + 1)); done
mine="$(ls -A "$tmp" | grep -v '^oa-remote\.other$')"
pids=()
for i in 1 2 3 4 5 6; do (OA_REMOTE_DRY_RUN=1 call run -- true > /dev/null 2>&1) & pids+=($!); done
for i in 1 2 3; do (OA_REMOTE_DAILY_USD=0 call run --no-fallback -- true > /dev/null 2>&1) & pids+=($!); done
for p in "${pids[@]}"; do wait "$p"; done
[[ "$(ls -A "$tmp" | grep -v '^oa-remote\.other$')" == "$mine" && -n "$mine" ]] \
  && ok "nine calls side by side left only the waiting call's folder" || bad "side by side: $(ls -A "$tmp")"
[[ "$(cat "$tmp/oa-remote.other/spec" 2> /dev/null)" == theirs ]] && ok "another call's folder is untouched" || bad "another call's folder was touched"
kill -s INT "$held"; kill "$(cat "$w/pids/held.pid")" 2> /dev/null; wait "$held"
clean "side by side"
rm -rf "${tmp:?}/oa-remote.other"

# 5. A TMPDIR inside the checkout is not used: the folder would be in the snapshot.
mkdir -p "$repo/build-remote/tmp"
out="$(cd "$repo/a" && OA_REMOTE_DRY_RUN=1 TMPDIR="$repo/build-remote/tmp" bash "$repo/coax/remote/remote.sh" run -- true 2>&1)"; s=$?
[[ $s == 0 && -z "$(ls -A "$repo/build-remote/tmp")" ]] && ok "a TMPDIR inside the checkout is not used" || bad "TMPDIR inside the checkout: status $s, left $(ls -A "$repo/build-remote/tmp")"
rmdir "$repo/build-remote/tmp"
clean "a TMPDIR inside the checkout"

if (( fails )); then echo "test-remote.sh: $fails failed"; exit 1; fi
echo "test-remote.sh: every case passed"
