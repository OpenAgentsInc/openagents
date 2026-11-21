#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds the CLI and runs a scripted session; every rule is Coax).
# The gamebuilder CLI end to end, in a folder of its own: a level and a game
# made, a thing added and changed, a refusal said with its number, a dry run
# that writes nothing, a batch against the revision read, and the manifest.
# scripts/check.sh runs it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="$("$here/build.sh" | tail -1)"
dir="$(mktemp -d)"; trap 'rm -rf "$dir"' EXIT
gb() { "$bin" --dir "$dir" "$@" < /dev/null; }   # no terminal: nothing waits for typing
fails=0
expect() {   # expect EXIT PART -- ARGS...
  local want="$1" part="$2"; shift 3
  local got code=0; got="$(gb "$@" 2>&1)" || code=$?
  if [[ "$code" != "$want" || "$got" != *"$part"* ]]; then
    echo "FAIL gamebuilder $*: exit $code, wanted $want and \"$part\"; got: $got" >&2; fails=$((fails + 1))
  fi
}
expect 0 '"levels":[]'                            -- list-levels
expect 0 '{"ok":true,"level":1}'                  -- new-level
expect 0 '"game":1,"level":2,"template":"adventure"' -- new-game adventure
expect 0 '"title":"Moss Run"'                     -- rename-game 1 "Moss Run"
expect 0 '"title":"Moss Run","template":"adventure","levels":[2]' -- list-games
expect 1 "this game's template has no such thing" -- add 2 ball
id="$(gb add 1 block --x 500 --z -300 | sed -n 's/.*"id":\([0-9]*\).*/\1/p')"
[[ -n "$id" ]] || { echo "FAIL add gave no id" >&2; fails=$((fails + 1)); }
expect 0 '"field":"East","value":800'             -- set-field 1 "$id" east 800
expect 1 'Turn is from -180 to 180°'              -- set-field 1 "$id" turn 900
expect 0 '"dry_run":true'                         -- delete 1 "$id" --dry-run
expect 0 "\"id\":$id,\"kind\":"                   -- read-level 1
rev="$(gb read-level 1 | sed -n 's/^{"schema":"gamebuilder.level.v1","rev":\([0-9]*\).*/\1/p')"
expect 0 "\"ids\":[$id]" -- apply-edits 1 "{\"schema\":\"world.edit.v1\",\"rev\":$rev,\"do\":\"apply\",\"edits\":[{\"op\":\"move\",\"id\":$id,\"x\":0,\"y\":0,\"z\":0}]}"
expect 1 '"error":"stale"' -- apply-edits 1 "{\"schema\":\"world.edit.v1\",\"rev\":$rev,\"do\":\"apply\",\"edits\":[{\"op\":\"move\",\"id\":$id,\"x\":100,\"y\":0,\"z\":0}]}"
expect 0 '"level":3,"from":0'                     -- copy-level 0
expect 1 'there is no such level'                 -- read-level 7
expect 3 'no command frobnicate'                  -- frobnicate
# the server's model key: made once, mode 0600, only its public key said; the same again, the same key
expect 0 '"pubkey":"' -- cloud-key "$dir/model.key"
[[ "$(stat -f %Lp "$dir/model.key" 2>/dev/null || stat -c %a "$dir/model.key")" == 600 ]] || { echo "FAIL cloud-key's file is not 0600" >&2; fails=$((fails + 1)); }
[[ "$(gb cloud-key "$dir/model.key")" == "$(gb cloud-key "$dir/model.key")" ]] || { echo "FAIL cloud-key made a second key" >&2; fails=$((fails + 1)); }
grep -q "$(head -c 64 "$dir/model.key")" <(gb cloud-key "$dir/model.key") && { echo "FAIL cloud-key printed the key" >&2; fails=$((fails + 1)); }
echo "not a key" > "$dir/bad.key"; expect 1 "doesn't hold a key" -- cloud-key "$dir/bad.key"
expect 3 "one of the app's own buttons"           -- undo 1
# --key: the first result is kept, and the same call again answers it and changes nothing
expect 0 '{"ok":true,"level":4,"key":"try-1"}'   -- new-level --key try-1
expect 0 '{"ok":true,"level":4,"key":"try-1"}'   -- new-level --key try-1
[[ ! -f "$dir/level-5.world" ]] || { echo "FAIL a repeat with the same key made another level" >&2; fails=$((fails + 1)); }
expect 1 'a key is one call'                      -- delete 1 1 --key try-1
expect 1 'starting with a letter or a digit'      -- new-level --key ../x
expect 0 '"dry_run":true'                         -- new-level --dry-run --key dry-1
[[ ! -f "$dir/results/dry-1" ]] || { echo "FAIL a dry run's result was kept" >&2; fails=$((fails + 1)); }
expect 0 '"schema":"gamebuilder.commands.v1"'     -- commands
# a one-off call is no agent: none of the calls above made a record (cli/attached.coax)
! compgen -G "$dir/agent-*" > /dev/null || { echo "FAIL a one-off call made an agent record" >&2; fails=$((fails + 1)); }
# mcp (coax/mcp.coax): a session on stdio sees the tools and edits the same files; a refusal is the CLI's own words
mcp="$(printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"gb_add","arguments":{"level":1,"kind":"block","x":100,"z":100}}}' \
  '{"jsonrpc":"2.0","id":"s","method":"tools/call","params":{"name":"gb_set_field","arguments":{"level":1,"thing":1,"field":"turn","value":900}}}' \
  | "$bin" --dir "$dir" mcp)"
