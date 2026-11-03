#!/usr/bin/env bash
# Firm reason: 4. It checks coax/mutation-gate.sh, part of the bootstrap.
#
# A surviving mutant stops a build only when it lies on a line changed since
# the base commit; the full check (strict) stops on every survivor; a report
# that failed for another reason is never read as survivors only.
#   coax/tests/mutation-gate.sh        (needs a built coax/bin/coax)
set -euo pipefail
coax="$(cd "$(dirname "$0")/.." && pwd -P)"
gate="$coax/mutation-gate.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-mutation-gate.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "mutation-gate test: $*" >&2; exit 1; }
repo="$work/repo"
mkdir -p "$repo/coax/lib"
git -C "$repo" init -q
git -C "$repo" config user.email test@example.invalid
git -C "$repo" config user.name test
# Line 1's '+' survives: the only check adds zeros.
cat > "$repo/coax/lib/weak.coax" << 'EOF'
add :: (a:s32,b:s32)->s32 {return a+b;}
twice :: (a:s32)->s32 {return a*2;}
#test "zero" {#assert add(0,0)==0;#assert twice(3)==6;}
main :: () {}
EOF
git -C "$repo" add -A && git -C "$repo" commit -q -m base
base="$(git -C "$repo" rev-parse HEAD)"
report() { (cd "$repo/coax" && "$coax/bin/coax" mutate lib/weak.coax > "$work/report" 2>&1) && fail "expected a survivor"; "$gate" survivors < "$work/report" > "$work/survivors"; }
report
[[ "$(cat "$work/survivors")" == "lib/weak.coax:1" ]] || fail "survivors: $(cat "$work/survivors")"

# A change elsewhere (a new line 5) leaves the line-1 survivor unchanged: it is reported, not gating.
echo '// a note' >> "$repo/coax/lib/weak.coax"
report
"$gate" changed "$repo" "$base" > "$work/changed"
grep -qx 'lib/weak.coax:5' "$work/changed" || fail "line 5 is not listed as changed"
"$gate" verdict "$work/changed" "$work/survivors" > /dev/null || fail "a survivor on an unchanged line gated"
"$gate" verdict strict "$work/survivors" > /dev/null && fail "the full check let a survivor through"

# The survivor's own line changed: it gates and is named.
git -C "$repo" checkout -q -- coax/lib/weak.coax
sed -i.bak 's/return a+b;/return b+a;/' "$repo/coax/lib/weak.coax"
report
"$gate" changed "$repo" "$base" > "$work/changed"
named="$("$gate" verdict "$work/changed" "$work/survivors")" && fail "a survivor on a changed line passed"
[[ "$named" == "lib/weak.coax:1" ]] || fail "verdict named: $named"

# Between two commits, as release.sh asks.
git -C "$repo" commit -q -am change
"$gate" changed "$repo" "$base" HEAD | grep -qx 'lib/weak.coax:1' || fail "commit range: line 1 is not listed"

# A check removed elsewhere in the file: the mutant it killed now survives on
# an unchanged line. The change caused that survivor, so it gates.
printf '%s\n' 'add :: (a:s32,b:s32)->s32 {return a+b;}' '#test "zero" {#assert add(0,0)==0;}' \
  '#test "one" {#assert add(1,2)==3;}' 'main :: () {}' > "$repo/coax/lib/strong.coax"
