#!/usr/bin/env bash
# Firm reason: 5. The check of after.sh's comparing gates (#13266) in a scratch repository: no host,
# no network, no GitHub (OA_PIPELINE_ISSUES=0) and a stand-in for check.sh (OA_AFTER_CHECK) that
# plays Everglade II's comparison and a flaky suite from marker files in the commit under check.
#   coax/remote/test-after.sh     exit 0 when every case passes
# The cases, each a push to the scratch main and one after.sh --once:
#   1. an ordinary commit: green, main-green moves
#   2. a commit that raises tools-port: red, the red list names that commit and "count tools-port"
#   3. a commit that takes the line out again: green, the entry is fixed
#   4. a commit that raises it with a "Rebaseline: tools-port: why" trailer: green
#   5. a commit worse than production in Everglade II (e2-regress), under two later ones: red, the
#      bisect names it and "e2 tick-p95"; its fix: green
#   6. main-green as bad as the head against production: reported, not red; green
#   7. a suite that fails once and passes when run again: flaky, the entry has an owner, and
#      `after.sh flaky` fails once the entry is older than 7 days
# And the host faults of #13256, none of which may blame a commit:
#   8. the toolchain moved under a host (the check can't judge: 97) until the slot's compiler is
#      installed again: green, main-green moves, nothing on the red list
#   9. a job that fails with empty output (a full host) once: green, nothing on the red list
#  10. a host that can never run the check (the pool is off: the cap): unfinished, main-green stays, nothing on the red list, and the alarm names the cause;
#      once it can, green
#  11. a check that fails on one host and passes on another (#13327): not the commit's; nothing filed
#  12. a full run that finds a culprit the fast runs passed: main-green goes back to just before
#      the culprit, not to the last full green run
# And the service watch (#13379):
#  13. a production crash alert's bucket gets one GitHub issue: none while GitHub can't be asked,
#      none for staging, no second when the watcher's memory is gone, a new one after its alert
#      opens again with the issue closed
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
w="$(mktemp -d "${TMPDIR:-/tmp}/oa-after-test.XXXXXXXX")" || exit 2
trap 'rm -rf "${w:?}"' EXIT
fails=0
ok() { echo "ok: $*"; }
bad() { echo "FAIL: $*"; fails=$((fails + 1)); }
g() { git -C "$w/repo" -c user.name=t -c user.email=t@t "$@"; }

git init -q --bare "$w/origin.git"
mkdir -p "$w/repo/coax/remote" "$w/repo/coax/web" "$w/repo/everglade" "$w/repo/proto" "$w/repo/metaverse-engine" "$w/repo/lib"
cp "$here/after.sh" "$here/pipeline-lib.sh" "$here/counts.sh" "$w/repo/coax/remote/"
printf '# list\na\nb\nc\n' > "$w/repo/coax/tools-port.txt"
printf '# list\nshape x\n' > "$w/repo/proto/wire-allowlist.txt"
printf '# layers\nengine-up a -> b\ntests c -> d\n' > "$w/repo/metaverse-engine/layer-allowlist.txt"
printf 'EF_KNOWN :: [2] string.[\n  "a",\n  "b"\n];\nEF_GAPS :: [1] string.[\n  "g"\n];\n' > "$w/repo/everglade/features.coax"
printf 'CC_TO_MOVE :: [1] CcMove.[\n  CcMove.{path = "x"}\n];\n' > "$w/repo/coax/web/components-check.coax"
echo 'one :: 1;' > "$w/repo/lib/a.coax"
(cd "$w/repo" && git init -q -b main && git remote add origin "$w/origin.git") || exit 2
g add -A && g commit -qm "base

Agent: test-base" && g push -q origin main && g push -q origin main:refs/heads/main-green || exit 2

# The stand-in check: CMD COMMIT OUTDIR CHECK-ARGS. A failure ends as check.sh's does ("check
# ...: FAILED"). Everglade II (room-since/tools with production, e2) fails "e2 tick-p95" when the
# commit has e2-regress; a suite part fails once when the commit has flaky-once. The host faults
# (#13256): toolchain-moved is not judged (97) until after.sh asks for the compiler again
# (OA_REMOTE_FRESH_COMPILER=1); host-full fails with no output until then; host-broken is never
# judged; full-break fails the full run's suite parts.
cat > "$w/check" <<'EOF'
#!/usr/bin/env bash
commit="$1" dir="$2"; shift 2
mkdir -p "$dir/build/pipeline"; : > "$dir/build/pipeline/failed"
has() { git cat-file -e "$commit:$1" 2> /dev/null; }
fail() { echo "$2" > "$dir/build/pipeline/failed"; echo "failed:"; echo "  $2"; echo "check $1: FAILED"; exit 1; }
if [[ "${OA_REMOTE_FRESH_COMPILER:-0}" != 1 ]]; then
  has toolchain-moved && { echo "check $1: not judged (97): the host's compiler doesn't match this tree"; exit 97; }
  has host-full && exit 1
