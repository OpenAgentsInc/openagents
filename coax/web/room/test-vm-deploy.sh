#!/usr/bin/env bash
# Firm reason: 5. The check of vm-deploy.sh's release rules (#13398) against a stand-in for the room
# VM: a scratch folder for /opt/oa-room, a sudo and a systemctl that only record, no gcloud, no SSH,
# no network (OA_ROOM_VM_STANDIN, which vm-deploy.sh reads). The rules themselves are
# coax/web/release-set-policy.coax; this checks that the script asks them and obeys them.
#   coax/web/room/test-vm-deploy.sh     exit 0 when every case passes (it needs the Coax compiler)
# The cases:
#   1. a release keeps its content tables beside its binary; one staged without tables is refused
#   2. a promotion names its release: none named is refused, and nothing in production moves
#   3. a promotion takes the release it names only while staging runs it: after another deploy to
#      staging (the 2026-11-12 incident, #13397) it is refused, by `deploy` and by `canary`
#   4. a seeded bad build is never promoted
#   5. rooms whose wire range leaves out the live page's version are not promoted; an unread page
#      is not held against them; rooms whose range can't be read are not promoted
#   5b. the same for the three pins (#13408): rooms that seat neither the live page's build nor a
#      release before that is it are not promoted; unread rooms' pins are not promoted; an unread
#      page is not held against them
#   6. the staging lock: while a release holds it, deploy --staging, rollback --staging and the
#      API's deploy refuse anyone else and name the holder; the holder's token passes; a second
#      taker is refused; only the holder's token releases it
#   7. a stale lock (its process gone on this computer, or past its time) is said to be stale, is
#      cleared by `lock clear`, and no longer stops a deploy; a live one is not cleared
#   8. the rollback target after a re-promote: a repair (--repair) keeps it; the release a rollback
#      leaves is marked bad and is never a target again; `previous NAME` sets one and refuses a bad,
#      seeded, missing or running release
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
w="$(mktemp -d "${TMPDIR:-/tmp}/oa-vm-deploy-test.XXXXXXXX")" || exit 2
holder=""
trap '[[ -z "$holder" ]] || kill "$holder" 2> /dev/null; rm -rf "${w:?}"' EXIT
fails=0
ok() { echo "ok: $*"; }
bad() { echo "FAIL: $*"; fails=$((fails + 1)); }
vm="$w/vm"; o="$vm/opt/oa-room"
mkdir -p "$vm/bin" "$o/releases" "$o/bin" "$vm/stage/src"
printf '#!/bin/sh\nexec "$@"\n' > "$vm/bin/sudo"
printf '#!/bin/sh\necho "systemctl $*" >> "%s/calls.log"\n[ "$1" = is-active ] && echo active\nexit 0\n' "$vm" > "$vm/bin/systemctl"
chmod +x "$vm/bin/sudo" "$vm/bin/systemctl"
printf 'aethyr.content.v1 everglade 1\nreach 100000\n' > "$vm/stage/src/everglade.content"
: > "$vm/calls.log"
# vm-deploy.sh on the stand-in; its release is named $name
name=r0
v() { OA_ROOM_VM_STANDIN="$vm" OA_ROOM_VM_STANDIN_NAME="$name" OA_STAGING_LOCK="" "$here/vm-deploy.sh" "$@" > "$w/out" 2>&1; }
stage() { name="$1"; shift; v deploy --staging "$@"; }
link() { [[ -L "$o/$1" ]] && basename "$(readlink "$o/$1")" || echo ""; }
said() { grep -q -- "$1" "$w/out"; }
show() { sed 's/^/    | /' "$w/out"; }
expect() {   # WHAT STATUS-WANTED(0|1) TEXT...: the last v's status and words
  local what="$1" want="$2" st="$3" t; shift 3
  if [[ "$want" == 0 && "$st" != 0 ]] || [[ "$want" == 1 && "$st" == 0 ]]; then bad "$what (status $st)"; show; return; fi
  for t in "$@"; do said "$t" || { bad "$what: it did not say \"$t\""; show; return; }; done
  ok "$what"
}