git -C "$repo" add coax/lib/strong.coax && git -C "$repo" commit -q -m strong
strong="$(git -C "$repo" rev-parse HEAD)"
mutate_strong() { (cd "$repo/coax" && "$coax/bin/coax" mutate lib/strong.coax > "$work/report" 2>&1); }
mutate_strong || fail "the strong fixture has a survivor"
sed -i.bak '3d' "$repo/coax/lib/strong.coax"
mutate_strong && fail "expected a survivor once the check went"
"$gate" survivors < "$work/report" > "$work/survivors"
[[ "$(cat "$work/survivors")" == "lib/strong.coax:1" ]] || fail "survivors: $(cat "$work/survivors")"
"$gate" changed "$repo" "$strong" > "$work/changed"
grep -qx 'lib/strong.coax:1' "$work/changed" && fail "line 1 is listed as changed"
grep -qx 'lib/strong.coax:checks' "$work/changed" || fail "the removed check is not listed"
named="$("$gate" verdict "$work/changed" "$work/survivors")" && fail "a survivor of a removed check passed"
[[ "$named" == "lib/strong.coax:1" ]] || fail "verdict named: $named"
# An edited (weakened) check counts as removed; one only added does not.
git -C "$repo" checkout -q -- coax/lib/strong.coax
sed -i.bak 's/add(1,2)==3/add(1,2)>=0/' "$repo/coax/lib/strong.coax"
"$gate" changed "$repo" "$strong" | grep -qx 'lib/strong.coax:checks' || fail "an edited check is not listed"
git -C "$repo" checkout -q -- coax/lib/strong.coax
echo '#test "two" {#assert add(2,2)==4;}' >> "$repo/coax/lib/strong.coax"
"$gate" changed "$repo" "$strong" | grep -qx 'lib/strong.coax:checks' && fail "an added check is listed as removed"
git -C "$repo" checkout -q -- coax/lib/strong.coax
# The gate's mutated file lost a check and the survivor is in a file it
# imports: the labelled survivor gates. Another gate's survivor does not.
echo 'lib/strong.coax:checks' > "$work/changed"
echo 'stage0 mutate lib/strong.coax --root-only: lib/other.coax:7' > "$work/labelled"
named="$("$gate" verdict "$work/changed" "$work/labelled")" && fail "a survivor of the gate's removed check passed"
[[ "$named" == "lib/other.coax:7" ]] || fail "verdict named: $named"
echo 'stage0 mutate lib/third.coax: lib/other.coax:7' > "$work/labelled"
"$gate" verdict "$work/changed" "$work/labelled" > /dev/null || fail "a survivor of an untouched gate gated"

# A root with parts (lib/split.coax loading lib/split/*.coax, as the checker
# does): a check removed from one part marks the root, so the root gate's
# survivors in every part gate. A part with no root beside its directory does not.
mkdir -p "$repo/coax/lib/split"
printf '%s\n' '#load "split/code.coax";' '#load "split/checks.coax";' > "$repo/coax/lib/split.coax"
printf '%s\n' 'add :: (a:s32,b:s32)->s32 {return a+b;}' 'main :: () {}' > "$repo/coax/lib/split/code.coax"
printf '%s\n' '#test "one" {#assert add(1,2)==3;}' '#test "two" {#assert add(2,2)==4;}' > "$repo/coax/lib/split/checks.coax"
git -C "$repo" add coax/lib && git -C "$repo" commit -q -m split
split="$(git -C "$repo" rev-parse HEAD)"
sed -i.bak '2d' "$repo/coax/lib/split/checks.coax"
rm "$repo/coax/lib/split/checks.coax.bak"
"$gate" changed "$repo" "$split" > "$work/changed"
grep -qx 'lib/split/checks.coax:checks' "$work/changed" || fail "the part's removed check is not listed"
grep -qx 'lib/split.coax:checks' "$work/changed" || fail "the part's removed check does not mark its root"
echo 'stage0 mutate lib/split.coax: lib/split/code.coax:1' > "$work/labelled"
"$gate" verdict "$work/changed" "$work/labelled" > /dev/null && fail "a survivor of a part's removed check passed"
grep -qx 'lib.coax:checks' "$work/changed" && fail "a directory with no root file is marked"
git -C "$repo" checkout -q -- coax/lib/split/checks.coax

# A file the working tree adds without committing is changed throughout, and
# a user's diff.noprefix does not hide the changed paths.
printf '%s\n' 'x :: ()->s32 {return 1;}' 'main :: () {}' > "$repo/coax/lib/new.coax"
"$gate" changed "$repo" "$strong" | grep -qx 'lib/new.coax:2' || fail "an untracked file is not listed"
rm "$repo/coax/lib/new.coax"
echo '// a note' >> "$repo/coax/lib/strong.coax"
git -C "$repo" config diff.noprefix true
"$gate" changed "$repo" "$strong" | grep -qx 'lib/strong.coax:5' || fail "diff.noprefix hid the changed lines"
git -C "$repo" config --unset diff.noprefix
"$gate" changed "$repo" "$base" > "$work/changed"

# Not survivors only: a report with an unreached procedure or no summary is refused.
printf 'lib/weak.coax:1:36: mutant survived: x\nnot reached by any check: twice\n3 mutants, 2 killed, 1 survived\n' > "$work/other"
"$gate" survivors < "$work/other" > /dev/null && fail "an unreached procedure was read as survivors only"
printf 'coax: the checks fail before any mutation; make them pass first\n' > "$work/other"
"$gate" survivors < "$work/other" > /dev/null && fail "a failing baseline was read as survivors only"
"$gate" verdict "$work/changed" /dev/null || fail "no survivors gated"
echo "mutation-gate: survivors gate on changed lines and removed checks only; strict gates on all"