fi
has host-broken && { echo "remote: the pool is off: the cap: the hosts cost about \$40.00 over 24 hours and OA_REMOTE_DAILY_USD is \$39.60. Fix: coax/remote/remote.sh status"; exit 97; }
# one-bad-host: the job fails on oa-bad (where it goes first), passes anywhere else
if has one-bad-host; then
  h=oa-bad; [[ "${OA_REMOTE_AVOID:-}" != oa-bad ]] || h=oa-good
  echo "remote: $h slot 1: checkout and compiler 1 s, command 1 s, status 0"
  [[ "$h" != oa-bad ]] || fail "$1" "compiler mutation_schedule"
fi
case "$1" in
  room-since|tools|e2)
    if { [[ "$1" == e2 ]] || [[ "$1" == tools && -n "${2:-}" ]] || [[ "$1" == room-since && -n "${3:-}" ]]; } && has e2-regress; then fail "$1" "e2 tick-p95"; fi ;;
  since)
    if has flaky-once && [[ ! -e "$OA_TEST_W/flaked" ]]; then : > "$OA_TEST_W/flaked"; fail "$1" "suite lib/a.coax"; fi ;;
  suites|only) has full-break && fail "$1" "suite lib/a.coax" ;;
esac
exit 0
EOF
chmod +x "$w/check"
export OA_PIPELINE_DIR="$w/pl" OA_PIPELINE_ISSUES=0 OA_AFTER_CHECK="$w/check" OA_AFTER_PAGES=0 OA_AFTER_GAP=99999999 OA_TEST_W="$w"
unset OA_PIPELINE_NOTIFY
mkdir -p "$w/pl/releases/after"; date +%s > "$w/pl/releases/after/last-full"
base="$(g rev-parse HEAD)"; export OA_AFTER_PRODUCTION="$base"
red="$w/pl/releases/after/red.json"
push() {   # agent subject [trailer]: commit what is staged and push it to the scratch main
  g add -A && g commit -qm "$2

${3:+$3
}Agent: $1" && g push -q origin main
}
run() { (cd "$w/repo" && coax/remote/after.sh --once) > "$w/run.log" 2>&1; tail -1 "$w/pl/releases/after/runs.tsv" | cut -f4; }
green_is() { [[ "$(git -C "$w/origin.git" rev-parse main-green)" == "$1" ]]; }
open_entry() { jq -r --arg c "$1" '.[] | select(.status == "open" and .check == $c) | .culprit' "$red"; }

# 1
echo 'two :: 2;' >> "$w/repo/lib/a.coax"; push agent-one "an ordinary change"; c1="$(g rev-parse HEAD)"
s="$(run)"; [[ "$s" == green ]] && green_is "$c1" && ok "1: an ordinary head is green and main-green moves" || { bad "1: status $s"; cat "$w/run.log"; }

# 2
echo d >> "$w/repo/coax/tools-port.txt"; push agent-count "a tool joins the port list"; c2="$(g rev-parse HEAD)"
echo 'three :: 3;' >> "$w/repo/lib/a.coax"; push agent-other "a later change"
s="$(run)"; e="$(open_entry "count tools-port")"
[[ "$s" == red && "$e" == "$c2" ]] && green_is "$c1" && ok "2: a raised count is red and names its commit (${c2:0:10})" || { bad "2: status $s, culprit ${e:-none}"; cat "$w/run.log"; }
(cd "$w/repo" && coax/remote/after.sh state) > "$w/state.txt" 2>&1
grep -q "tools-port.*RAISED" "$w/state.txt" && grep -q "raised: tools-port by ${c2:0:10}" "$w/state.txt" && ok "2: state shows the raised count" || { bad "2: state"; cat "$w/state.txt"; }

