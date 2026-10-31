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

# Not survivors only: a report with an unreached procedure or no summary is refused.
printf 'lib/weak.coax:1:36: mutant survived: x\nnot reached by any check: twice\n3 mutants, 2 killed, 1 survived\n' > "$work/other"
"$gate" survivors < "$work/other" > /dev/null && fail "an unreached procedure was read as survivors only"
printf 'coax: the checks fail before any mutation; make them pass first\n' > "$work/other"
"$gate" survivors < "$work/other" > /dev/null && fail "a failing baseline was read as survivors only"
"$gate" verdict "$work/changed" /dev/null || fail "no survivors gated"
echo "mutation-gate: survivors gate only on changed lines; strict gates on all"
