#!/bin/sh
# Claude Code hook entry for agent-run.py (#11433): see .claude/settings.json.
# bash-post fires after every Bash call, so it returns at once unless the
# call ran in the background. Turn the run hooks off with OA_RUN_HOOKS=off.
[ "${OA_RUN_HOOKS:-on}" = off ] && exit 0
here=$(dirname "$0")
if [ "$1" = bash-post ]; then
  input=$(cat)
  case "$input" in
    *'"run_in_background":true'* | *'"run_in_background": true'*) ;;
    *) exit 0 ;;
  esac
  printf '%s' "$input" | python3 "$here/agent-run.py" hook bash-post
  exit 0
fi
python3 "$here/agent-run.py" hook "$1"
exit 0
