#!/usr/bin/env bash
#
# virtual-merge-queue.test.sh — focused tests for supervisor virtual HEAD.
#
# Run: bash apps/pylon/scripts/codex-supervisor/virtual-merge-queue.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

export SUP_STATE_DIR="$WORK/state"
export SUP_VMQ_DIR="$WORK/vmq"
export SUP_VMQ_MAX_BRANCHES=24

# shellcheck source=virtual-merge-queue.sh
source "$SCRIPT_DIR/virtual-merge-queue.sh"

git_init_repo() {
  local repo="$1"
  mkdir -p "$repo"
  git -C "$repo" init -b main >/dev/null 2>&1 || return 1
  git -C "$repo" config user.email "pylon-test@example.invalid"
  git -C "$repo" config user.name "Pylon Test"
}

git_commit_file() {
  local repo="$1" file="$2" body="$3" msg="$4"
  printf '%s\n' "$body" > "$repo/$file"
  git -C "$repo" add "$file" >/dev/null 2>&1 || return 1
  git -C "$repo" commit -m "$msg" >/dev/null 2>&1
}

repo="$WORK/repo"
git_init_repo "$repo" || { echo "git init failed" >&2; exit 1; }
git_commit_file "$repo" README.md "base" "base" || exit 1
base1="$(git -C "$repo" rev-parse HEAD)"

git -C "$repo" checkout -b pylon/assignment-issue-100 >/dev/null 2>&1
git_commit_file "$repo" feature-a.txt "feature a" "feature a" || exit 1
git -C "$repo" checkout main >/dev/null 2>&1

projected1="$(sup_vmq_project_head "$repo" "$base1")"
if git -C "$SUP_VMQ_DIR" show "$projected1:feature-a.txt" >/dev/null 2>&1; then
  ok "projects a clean assignment branch onto main"
else
  bad "projected head is missing clean assignment branch content"
fi

git -C "$repo" checkout -b pylon/assignment-issue-101 main >/dev/null 2>&1
git_commit_file "$repo" conflict.txt "from branch" "conflicting branch" || exit 1
git -C "$repo" checkout main >/dev/null 2>&1
git_commit_file "$repo" conflict.txt "from main" "main advanced" || exit 1
base2="$(git -C "$repo" rev-parse HEAD)"

projected2="$(sup_vmq_project_head "$repo" "$base2")"
if [ -n "$projected2" ] && git -C "$SUP_VMQ_DIR" show "$projected2:feature-a.txt" >/dev/null 2>&1; then
  ok "skips a conflicting assignment branch and keeps projecting clean branches"
else
  bad "conflicting assignment branch stopped virtual-head projection"
fi

if [ "$(git -C "$SUP_VMQ_DIR" show "$projected2:conflict.txt" 2>/dev/null)" = "from main" ]; then
  ok "conflicting branch does not overwrite advanced main content"
else
  bad "conflicting branch changed advanced main content"
fi

git -C "$repo" checkout main >/dev/null 2>&1
git_commit_file "$repo" main-advance.txt "new main" "main advances again" || exit 1
base3="$(git -C "$repo" rev-parse HEAD)"
projected3="$(sup_vmq_project_head "$repo" "$base3")"
if [ "$projected3" != "$projected2" ] && git -C "$SUP_VMQ_DIR" show "$projected3:main-advance.txt" >/dev/null 2>&1; then
  ok "recomputes projected head after main updates"
else
  bad "projected head did not refresh after main update"
fi

git -C "$repo" checkout -b pylon/assignment-issue-102 main >/dev/null 2>&1
git_commit_file "$repo" stale-pr.txt "stale branch" "stale closed branch" || exit 1
git -C "$repo" checkout main >/dev/null 2>&1

cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' '[{"headRefName":"pylon/assignment-issue-100"}]'
STUB
chmod +x "$WORK/gh"
SUP_GH_BIN="$WORK/gh" SUP_REPO="OpenAgentsInc/openagents" projected_open_only="$(sup_vmq_project_head "$repo" "$base3")"
if git -C "$SUP_VMQ_DIR" show "$projected_open_only:stale-pr.txt" >/dev/null 2>&1; then
  bad "virtual queue replayed a branch absent from the open PR list"
else
  ok "open-PR candidate list excludes stale assignment branches"
fi

disabled="$(SUP_VMQ_ENABLED=0 sup_vmq_project_head "$repo" "$base3" 2>/dev/null)"
if [ -z "$disabled" ]; then
  ok "SUP_VMQ_ENABLED=0 disables projection"
else
  bad "disabled virtual merge queue still returned '$disabled'"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