# 3
sed -i.bak '/^d$/d' "$w/repo/coax/tools-port.txt"; rm -f "$w/repo/coax/tools-port.txt.bak"; push agent-count "the tool is Coax now"; c3="$(g rev-parse HEAD)"
s="$(run)"; [[ "$s" == green && -z "$(open_entry "count tools-port")" ]] && green_is "$c3" && ok "3: the count back down is green and the entry is fixed" || { bad "3: status $s"; cat "$w/run.log"; }

# 4
printf 'e\nf\n' >> "$w/repo/coax/tools-port.txt"
push agent-wide "tools-check covers a new folder" "Rebaseline: tools-port: tools-check now lists the scripts under lib/ it did not read before"; c4="$(g rev-parse HEAD)"
s="$(run)"; [[ "$s" == green ]] && green_is "$c4" && ok "4: a raise with a Rebaseline trailer is green" || { bad "4: status $s"; cat "$w/run.log"; }

# 5
echo 'four :: 4;' >> "$w/repo/lib/a.coax"; push agent-x "before"
touch "$w/repo/e2-regress"; push agent-slow "a slower room"; c5="$(g rev-parse HEAD)"
echo 'five :: 5;' >> "$w/repo/lib/a.coax"; push agent-y "after one"
echo 'six :: 6;' >> "$w/repo/lib/a.coax"; push agent-z "after two"
s="$(run)"; e="$(open_entry "e2 tick-p95")"
[[ "$s" == red && "$e" == "$c5" ]] && green_is "$c4" && ok "5: worse than production is red and the bisect names its commit (${c5:0:10})" || { bad "5: status $s, culprit ${e:-none}"; cat "$w/run.log"; }
rm -f "$w/repo/e2-regress"; push agent-slow "the room is quick again"; c5b="$(g rev-parse HEAD)"
s="$(run)"; [[ "$s" == green && -z "$(open_entry "e2 tick-p95")" ]] && green_is "$c5b" && ok "5: its fix is green and the entry is fixed" || { bad "5 fix: status $s"; cat "$w/run.log"; }

# 6
touch "$w/repo/e2-regress"; push agent-slow "slow again"; c6="$(g rev-parse HEAD)"
g push -q -f origin "$c6:refs/heads/main-green"
echo 'seven :: 7;' >> "$w/repo/lib/a.coax"; push agent-y "on top"; c6b="$(g rev-parse HEAD)"
s="$(run)"
[[ "$s" == green && -z "$(open_entry "e2 tick-p95")" ]] && green_is "$c6b" && grep -q "reported, not red" "$w/run.log" && ok "6: as bad as main-green against production: reported, not red" || { bad "6: status $s"; cat "$w/run.log"; }
rm -f "$w/repo/e2-regress"

# 7
touch "$w/repo/flaky-once"; echo 'eight :: 8;' >> "$w/repo/lib/a.coax"; push agent-flake "a change under a flaky suite"; c7="$(g rev-parse HEAD)"
s="$(run)"; f="$w/pl/releases/after/flaky.tsv"
[[ "$s" == green && "$(cut -f1 "$f" 2> /dev/null)" == "suite lib/a.coax" && "$(cut -f6 "$f")" == agent-flake ]] && ok "7: a flaky suite is green with an owned entry (owner $(cut -f6 "$f"))" || { bad "7: status $s, entry $(cat "$f" 2> /dev/null)"; cat "$w/run.log"; }
(cd "$w/repo" && coax/remote/after.sh flaky) > /dev/null 2>&1 && ok "7: a new entry is not a failure" || bad "7: a new entry fails"
awk -F'\t' -v OFS='\t' '{ $3 = $3 - 8 * 86400 } { print }' "$f" > "$f.new" && mv "$f.new" "$f"
out="$( (cd "$w/repo" && coax/remote/after.sh flaky) 2>&1)"; st=$?
(( st == 1 )) && grep -q FAILING <<< "$out" && ok "7: an entry over 7 days old is a failure" || { bad "7: aged entry: status $st"; echo "$out"; }

rm -f "$w/repo/flaky-once"
open_count() { jq '[.[] | select(.status == "open")] | length' "$red"; }
before="$(open_count)"