# ---- 1. the content tables
stage r1; st=$?
expect "a release staged with its tables is put on staging" 0 $st "deploy --release r1"
[[ -s "$o/releases/r1/everglade.content" && ! -e "$o/releases/r1/src" && "$(link staging)" == r1 ]] && ok "its tables are beside its binary, and its sources are gone" || bad "r1's tables are not beside its binary"
mv "$vm/stage/src/everglade.content" "$w/tables"
stage r1x; st=$?
expect "a release staged without tables is refused" 1 $st "no content tables"
[[ "$(link staging)" == r1 ]] && ok "staging still runs r1" || bad "staging moved to a release without tables"
mv "$w/tables" "$vm/stage/src/everglade.content"

# ---- 2. a promotion names its release
v deploy; st=$?
expect "a promotion that names no release is refused" 1 $st "no release is named" "deploy --release r1"
[[ -z "$(link current)" ]] && ok "production did not move" || bad "production moved without a named release"
v deploy --release r1; st=$?
expect "the named release, which staging runs, is promoted" 0 $st "production's release is r1"
[[ "$(link current)" == r1 && "$(readlink "$o/bin/s6")" == "$o/current/room" && "$(grep -c '^drain ' "$vm/calls.log")" == 6 ]] && ok "all six shards were drained onto r1" || bad "the shards are not on r1"

# ---- 3. staging moved since the release was staged
stage r2 > /dev/null   # the release's own build
stage r3 > /dev/null   # another agent's deploy to staging, between the canary and the promotion
v deploy --release r2; st=$?
expect "a promotion is refused when staging no longer runs its release" 1 $st "REFUSED" "someone deployed to staging since" "Staging runs r3" "names r2"
[[ "$(link current)" == r1 ]] && ok "production still runs r1" || bad "production took $(link current)"
v canary 3 --release r2; st=$?
expect "a canary is refused the same way" 1 $st "someone deployed to staging since"
[[ ! -e "$o/canary" ]] && ok "no shard became a canary" || bad "a canary was put out"
v canary 3; st=$?
expect "a canary that names no release is refused" 1 $st "no release is named"
v canary 3 --release r3; st=$?
expect "a canary of the release staging runs goes out" 0 $st "shard 3 is the canary on r3"
[[ "$(cat "$o/canary")" == "3 r3" && "$(readlink "$o/bin/s3")" == "$o/releases/r3/room" ]] && ok "shard 3 runs r3" || bad "shard 3 is not on r3"
v deploy --release r3; st=$?
expect "the rest follow, and the release it replaces is the rollback target" 0 $st "the rollback target is r1" "shard 3 is the canary"
[[ "$(link current)" == r3 && "$(link previous)" == r1 && ! -e "$o/canary" ]] && ok "production runs r3; previous is r1" || bad "current $(link current), previous $(link previous)"

# ---- 4. a seeded bad build
stage r4 --seed-tick-us 100 > /dev/null
[[ "$(link staging)" == r4-seeded ]] || bad "the seeded build is not on staging"
v deploy --release r4-seeded; st=$?
expect "a seeded bad build is never promoted" 1 $st "seeded bad build"
v rollback --staging; st=$?
expect "staging goes back to r3" 0 $st "staging runs r3 again"

