#!/usr/bin/env bash
#
# replenishment.test.sh — tests for codex-supervisor LOCKOUT replenishment (#6822).
#
# Run: bash apps/pylon/scripts/codex-supervisor/replenishment.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

export SUP_STATE_DIR="$WORK/state"
export SUP_LOCKOUT_CACHE_DIR="$WORK/cache"
export SUP_REPO="OpenAgentsInc/openagents"
export SUP_REPLENISHMENT_LABEL="supervisor-replenishment"
export SUP_REPLENISHMENT_MAX_CREATE=3
export SUP_GH_TIMEOUT_SECS=5
mkdir -p "$SUP_STATE_DIR" "$SUP_LOCKOUT_CACHE_DIR" "$WORK/bin" "$WORK/gh-state"

cat > "$WORK/bin/gh" <<'STUB'
#!/usr/bin/env bash
set -uo pipefail

state_dir="${STUB_GH_STATE_DIR:?}"
mkdir -p "$state_dir"
open_file="$state_dir/open.tsv"
create_log="$state_dir/create.log"
touch "$open_file" "$create_log"

if [ "${1:-}" = "issue" ] && [ "${2:-}" = "list" ]; then
  python3 - "$open_file" <<'PY'
import json,sys
rows=[]
with open(sys.argv[1], encoding="utf-8") as handle:
    for line in handle:
        line=line.rstrip("\n")
        if not line:
            continue
        number,title=line.split("\t",1)
        rows.append({"number": int(number), "title": title})
print(json.dumps(rows))
PY
  exit 0
fi

if [ "${1:-}" = "issue" ] && [ "${2:-}" = "create" ]; then
  title=""
  body=""
  labels=()
  shift 2
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --title) title="$2"; shift 2 ;;
      --body) body="$2"; shift 2 ;;
      --label) labels+=("$2"); shift 2 ;;
      --repo) shift 2 ;;
      *) shift ;;
    esac
  done
  next="$(python3 - "$open_file" <<'PY'
import sys
highest=8000
with open(sys.argv[1], encoding="utf-8") as handle:
    for line in handle:
        if not line.strip():
            continue
        highest=max(highest, int(line.split("\t",1)[0]))
print(highest+1)
PY
)"
  printf '%s\t%s\n' "$next" "$title" >> "$open_file"
  printf '%s|%s|%s\n' "$next" "$title" "$(IFS=,; printf '%s' "${labels[*]}")" >> "$create_log"
  case "$body" in
    *"multi-session-campaign.ts"*) : ;;
    *) echo "body missing forbidden-stub boundary" >&2; exit 3 ;;
  esac
  printf 'https://github.com/OpenAgentsInc/openagents/issues/%s\n' "$next"
  exit 0
fi

echo "unexpected gh args: $*" >&2
exit 2
STUB
chmod +x "$WORK/bin/gh"

export STUB_GH_STATE_DIR="$WORK/gh-state"
export SUP_GH_BIN="$WORK/bin/gh"

# shellcheck source=replenishment.sh
source "$SCRIPT_DIR/replenishment.sh"

first="$(sup_ensure_replenishment_issues | tr '\n' ' ')"
[ "$first" = "8001 8002 8003 " ] && ok "creates the three bounded replenishment issues" \
  || bad "first replenishment returned '$first'"

created_count="$(wc -l < "$WORK/gh-state/create.log" | tr -d ' ')"
[ "$created_count" = "3" ] && ok "creation bounded to three templates" || bad "created '$created_count' issues"

if grep -q 'supervisor-replenishment,standing-task,prio:4-backstop-burn' "$WORK/gh-state/create.log"; then
  ok "created issues carry replenishment, standing-task, and backstop labels"
else
  bad "created issues missing expected labels"
fi

second="$(sup_ensure_replenishment_issues | tr '\n' ' ')"
[ "$second" = "8001 8002 8003 " ] && ok "reuses exact-title open replenishment issues" \
  || bad "second replenishment returned '$second'"

created_count2="$(wc -l < "$WORK/gh-state/create.log" | tr -d ' ')"
[ "$created_count2" = "3" ] && ok "dedupe prevents duplicate issue spam" || bad "created count after rerun '$created_count2'"

export SUP_REPLENISHMENT_MAX_CREATE=1
rm -rf "$SUP_LOCKOUT_CACHE_DIR/replenishment.lock"
rm -f "$WORK/gh-state/open.tsv" "$WORK/gh-state/create.log"
touch "$WORK/gh-state/open.tsv" "$WORK/gh-state/create.log"
limited="$(sup_ensure_replenishment_issues | tr '\n' ' ')"
[ "$limited" = "8001 " ] && ok "SUP_REPLENISHMENT_MAX_CREATE bounds one pass" \
  || bad "limited replenishment returned '$limited'"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