[[ "$(wc -l <<< "$mcp")" -eq 4 && "$mcp" == *'"name":"gb_apply_edits"'* && "$mcp" == *'{\"ok\":true,\"id\":'* \
  && "$mcp" == *'"id":"s","result"'*'Turn is from -180 to 180°'*'"isError":true'* && "$mcp" != *'gb_login'* ]] \
  || { echo "FAIL gamebuilder mcp: $mcp" >&2; fails=$((fails + 1)); }
[[ -f "$dir/level-1.world" && -f "$dir/level-3.world" && -f "$dir/game-1.game" ]] || { echo "FAIL the files were not written" >&2; fails=$((fails + 1)); }
# the session was an attached agent: one record, its calls logged, done when its input ended
recs=("$dir"/agent-*.agent); rec="$(cat "${recs[0]}" 2>/dev/null)"; log="$(cat "${recs[0]%.agent}.events" 2>/dev/null)"
[[ "${#recs[@]}" -eq 1 && "$rec" == *'"place":"attached"'* && "$rec" == *'"state":"done"'* \
  && "$log" == *'"tool":"gb_add"'* && "$log" == *'"type":"edit"'* && "$log" == *'"tool":"gb_set_field"'* ]] \
  || { echo "FAIL the MCP session's record: ${recs[*]}: $rec $log" >&2; fails=$((fails + 1)); }