# ---- 5. the rooms' wire range beside the live page
stage r5 > /dev/null
echo 6-7 > "$vm/rooms-wire"; echo 5 > "$vm/page-wire"
v deploy --release r5; st=$?
expect "rooms two wire versions ahead of the live page are not promoted" 1 $st "REFUSED" "speaks wire 6 to 7" "speaks wire 5"
[[ "$(link current)" == r3 ]] && ok "production still runs r3" || bad "production took $(link current)"
echo 6-6 > "$vm/rooms-wire"; echo 7 > "$vm/page-wire"
v deploy --release r5; st=$?
expect "rooms behind the live page are not promoted" 1 $st "speaks wire 6 to 6"
echo 0-0 > "$vm/rooms-wire"; echo 5 > "$vm/page-wire"
v deploy --release r5; st=$?
expect "rooms whose wire range can't be read are not promoted" 1 $st "could not be read"
echo 6-7 > "$vm/rooms-wire"; echo 0 > "$vm/page-wire"
v canary 2 --release r5; st=$?
expect "an unread live page is not held against a promotion" 0 $st "could not be read: not held against it"
# ---- 5b. the rooms' three pins beside the live page (#13408); the wire is in range throughout
echo 5-6 > "$vm/rooms-wire"; echo 5 > "$vm/page-wire"
echo 1,1,3 > "$vm/rooms-pins"; echo 1,1,2 > "$vm/rooms-before"; echo 1,1,1 > "$vm/page-pins"
v deploy --release r5; st=$?
expect "rooms two content revisions ahead of the live page are not promoted" 1 $st "REFUSED" "runs engine 1 rules 1 content 3" "seats the release before's 1 1 2" "says engine 1 rules 1 content 1"
[[ "$(link current)" == r3 ]] && ok "production still runs r3" || bad "production took $(link current)"
echo 2,2,2 > "$vm/rooms-pins"; echo 1,1,1 > "$vm/rooms-before"; echo 2,1,1 > "$vm/page-pins"
v canary 3 --release r5; st=$?
expect "a live page on a mix of the two builds is not seated: no canary either" 1 $st "REFUSED" "says engine 2 rules 1 content 1"
echo 0,0,0 > "$vm/rooms-pins"; echo 1,1,1 > "$vm/rooms-before"; echo 1,1,1 > "$vm/page-pins"
v deploy --release r5; st=$?
expect "rooms whose pins can't be read are not promoted" 1 $st "pins r5 runs and seats could not be read"
echo 1,2,2 > "$vm/rooms-pins"; echo 0,0,0 > "$vm/rooms-before"; echo 1,2,2 > "$vm/page-pins"
v deploy --release r5; st=$?
expect "rooms whose seats-before pins can't be read are not promoted" 1 $st "could not be read"
echo 1,2,2 > "$vm/rooms-pins"; echo 1,1,1 > "$vm/rooms-before"; echo 0,0,0 > "$vm/page-pins"
v canary 2 --release r5; st=$?
expect "an unread live page's pins are not held against a promotion" 0 $st "pins could not be read: not held against it"
echo 1,2,2 > "$vm/rooms-pins"; echo 1,1,1 > "$vm/rooms-before"; echo 1,1,1 > "$vm/page-pins"
v deploy --release r5; st=$?
expect "rooms that speak the live page's version and seat its build as the release before's are promoted" 0 $st "speaks wire 5 to 6, and the live page speaks 5" "it seats engine 1 rules 1 content 1 (its own 1 2 2), and the live page says 1 1 1" "the rollback target is r3"
rm -f "$vm/rooms-pins" "$vm/rooms-before" "$vm/page-pins"

