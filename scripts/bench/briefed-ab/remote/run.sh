#!/usr/bin/env bash
# The bench's build host side (#11211): run one command in a slot checkout
# at BASE with the trial's working copy applied, then save the slot's
# resulting change so the trial can pick up what the command wrote (fmt,
# a lockfile).
#
#   run.sh SLOT BASE RELCWD TIMEOUT_SECS -- CMD...   (stdin: the patch)
#
# Slots share one persistent target dir, ~/ab/target. Never touches
# ~/openagents/target.
set -u
slot=$1 base=$2 rel=$3 limit=$4
shift 5
dir=$HOME/ab/slot$slot
exec 9>"$HOME/ab/slot$slot.lock"
flock 9
patch=$(mktemp)
trap 'rm -f "$patch"' EXIT
cat > "$patch"
cd "$dir" || exit 98
git reset -q --hard "$base" 2>/dev/null || { git -C "$HOME/openagents" fetch -q origin && git reset -q --hard "$base"; } || exit 98
git clean -fdq
if [ -s "$patch" ]; then
  git apply --binary --whitespace=nowarn "$patch" || { echo "ab: the working copy did not apply on the build host"; exit 97; }
fi
cd "$dir/$rel" 2>/dev/null || cd "$dir"
args=()
for a in "$@"; do args+=("${a//@ROOT@/$dir}"); done
export CARGO_TARGET_DIR=$HOME/ab/target CARGO_TERM_COLOR=never CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
timeout -k 10 "$limit" "${args[@]}" 2>&1 | sed -u "s#$dir/##g; s#$dir#.#g"
rc=${PIPESTATUS[0]}
[ "$rc" = 124 ] && echo "ab: the command ran past ${limit}s and was stopped"
cd "$dir"
git add -A >/dev/null 2>&1
git diff --cached --binary "$base" > "$HOME/ab/slot$slot.result.patch"
git reset -q
exit "$rc"
