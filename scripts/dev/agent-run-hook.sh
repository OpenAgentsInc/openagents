#!/bin/sh
# Claude Code hook entry for agent-run.py (#11433): see .claude/settings.json.
# bash-post records activity after every Bash call. Turn the run hooks off
# with OA_RUN_HOOKS=off.
[ "${OA_RUN_HOOKS:-on}" = off ] && exit 0
here=$(dirname "$0")
python3 "$here/agent-run.py" hook "$1"
exit 0