# ---- 6. the staging lock
sleep 300 & holder=$!
OA_STAGING_LOCK_PID=$holder v lock take "release-set abc.1 by tester"; st=$?
token="$(sed -n 's/^lock-token //p' "$w/out")"
[[ $st == 0 && ${#token} == 24 ]] && ok "a release takes staging's lock" || { bad "the lock was not taken"; show; }
stage r6; st=$?
expect "another deploy to staging is refused while the release holds it" 1 $st "REFUSED" "release-set abc.1 by tester" "process $holder"
[[ "$(link staging)" == r5 && ! -e "$o/releases/r6" ]] && ok "staging still runs r5, and nothing was built" || bad "staging moved under the lock"
v rollback --staging; st=$?
expect "a rollback of staging by another is refused" 1 $st "release-set abc.1 by tester"
OA_ROOM_VM_STANDIN="$vm" OA_STAGING_LOCK="" "$here/../api/deploy.sh" --staging > "$w/out" 2>&1; st=$?
expect "the API's staging deploy is refused, naming the holder" 1 $st "release-set abc.1 by tester" "api: REFUSED"
OA_ROOM_VM_STANDIN="$vm" OA_STAGING_LOCK="" "$here/../api/deploy.sh" > "$w/out" 2>&1; st=$?
expect "and so is the API's production deploy, which goes through staging" 1 $st "api: REFUSED"
OA_ROOM_VM_STANDIN="$vm" "$here/../api/deploy.sh" --lock "$token" --staging > "$w/out" 2>&1; st=$?
expect "the release's own API deploy passes with its token" 0 $st "staging's lock allows this deploy"
v lock take "another release"; st=$?
expect "a second release can't take the lock" 1 $st "REFUSED" "release-set abc.1 by tester"
v lock status; st=$?
expect "lock status names the holder" 0 $st "held by release-set abc.1 by tester"
v lock clear; st=$?
expect "a live lock is not cleared" 1 $st "The lock is live"
v lock release 000000000000000000000000; st=$?
expect "another token does not release it" 1 $st "does not hold staging"
name=r6; v deploy --staging --lock "$token"; st=$?
expect "the release's own staging deploy passes with its token" 0 $st "staging runs r6"
v rollback --staging --lock "$token"; st=$?
expect "and its own rollback of staging" 0 $st "staging runs r5 again"
v lock check --lock "$token"; st=$?
expect "lock check passes for the holder" 0 $st
v lock check; st=$?
expect "and fails for anyone else" 1 $st "REFUSED"
v lock release "$token"; st=$?
expect "the holder releases it" 0 $st "released"
v lock status; st=$?
expect "staging is free again" 0 $st "staging lock: free"
# the nightly soak holds staging too, and gives way to a release (#13384)
OA_STAGING_LOCK_PID=$holder v lock take "soak 8da7b1d5e7" 600; st=$?
soak_token="$(sed -n 's/^lock-token //p' "$w/out")"
[[ $st == 0 && ${#soak_token} == 24 ]] && ok "the soak takes staging's lock" || { bad "the soak did not take the lock"; show; }
v lock mine "$soak_token"; st=$?
expect "the soak's token holds staging" 0 $st "staging lock: yours"
OA_STAGING_LOCK_PID=$holder v lock take "soak 0123456789" 600; st=$?
expect "a second soak can't take it" 1 $st "REFUSED" "soak 8da7b1d5e7"
v lock status; st=$?
expect "lock status says the soak gives way" 0 $st "held by soak 8da7b1d5e7" "gives way"
OA_STAGING_LOCK_PID=$holder v lock take "release-set abc.2 by tester"; st=$?
token="$(sed -n 's/^lock-token //p' "$w/out")"
expect "a release takes staging from the soak" 0 $st "gives way" "lock-token"
v lock mine "$soak_token"; st=$?
expect "the soak sees its lock is gone" 1 $st "not yours"
OA_STAGING_LOCK_PID=$holder v lock take "soak 8da7b1d5e7" 600; st=$?
expect "and a soak does not start while the release holds staging" 1 $st "REFUSED" "release-set abc.2 by tester"
v lock release "$token"; st=$?
expect "the release lets staging go" 0 $st "released"
OA_STAGING_LOCK_PID=$holder v lock take "soak 8da7b1d5e7" 600; st=$?
soak_token="$(sed -n 's/^lock-token //p' "$w/out")"
stage r7; st=$?
expect "a deploy to staging with no lock takes it from the soak too" 0 $st "gives way"
v lock mine "$soak_token"; st=$?
expect "and the soak sees that" 1 $st "not yours"
v lock status; st=$?
expect "staging is free after it" 0 $st "staging lock: free"
OA_ROOM_VM_STANDIN="$vm" OA_STAGING_LOCK="" "$here/../api/deploy.sh" --staging > "$w/out" 2>&1; st=$?
expect "the API's deploy passes when staging is free" 0 $st "staging's lock allows this deploy"

# ---- 7. a stale lock
OA_STAGING_LOCK_PID=$holder v lock take "release-set dead.2"; st=$?
[[ $st == 0 ]] || { bad "the second lock was not taken"; show; }
kill "$holder" 2> /dev/null; wait "$holder" 2> /dev/null
v lock status; st=$?
expect "a lock whose process is gone is said to be stale" 0 $st "STALE" "release-set dead.2" "lock clear"
v lock clear; st=$?
expect "lock clear removes a stale lock" 0 $st "stale lock" "is removed"
[[ ! -e "$o/staging.lock" ]] && ok "the lock is gone" || bad "the stale lock is still there"
echo "aaaaaaaaaaaaaaaaaaaaaaaa $(( $(date +%s) - 5 )) elsewhere 1 release-set old.3" > "$o/staging.lock"
v lock status; st=$?
expect "a lock past its time is stale, whatever computer took it" 0 $st "STALE" "release-set old.3"
stage r7; st=$?
expect "a stale lock does not stop a deploy, and is removed" 0 $st "was stale and is removed" "staging runs r7"
echo "bbbbbbbbbbbbbbbbbbbbbbbb $(( $(date +%s) + 600 )) elsewhere 1 release-set live.4" > "$o/staging.lock"
v lock clear; st=$?
expect "a lock in its time from another computer is live: not cleared" 1 $st "The lock is live" "release-set live.4"
stage r8; st=$?
expect "and it stops a deploy" 1 $st "release-set live.4"
rm -f "$o/staging.lock"

# ---- 8. the rollback target after a re-promote
# production runs r5 (good), previous r3. A wrong build goes out, and is repaired.
stage rW > /dev/null; v deploy --release rW; st=$?
expect "the wrong build is promoted (by its name)" 0 $st "the rollback target is r5"
stage rR > /dev/null; v deploy --release rR --repair; st=$?
expect "the repair keeps the rollback target" 0 $st "the rollback target stays r5" "rW was a mistake"
[[ "$(link current)" == rR && "$(link previous)" == r5 ]] && ok "production runs rR; a rollback goes to r5, not to the wrong build" || bad "current $(link current), previous $(link previous)"
v rollback; st=$?
expect "a rollback goes to r5" 0 $st "production's release is r5"
grep -qx rR "$o/bad" && ok "the release the rollback left is marked bad" || bad "rR is not marked bad"
v rollback; st=$?
expect "a second rollback does not go back to the bad release" 1 $st "REFUSED" "marked bad"
[[ "$(link current)" == r5 ]] && ok "production still runs r5" || bad "production went to $(link current)"
v previous rR; st=$?
expect "previous refuses a release marked bad" 1 $st "marked bad"
v previous r5; st=$?
expect "previous refuses what production runs" 1 $st "what production runs"
v previous nope; st=$?
expect "previous refuses a release that is not there" 1 $st "No release nope"
v previous r4-seeded; st=$?
expect "previous refuses a seeded build" 1 $st "seeded"
v previous r3; st=$?
expect "previous sets the target by name" 0 $st "the rollback target is r3"
[[ "$(link previous)" == r3 ]] && ok "previous is r3" || bad "previous is $(link previous)"
# a release marked bad by hand is never made the target by the promotion that replaces it
stage rB > /dev/null; v deploy --release rB > /dev/null; v bad rB
stage rC > /dev/null; v deploy --release rC; st=$?
expect "a promotion over a release marked bad keeps the target" 0 $st "the rollback target stays r5" "rB is marked bad"
[[ "$(link previous)" == r5 ]] && ok "previous is r5" || bad "previous is $(link previous)"
# promoting the target itself leaves none, and says so
stage rD > /dev/null; v deploy --release rD > /dev/null   # previous rC
v previous r5 > /dev/null
ln -sfn "$o/releases/r5" "$o/staging"
v deploy --release r5 --repair; st=$?
expect "a repair onto the target itself says there is no target left" 0 $st "NO rollback target"
[[ -z "$(link previous)" ]] && ok "previous is unset" || bad "previous is $(link previous)"
v releases; st=$?
expect "releases lists the links and the bad ones" 0 $st "current: r5" "bad: rR rB"

echo
if (( fails )); then echo "test-vm-deploy: $fails case(s) FAILED"; exit 1; fi
echo "test-vm-deploy: every case passed"