# --as-agent: one record for the name, kept across calls; a name that can't be a file is refused
expect 0 '"levels":['                             -- --as-agent codex list-levels
expect 0 '"levels":['                             -- --as-agent codex list-levels
[[ "$(grep -l '"runner":"codex"' "$dir"/agent-*.agent | wc -l)" -eq 1 ]] || { echo "FAIL --as-agent made more than one record for codex" >&2; fails=$((fails + 1)); }
expect 1 'a name of 1 to 16'                     -- --as-agent '../x' list-levels
# The account, offline, in a home of its own: nobody logged in, the app's
# login read only while its access token lasts, a bad profile, words the
# login refuses before it asks the server, and logout.
home="$dir/home"; mkdir -p "$home/.openagents/gamebuilder"
export GAMEBUILDER_PROFILE="" GAMEBUILDER_ORIGIN=""
HOME="$home" expect 1 '"error":"not-logged-in"'   -- whoami
printf '{"access":"oa_at_old_token","refresh":"oa_rt_never_used","accessUntil":100,"account":"acct_x","deviceBound":true}' > "$home/.openagents/gamebuilder/login.json"
HOME="$home" expect 1 '"error":"not-logged-in"'   -- whoami
HOME="$home" expect 1 '--profile is 1 to 32'      -- whoami --profile 'No/Such'
HOME="$home" expect 1 'it takes no arguments'     -- whoami me
HOME="$home" expect 1 'no code was asked for'   -- login --code 123456
HOME="$home" expect 1 'isn'"'"'t an email address' -- login --email nobody
# the browser's way in (docs/cli.md, "Log in from a browser"): the page is named and no browser is
# opened here; a code of the wrong shape is refused before the server is asked; one way in at a time
export GAMEBUILDER_NO_BROWSER=1
HOME="$home" expect 1 'no code was pasted'        -- login --browser
HOME="$home" expect 1 '/login/cli'                -- login --browser
HOME="$home" expect 1 "isn't the page's code"    -- login --browser --code 482913
HOME="$home" expect 1 "isn't the page's code"    -- login --browser --code 0123456789abcdef0123456789abcdeg
HOME="$home" expect 1 'two ways in'               -- login --browser --email me@example.com
# with no terminal, a bare login is the email's: it asks for the email on stdin
HOME="$home" expect 1 'isn'"'"'t an email address' -- login
HOME="$home" expect 0 '"profile":"default"'       -- logout
[[ ! -e "$home/.openagents/gamebuilder/cli-login-default" ]] || { echo "FAIL a login was kept without one" >&2; fails=$((fails + 1)); }
# The golden cases (tests/commands): each call gives the same answer through the core, the CLI, MCP and HTTP (serve).
(cd "$here/.." && HOME="$home" GAMEBUILDER_NOW=1763380000000 GAMEBUILDER_COMPUTER="this Mac" "$here/../../coax/web/tool.sh" tools/commands-golden.coax "$bin" tests/commands/*.case 2>/dev/null | tail -n 20) \
  || { echo "FAIL the golden cases (tools/commands-golden.coax)" >&2; fails=$((fails + 1)); }
# --attach end to end: the tool plays the app's part on a real socket (coax/attach.coax).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/attach-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL --attach (tools/attach-check.coax)" >&2; fails=$((fails + 1)); }
# serve --accounts end to end: the tool stands in for the login service (tools/hosted-check.coax).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/hosted-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL serve --accounts (tools/hosted-check.coax)" >&2; fails=$((fails + 1)); }
# A cloud agent on serve --accounts end to end: the tool stands in for the login service and OpenAgents'
# model route, and its tool calls change the account's level through the rows (tools/cloud-check.coax; issue 7).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/cloud-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL a cloud agent (tools/cloud-check.coax)" >&2; fails=$((fails + 1)); }
# The resident service runs agents queued on this computer with the app closed, a stand-in for
# Claude Code its turns, and is quit, killed and paused (tools/resident-check.coax; issue 11).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/resident-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL the resident service (tools/resident-check.coax)" >&2; fails=$((fails + 1)); }
# The website's saving against the app's: the page's own account module asks serve --accounts as a logged-in
# browser, and the files are held byte for byte (tools/web-account-check.coax; issue 16).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/web-account-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL the website's saving against the app's (tools/web-account-check.coax)" >&2; fails=$((fails + 1)); }
# The app's sync against the website's saving, both ways: the app's own sync and the page's own account module ask
# the same serve --accounts; games cross as games of their type, and a deletion is not undone
# (tools/sync-games-check.coax; issue 19).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/sync-games-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL the app's sync against the website's saving (tools/sync-games-check.coax)" >&2; fails=$((fails + 1)); }
# The website's publishing against the app's core: the page's own module publishes through the rows on the real
# hosted server, and the link plays the published file with no login (tools/web-publish-check.coax; issue 17).
(cd "$here/.." && HOME="$home" "$here/../../coax/web/tool.sh" tools/web-publish-check.coax "$bin" 2>/dev/null | tail -n 20) \
  || { echo "FAIL the website's publishing against the app's (tools/web-publish-check.coax)" >&2; fails=$((fails + 1)); }
# The skill: made from the table, and the repo's copy is the same (gamebuilder skill --install).
expect 0 '"repo":"OpenAgentsInc/openagents"'      -- engine
[[ "$("$bin" skill)" == "$(cat "$here/../.claude/skills/gamebuilder/SKILL.md")" ]] || { echo "FAIL .claude/skills/gamebuilder/SKILL.md is not the table's: run build/cli/gamebuilder skill --install" >&2; fails=$((fails + 1)); }
(cd "$dir" && "$bin" skill --install > /dev/null) && [[ -s "$dir/.claude/skills/gamebuilder/SKILL.md" ]] || { echo "FAIL skill --install wrote nothing" >&2; fails=$((fails + 1)); }
[[ "$fails" == 0 ]] || exit 1
echo "cli: ok"
