#!/usr/bin/env bash
# Tests scripts/dev/issue-board.sh against a fake `gh` (no live GitHub calls).
# Run: scripts/dev/issue-board.test.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
export FAKE_LOG="$tmp/log"

# The fake: project 22 holds #7 (and #70, whose title mentions 7); project 19
# does not hold #7; project 5 cannot be read. PATCH on project 22 succeeds.
cat >"$tmp/bin/gh" <<'FAKE'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$FAKE_LOG"
jq_filter=; method=GET; path=
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  case ${args[i]} in
    --jq) jq_filter=${args[i+1]} ;;
    -X) method=${args[i+1]} ;;
    /*) [[ -z $path ]] && path=${args[i]} ;;
  esac
done
out() { if [[ -n $jq_filter ]]; then jq -r "$jq_filter"; else cat; fi; }
case "$1 $2" in
  "issue comment"|"issue close") [[ ${FAKE_CLOSE_FAILS:-} == 1 && $2 == close ]] && exit 1; exit 0 ;;
esac
case "$method $path" in
  "GET /orgs/Acme/projectsV2?per_page=100")
    echo '[{"number":22,"state":"open"},{"number":19,"state":"open"},{"number":5,"state":"open"},{"number":3,"state":"closed"}]' | out ;;
  "GET /orgs/Acme/projectsV2/22/items")
    echo '[{"id":900,"content":{"number":70,"repository_url":"https://api.github.com/repos/Acme/app"}},
           {"id":901,"content":{"number":7,"repository_url":"https://api.github.com/repos/Acme/app"}},
           {"id":902,"content":{"number":7,"repository_url":"https://api.github.com/repos/Acme/other"}}]' | out ;;
  "GET /orgs/Acme/projectsV2/19/items") echo '[]' | out ;;
  "GET /orgs/Acme/projectsV2/5/items") exit 1 ;;
  "GET /orgs/Acme/projectsV2/22/fields")
    echo '[{"id":11,"name":"Title"},{"id":42,"name":"Status","options":[
      {"id":"t0","name":{"raw":"Todo"}},{"id":"p1","name":{"raw":"In Progress"}},
      {"id":"b2","name":{"raw":"Blocked"}},{"id":"d3","name":{"raw":"Done"}}]}]' | out ;;
  "PATCH /orgs/Acme/projectsV2/22/items/901") cat >>"$FAKE_LOG"; echo >>"$FAKE_LOG"; echo '{}' ;;
  *) echo "unexpected: $*" >&2; exit 1 ;;
esac
FAKE
chmod +x "$tmp/bin/gh"
export PATH="$tmp/bin:$PATH" ISSUE_BOARD_REPO=Acme/app

fail=0
check() { # name, expected substring, text
  if [[ $3 == *"$2"* ]]; then echo "ok   $1"; else echo "FAIL $1: wanted \"$2\" in:"; echo "$3"; fail=1; fi
}
absent() {
  if [[ $3 != *"$2"* ]]; then echo "ok   $1"; else echo "FAIL $1: did not want \"$2\" in:"; echo "$3"; fail=1; fi
}

: >"$FAKE_LOG"
said=$("$here/issue-board.sh" claim 7)
check "claim moves the matching item" "board: #7 -> In Progress on project 22." "$said"
check "claim says an unreadable project" "could not read project 5" "$said"
check "claim patches item 901 with the option id" '{"fields":[{"id":42,"value":"p1"}]}' "$(cat "$FAKE_LOG")"
absent "claim leaves #70 and the other repo's #7 alone" "items/900" "$(cat "$FAKE_LOG")"
absent "claim skips the closed project" "projectsV2/3/" "$(cat "$FAKE_LOG")"

: >"$FAKE_LOG"
said=$("$here/issue-board.sh" block 7 "waits on the owner's key")
check "block comments the reason" "issue comment 7 -R Acme/app --body Blocked: waits on the owner's key" "$(cat "$FAKE_LOG")"
check "block moves to Blocked" '"value":"b2"' "$(cat "$FAKE_LOG")"
check "block says it" "#7 -> Blocked on project 22." "$said"

set +e
"$here/issue-board.sh" block 7 >/dev/null 2>&1; code=$?
set -e
check "block without a reason is refused" "2" "$code"

: >"$FAKE_LOG"
said=$("$here/issue-board.sh" close 7 "Landed in abc123.")
check "close closes with the comment" "issue close 7 -R Acme/app --reason completed --comment Landed in abc123." "$(cat "$FAKE_LOG")"
check "close moves to Done" '"value":"d3"' "$(cat "$FAKE_LOG")"

: >"$FAKE_LOG"
said=$("$here/issue-board.sh" set 7 "todo")
check "set matches without regard to case" '"value":"t0"' "$(cat "$FAKE_LOG")"

said=$("$here/issue-board.sh" set 7 "In review")
check "a missing value is said, not fatal" 'project 22 has no Status "In review"' "$said"

: >"$FAKE_LOG"
set +e
said=$(FAKE_CLOSE_FAILS=1 "$here/issue-board.sh" close 7); code=$?
set -e
check "a failed close exits 1" "1" "$code"
absent "a failed close leaves the board alone" "PATCH" "$(cat "$FAKE_LOG")"

: >"$FAKE_LOG"
said=$(ISSUE_BOARD_PROJECTS=19 "$here/issue-board.sh" claim 7)
check "an issue on no listed project is said" "is on no project" "$said"
absent "ISSUE_BOARD_PROJECTS skips the project listing" "per_page=100 --jq" "$(head -n1 "$FAKE_LOG")"

exit $fail