# 8
touch "$w/repo/toolchain-moved"; echo 'nine :: 9;' >> "$w/repo/lib/a.coax"; push agent-tc "a change while nixpkgs moved"; c8="$(g rev-parse HEAD)"
s="$(run)"
[[ "$s" == green && "$(open_count)" == "$before" ]] && green_is "$c8" && grep -q "not judged (97)" "$w/pl/releases/after/$c8"/*.try1.out &&
  ok "8: a moved toolchain is not the commit's: run again with the compiler installed again, green, nothing filed" || { bad "8: status $s, open $(open_count)"; cat "$w/run.log"; }
rm -f "$w/repo/toolchain-moved"

# 9
touch "$w/repo/host-full"; echo 'ten :: 10;' >> "$w/repo/lib/a.coax"; push agent-full "a change while the hosts were full"; c9="$(g rev-parse HEAD)"
s="$(run)"
[[ "$s" == green && "$(open_count)" == "$before" ]] && green_is "$c9" && grep -q "no verdict from the check" "$w/pl/releases/after/$c9"/*.try1.out &&
  ok "9: a failure with empty output is not judged: green, nothing filed" || { bad "9: status $s, open $(open_count)"; cat "$w/run.log"; }
rm -f "$w/repo/host-full"

# 10
touch "$w/repo/host-broken"; echo 'eleven :: 11;' >> "$w/repo/lib/a.coax"; push agent-hb "a change no host can check"; c10="$(g rev-parse HEAD)"
s="$(OA_AFTER_UNFINISHED=1 run)"
[[ "$s" == unfinished && "$(open_count)" == "$before" ]] && green_is "$c9" && ok "10: a host that can't run the check leaves the head unjudged; main-green stays; nothing filed" || { bad "10: status $s, open $(open_count)"; cat "$w/run.log"; }
grep -q "the pool is off: the cap: no check of ${c10:0:10} could run" "$w/pl/ALARM" && ok "10: the alarm names the cause: the pool is off: the cap" || { bad "10: the alarm: $(cat "$w/pl/ALARM" 2> /dev/null)"; }
rm -f "$w/repo/host-broken"; push agent-hb "the hosts are back"; c10b="$(g rev-parse HEAD)"
s="$(run)"; [[ "$s" == green ]] && green_is "$c10b" && ok "10: once a host can, green" || { bad "10 after: status $s"; cat "$w/run.log"; }

# 11
touch "$w/repo/one-bad-host"; echo 'eleven2 :: 11;' >> "$w/repo/lib/a.coax"; push agent-bh "a change checked on a bad host"; c12="$(g rev-parse HEAD)"
s="$(run)"
[[ "$s" == green && "$(open_count)" == "$before" ]] && green_is "$c12" && grep -q "remote: oa-good" "$w/pl/releases/after/$c12"/bisect-0/c1-*/out &&
  ok "11: a failure on one host is run again on another, passes there, and nothing is filed" || { bad "11: status $s, open $(open_count)"; cat "$w/run.log"; }
rm -f "$w/repo/one-bad-host"; push agent-bh "off the bad host"; c10b="$(g rev-parse HEAD)"; s="$(run)"

# 12
echo "$c10b" > "$w/pl/releases/after/full-green"
echo 'twelve :: 12;' >> "$w/repo/lib/a.coax"; push agent-a "good before the break"; c11a="$(g rev-parse HEAD)"
touch "$w/repo/full-break"; push agent-break "breaks what only the full run checks"; c11="$(g rev-parse HEAD)"
echo 'thirteen :: 13;' >> "$w/repo/lib/a.coax"; push agent-b "on top of the break"; c11b="$(g rev-parse HEAD)"
s="$(run)"; green_is "$c11b" || { bad "12: the fast run did not pass: $s"; cat "$w/run.log"; }
rm -f "$w/pl/releases/after/last-full"
s="$(run)"; e="$(open_entry "suite lib/a.coax")"
[[ "$s" == red && "$e" == "$c11" ]] && green_is "$c11a" && ok "12: a full run's culprit sets main-green back to just before it (${c11a:0:10}), not to the last full green run (${c10b:0:10})" || { bad "12: status $s, culprit ${e:-none}, main-green $(git -C "$w/origin.git" rev-parse main-green | cut -c1-10)"; cat "$w/run.log"; }

