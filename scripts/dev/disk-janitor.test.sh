#!/usr/bin/env bash
# Tests scripts/dev/disk-janitor.sh in a fake HOME: what it removes and,
# above all, what it must keep.
# Run: scripts/dev/disk-janitor.test.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(cd "$(mktemp -d)" && pwd -P)
trap 'rm -rf "$tmp"' EXIT
H=$tmp/home
mkdir -p "$H/work" "$H/.openagents/scratch" "$H/.openagents/exports"
export JANITOR_HOME=$H JANITOR_FREE_GB=500 JANITOR_LSOF_FILE=$tmp/lsof
: >"$tmp/lsof"
old() { touch -t 202001010000 "$@"; }
age_tree() { find "$1" -exec touch -h -t 202001010000 {} +; }

fail=0
expect_gone() { [ ! -e "$1" ] || { echo "FAIL: expected removed: $1"; fail=1; }; }
expect_kept() { [ -e "$1" ] || { echo "FAIL: expected kept: $1"; fail=1; }; }

# Build folders: one idle, one recently written, one held open by a process.
for t in idle fresh open; do
  mkdir -p "$H/work/openagents-target-$t/debug/deps" "$H/work/openagents-target-$t/debug/incremental/a"
  echo x >"$H/work/openagents-target-$t/debug/deps/libx.rlib"
done
age_tree "$H/work/openagents-target-idle"
age_tree "$H/work/openagents-target-open"
echo "$H/work/openagents-target-open/debug/deps/libx.rlib" >"$tmp/lsof"

# A repo with a pushed clean worktree, a dirty one, an unpushed one, and a
# fresh one.
git init -q -b main "$tmp/remote.git" --bare
git init -q -b main "$H/work/openagents"
git -C "$H/work/openagents" -c user.email=t@t -c user.name=t commit -q --allow-empty -m one
git -C "$H/work/openagents" remote add origin "$tmp/remote.git"
git -C "$H/work/openagents" push -q origin main
for w in clean dirty unpushed fresh; do
  git -C "$H/work/openagents" worktree add -q --detach "$H/work/oa-$w" main
done
echo change >"$H/work/oa-dirty/new.txt"
git -C "$H/work/oa-unpushed" -c user.email=t@t -c user.name=t commit -q --allow-empty -m local
for w in clean dirty unpushed; do
  age_tree "$H/work/oa-$w"
  gd=$(git -C "$H/work/oa-$w" rev-parse --path-format=absolute --git-dir)
  old "$gd/HEAD" "$gd/index" 2>/dev/null || true
done

# Scratch: a dead session's old folder, an old folder named in the keep
# file, an old folder holding unsaved git work, a recent folder, and a
# dead process's marker-only folder.
mkdir -p "$H/.openagents/scratch/codex-old/x" "$H/.openagents/scratch/codex-kept/x" \
  "$H/.openagents/scratch/codex-new/x" "$H/.openagents/scratch/process-999999"
git -C "$H/work/openagents" worktree add -q --detach "$H/.openagents/scratch/codex-wip/wt" main
echo wip >"$H/.openagents/scratch/codex-wip/wt/wip.txt"
echo s >"$H/.openagents/scratch/process-999999/.session"
age_tree "$H/.openagents/scratch/codex-old"
age_tree "$H/.openagents/scratch/codex-kept"
age_tree "$H/.openagents/scratch/codex-wip"
age_tree "$H/.openagents/scratch/process-999999"
mkdir -p "$H/.openagents/coder/sessions"
echo chat >"$H/.openagents/coder/sessions/a.atif.json"
age_tree "$H/.openagents/coder"
echo "~/.openagents/scratch/codex-kept" >"$H/.openagents/disk-janitor.keep"

# A dry run removes nothing.
bash "$here/disk-janitor.sh" --now --dry-run
expect_kept "$H/work/openagents-target-idle"
expect_kept "$H/work/oa-clean"
grep -q "would-remove target" "$H/.openagents/logs/disk-janitor.log" || { echo "FAIL: dry run logged nothing"; fail=1; }

bash "$here/disk-janitor.sh" --now

expect_gone "$H/work/openagents-target-idle"
expect_kept "$H/work/openagents-target-fresh/debug/deps/libx.rlib"
expect_kept "$H/work/openagents-target-open"
expect_gone "$H/work/oa-clean"
git -C "$H/work/openagents" worktree list | grep -q oa-clean && { echo "FAIL: git still lists oa-clean"; fail=1; }
expect_kept "$H/work/oa-dirty/new.txt"
expect_kept "$H/work/oa-unpushed"
expect_kept "$H/work/oa-fresh"
expect_kept "$H/work/openagents/.git"
expect_gone "$H/.openagents/scratch/codex-old"
expect_gone "$H/.openagents/scratch/process-999999"
expect_kept "$H/.openagents/scratch/codex-kept"
expect_kept "$H/.openagents/scratch/codex-wip/wt/wip.txt"
expect_kept "$H/.openagents/scratch/codex-new"
expect_kept "$H/.openagents/coder/sessions/a.atif.json"
grep -q "removed target" "$H/.openagents/logs/disk-janitor.log" || { echo "FAIL: no removal logged"; fail=1; }

# The off switch stops every run.
rm -rf "$H/.openagents/scratch/codex-new"
mkdir -p "$H/work/openagents-target-later/debug/deps"
age_tree "$H/work/openagents-target-later"
touch "$H/.openagents/disk-janitor.off"
bash "$here/disk-janitor.sh" --now
expect_kept "$H/work/openagents-target-later"

[ "$fail" = 0 ] && echo "disk-janitor tests passed"
exit "$fail"
