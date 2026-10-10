# Shared by run.sh and eval.sh (#11211). One build checkout, ~/ab/build,
# and one target dir, ~/ab/target, for every trial: builds take turns under
# ~/ab/build.lock, and trials of the same issue share the base commit, so a
# switch rebuilds only what the patches touch. The disk guard keeps the
# host above 50 GB free: below AB_MIN_FREE_GB the bench's own target dir is
# cleared. Nothing here touches ~/openagents/target.
AB_MIN_FREE_GB=${AB_MIN_FREE_GB:-58}
# AB_BUILD=NAME picks a second checkout and target (the grader's
# validation runs beside trials without thrashing their build).
sfx=${AB_BUILD:+-$AB_BUILD}
dir=$HOME/ab/build$sfx
export CARGO_TARGET_DIR=$HOME/ab/target$sfx CARGO_TERM_COLOR=never CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
ab_lock() {
  exec 9>"$HOME/ab/build$sfx.lock"
  flock 9
  [ -d "$dir" ] || git -C "$HOME/openagents" worktree add -q --detach "$dir" origin/main
  ab_guard
}
ab_guard() {
  local free
  free=$(df -BG --output=avail / | tail -1 | tr -dc 0-9)
  if [ "$free" -lt "$AB_MIN_FREE_GB" ]; then
    echo "ab: the build host had ${free} GB free; cleared the bench's target dir (cold build)" >&2
    rm -rf "$CARGO_TARGET_DIR"; mkdir -p "$CARGO_TARGET_DIR"
  fi
}
ab_checkout() {
  local base=$1
  cd "$dir" || exit 98
  git reset -q --hard "$base" 2>/dev/null || { git -C "$HOME/openagents" fetch -q origin && git reset -q --hard "$base"; } || exit 98
  git clean -fdq
}