# 13: a production crash alert's bucket becomes one GitHub issue (#13379), with stand-ins for curl
# (the service health and the crashes) and gh (its calls logged; an issue it made is open after).
mkdir -p "$w/fake" "$w/pl13"
cat > "$w/fake/curl" <<'EOF'
#!/usr/bin/env bash
for a in "$@"; do url="$a"; done
case "$url" in
  https://openagents.com/*service-health) cat "$OA_TEST_W/health" ;;
  https://openagents.com/*crashes) echo '{"env":"production","now_ms":5000000,"buckets":[{"bucket":"a2dd7180bb5f3e4a78467bd7b64950aa","source":"room","sig":"SIGABRT|rm_x|main","last_ms":4999000,"reports":1,"newest":"crash 1\nsig SIGABRT|rm_x|main\n"},{"bucket":"01163983a18bf7ff2acacbf70025a95a","source":"page","sig":"trap|frame","last_ms":4999000,"reports":2,"newest":"crash 1\n"}]}' ;;
  *service-health) echo '{"alerts":[{"key":"crash:page","title":"a staging page crash"}]}' ;;
  *) exit 22 ;;
esac
EOF
cat > "$w/fake/gh" <<'EOF'
#!/usr/bin/env bash
echo "$*" | head -1 >> "$OA_TEST_W/gh.log"
[[ ! -e "$OA_TEST_W/gh-down" ]] || exit 1
case "$1 $2" in
  "issue list") tag="$(printf '%s\n' "$@" | grep -o 'bucket [0-9a-f]*' | head -1)"; grep -F -- "$tag" "$OA_TEST_W/gh-open" 2> /dev/null | cut -f1 | head -1 ;;
  "issue create") n=$((900 + $(grep -c '^issue create' "$OA_TEST_W/gh.log")))
    while [[ $# -gt 0 ]]; do [[ "$1" != --title ]] || t="$2"; shift; done
    printf '%s\t%s\n' "$n" "$t" >> "$OA_TEST_W/gh-open"; echo "https://github.com/x/y/issues/$n" ;;
esac
exit 0
EOF
chmod +x "$w/fake/curl" "$w/fake/gh"
watch13() { (export PATH="$w/fake:$PATH" OA_PIPELINE_DIR="$w/pl13" OA_SERVICE_ORIGINS="https://openagents.com https://staging.openagents.com"
  source "$here/pipeline-lib.sh"; pl_issues=1; pl_service_watch) > "$w/watch13.log" 2>&1; }
made13() { grep -c '^issue create' "$w/gh.log" 2> /dev/null || true; }
echo '{"alerts":[{"key":"crash:a2dd7180","title":"a room crash"}]}' > "$w/health"
touch "$w/gh-down"; watch13; rm -f "$w/gh-down"
[[ "$(made13)" == 0 && ! -s "$w/pl13/crash-issues" ]] && ok "13: with GitHub unreachable nothing is filed or remembered" || { bad "13: filed with GitHub down"; cat "$w/watch13.log"; }
watch13; watch13
[[ "$(made13)" == 1 ]] && grep -q "bucket a2dd7180bb5f" "$w/gh-open" && grep -q "filed as #901" "$w/pl13/inbox/foreman.log" &&
  ok "13: a production room bucket under its open alert gets one issue; the page bucket (no page alert) and staging get none" || { bad "13: $(made13) issue(s) made"; cat "$w/watch13.log" "$w/gh.log"; }
rm -f "$w/pl13/crash-issues"; watch13
[[ "$(made13)" == 1 ]] && ok "13: a watcher with no memory finds the open issue and files no second" || { bad "13: a second issue for one bucket"; cat "$w/gh.log"; }
echo '{"alerts":[{"key":"crash:a2dd7180","title":"a room crash"},{"key":"crash:page","title":"a page crash"}]}' > "$w/health"; watch13; watch13
[[ "$(made13)" == 2 ]] && grep -q "bucket 01163983a18b" "$w/gh-open" && ok "13: the page alert files the page bucket's issue once" || { bad "13: $(made13) issue(s) after the page alert"; cat "$w/gh.log"; }
echo '{"alerts":[]}' > "$w/health"; watch13; : > "$w/gh-open"
echo '{"alerts":[{"key":"crash:a2dd7180","title":"a room crash"}]}' > "$w/health"; watch13; watch13
[[ "$(made13)" == 3 ]] && ok "13: a bucket whose issue was closed gets a new one when its alert opens again" || { bad "13: $(made13) issue(s) after the alert opened again"; cat "$w/gh.log"; }

(( fails == 0 )) && echo "test-after: every case passed" || echo "test-after: $fails case(s) failed"
exit $(( fails > 0 ))
