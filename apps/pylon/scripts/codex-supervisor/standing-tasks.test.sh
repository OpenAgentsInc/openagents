#!/usr/bin/env bash
#
# standing-tasks.test.sh — tests for codex-supervisor standing-task auto-recreate
# (#6711). Stubs the `gh` binary (no network) and asserts:
#   * sup_standing_tasks_to_recreate selects a CLOSED standing task that has NO
#     open sibling by title,
#   * it does NOT select a closed task whose title already has an OPEN sibling
#     (the no-duplicate guard),
#   * when several closed clones share a title, only the title is recreated once
#     (via the most-recent closed source), never one-per-closed-clone,
#   * sup_recreate_closed_standing_tasks issues exactly one `gh issue create`
#     per orphaned title, with the source title/body/labels,
#   * after that fresh clone exists (open sibling present), a second sweep
#     creates nothing (idempotent across runs),
#   * a missing gh recreates nothing (fails soft).
#
# Run: bash apps/pylon/scripts/codex-supervisor/standing-tasks.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

STUB_DIR="$WORK/fixtures"; mkdir -p "$STUB_DIR"
CREATE_LOG="$WORK/created.log"; : > "$CREATE_LOG"

# --- stub gh ---------------------------------------------------------------
#   issue list --label standing-task --state all --json ... -> $STUB_DIR/list.json
#   issue view N --json title,body,labels                   -> $STUB_DIR/view.N.json
#   issue create --title T ...                               -> append T to CREATE_LOG
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
sub="${1:-}"; shift || true
[ "$sub" = "issue" ] || exit 0
action="${1:-}"; shift || true
case "$action" in
  list) cat "$STUB_DIR/list.json" ;;
  view)
    n="${1:-}"
    f="$STUB_DIR/view.$n.json"
    [ -f "$f" ] && cat "$f"
    ;;
  create)
    title=""
    args=("$@"); i=0
    while [ $i -lt ${#args[@]} ]; do
      [ "${args[$i]}" = "--title" ] && title="${args[$((i+1))]}"
      i=$((i+1))
    done
    printf '%s\n' "$title" >> "$CREATE_LOG"
    echo "https://github.com/OpenAgentsInc/openagents/issues/9001"
    ;;
esac
STUB
chmod +x "$WORK/gh"
export STUB_DIR CREATE_LOG
export SUP_GH_BIN="$WORK/gh"
export SUP_REPO="OpenAgentsInc/openagents"

# shellcheck source=standing-tasks.sh
source "$SCRIPT_DIR/standing-tasks.sh"

# Fixture: standing-task issues.
#  - "standing backstop burn"  : CLOSED #6710, no open sibling   -> recreate
#  - "standing triage"         : OPEN #6708 + CLOSED #6700       -> SKIP (open sibling)
#  - "standing promise sync"   : CLOSED #6709 + CLOSED #6690     -> recreate ONCE (from #6709)
printf '%s' '[
  {"number":6710,"title":"standing backstop burn","state":"CLOSED"},
  {"number":6708,"title":"standing triage","state":"OPEN"},
  {"number":6700,"title":"standing triage","state":"CLOSED"},
  {"number":6709,"title":"standing promise sync","state":"CLOSED"},
  {"number":6690,"title":"standing promise sync","state":"CLOSED"}
]' > "$STUB_DIR/list.json"

printf '%s' '{"title":"standing backstop burn","body":"Recurring backstop body","labels":[{"name":"standing-task"},{"name":"prio:4-backstop-burn"}]}' > "$STUB_DIR/view.6710.json"
printf '%s' '{"title":"standing promise sync","body":"Recurring promise body","labels":[{"name":"standing-task"},{"name":"prio:3-product-promises"}]}' > "$STUB_DIR/view.6709.json"

# --- sup_standing_tasks_to_recreate ---------------------------------------
to_recreate="$(sup_standing_tasks_to_recreate | sort -n | tr '\n' ' ')"
[ "$to_recreate" = "6709 6710 " ] \
  && ok "to_recreate selects orphaned closed titles only (#6709,#6710), skips open-sibling title" \
  || bad "to_recreate returned '$to_recreate' (want '6709 6710 ')"

# The open-sibling title (#6708/#6700 "standing triage") must NOT appear.
if printf '%s' "$to_recreate" | grep -qE '(^| )6700( |$)'; then
  bad "to_recreate must not select #6700 (its title has an open sibling)"
else
  ok "no-dup guard: closed #6700 skipped (open sibling #6708 exists)"
fi
# The duplicate-closed-clone title must appear at most once (via newest #6709, not #6690).
dup_count="$(printf '%s\n' $to_recreate | grep -cE '6690')"
[ "$dup_count" = "0" ] && ok "duplicate closed clone #6690 not double-counted (newest #6709 used)" \
  || bad "duplicate closed clone #6690 was selected"

# --- sup_recreate_closed_standing_tasks -----------------------------------
made="$(sup_recreate_closed_standing_tasks)"
[ "$made" = "2" ] && ok "recreate created 2 issues (one per orphaned title)" || bad "recreate count '$made' (want 2)"
created_titles="$(sort "$CREATE_LOG" | tr '\n' '|')"
[ "$created_titles" = "standing backstop burn|standing promise sync|" ] \
  && ok "recreate used the source titles" || bad "created titles '$created_titles'"

# --- idempotent across runs -----------------------------------------------
# Simulate the recreate: both titles now have an OPEN sibling.
printf '%s' '[
  {"number":6710,"title":"standing backstop burn","state":"CLOSED"},
  {"number":9101,"title":"standing backstop burn","state":"OPEN"},
  {"number":6708,"title":"standing triage","state":"OPEN"},
  {"number":6709,"title":"standing promise sync","state":"CLOSED"},
  {"number":9102,"title":"standing promise sync","state":"OPEN"}
]' > "$STUB_DIR/list.json"
: > "$CREATE_LOG"
made2="$(sup_recreate_closed_standing_tasks)"
[ "$made2" = "0" ] && ok "second sweep creates nothing (idempotent; open siblings now exist)" \
  || bad "second sweep created '$made2' (want 0)"
[ ! -s "$CREATE_LOG" ] && ok "no gh issue create calls on idempotent sweep" || bad "unexpected create calls: $(cat "$CREATE_LOG")"

# --- fails soft when gh missing -------------------------------------------
if SUP_GH_BIN="$WORK/does-not-exist" sup_recreate_closed_standing_tasks >/dev/null 2>&1; then
  bad "missing gh should return nonzero (fail soft, no recreation)"
else
  ok "missing gh fails soft (no recreation)"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
