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
mkdir -p "$w/repo/coax/remote" "$w/repo/coax/web" "$w/repo/everglade" "$w/repo/proto" "$w/repo/aethyr" "$w/repo/lib"
cp "$here/after.sh" "$here/pipeline-lib.sh" "$here/counts.sh" "$w/repo/coax/remote/"
printf '# list\na\nb\nc\n' > "$w/repo/coax/tools-port.txt"
printf '# list\nshape x\n' > "$w/repo/proto/wire-allowlist.txt"
printf '# layers\naethyr-up a -> b\ntests c -> d\n' > "$w/repo/aethyr/layer-allowlist.txt"
printf 'EF_KNOWN :: [2] string.[\n  "a",\n  "b"\n];\nEF_GAPS :: [1] string.[\n  "g"\n];\n' > "$w/repo/everglade/features.coax"
printf 'CC_TO_MOVE :: [1] CcMove.[\n  CcMove.{path = "x"}\n];\n' > "$w/repo/coax/web/components-check.coax"
echo 'one :: 1;' > "$w/repo/lib/a.coax"
(cd "$w/repo" && git init -q -b main && git remote add origin "$w/origin.git") || exit 2
g add -A && g commit -qm "base

Agent: test-base" && g push -q origin main && g push -q origin main:refs/heads/main-green || exit 2

# The stand-in check: CMD COMMIT OUTDIR CHECK-ARGS. Everglade II (room-since/tools with production,
# e2) fails "e2 tick-p95" when the commit has e2-regress; a suite part fails once when the commit has
# flaky-once.
cat > "$w/check" <<'EOF'
#!/usr/bin/env bash
commit="$1" dir="$2"; shift 2
mkdir -p "$dir/build/pipeline"; : > "$dir/build/pipeline/failed"
has() { git cat-file -e "$commit:$1" 2> /dev/null; }
case "$1" in
  room-since|tools|e2)
    if { [[ "$1" == e2 ]] || [[ "$1" == tools && -n "${2:-}" ]] || [[ "$1" == room-since && -n "${3:-}" ]]; } && has e2-regress; then echo "e2 tick-p95" > "$dir/build/pipeline/failed"; echo worse; exit 1; fi ;;
  since)
    if has flaky-once && [[ ! -e "$OA_TEST_W/flaked" ]]; then : > "$OA_TEST_W/flaked"; echo "suite lib/a.coax" > "$dir/build/pipeline/failed"; exit 1; fi ;;
  only) exit 0 ;;
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

(( fails == 0 )) && echo "test-after: every case passed" || echo "test-after: $fails case(s) failed"
exit $(( fails > 0 ))
