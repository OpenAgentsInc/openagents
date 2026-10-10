#!/usr/bin/env bash
# The bench's build host side (#11211): run one command in the build
# checkout at BASE with the trial's working copy applied, then save the
# resulting change (per trial slot) so the trial can pick up what the
# command wrote (fmt, a lockfile).
#
#   run.sh SLOT BASE RELCWD TIMEOUT_SECS -- CMD...   (stdin: the patch)
#
# See common.sh: one build checkout, one persistent target dir.
set -u
slot=$1 base=$2 rel=$3 limit=$4
shift 5
. "$(dirname "$0")/common.sh"
ab_lock
patch=$(mktemp)
trap 'rm -f "$patch"' EXIT
cat > "$patch"
ab_checkout "$base"
if [ -s "$patch" ]; then
  git apply --binary --whitespace=nowarn "$patch" || { echo "ab: the working copy did not apply on the build host"; exit 97; }
fi
cd "$dir/$rel" 2>/dev/null || cd "$dir"
args=()
for a in "$@"; do args+=("${a//@ROOT@/$dir}"); done
timeout -k 10 "$limit" "${args[@]}" 2>&1 | sed -u "s#$dir/##g; s#$dir#.#g"
rc=${PIPESTATUS[0]}
[ "$rc" = 124 ] && echo "ab: the command ran past ${limit}s and was stopped"
cd "$dir"
git add -A >/dev/null 2>&1
git diff --cached --binary "$base" > "$HOME/ab/slot$slot.result.patch"
git reset -q
exit "$rc"
