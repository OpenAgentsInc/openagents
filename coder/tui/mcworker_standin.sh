#!/bin/sh
# Firm reason: 5 (a fixture: a fixed script of worker lines for coder/tui/host_smoke.coax, no model)
# A stand-in Microcoder worker (coder/tui/mcworker.coax's lines, coax/lib/agent/engine/mcworker-wire.coax)
# for the Coder Desktop chat that coder/tui/host_smoke.coax shows on the paired phone (GitHub #13713).
# The daemon runs it as a worker run under a name ending in /mcworker. It reads one line at a time:
#   a task          a chat's turn: a thought, then a delegate line, and it keeps running;
#                   a sub-agent's task (its session has -agent-): a reply and a command, and it keeps running
#   a steer         accepted with its words and applied, then a reply (rejected when no turn runs)
#   an interrupt    the turn ends stopped
#   {"stop":true}   the turn ends stopped; a sub-agent exits
#   queue add       kept and the queue line said; queue promote starts it as a turn that finishes
say() { printf '%s\n' "$1"; }
# the last "NAME":"VALUE" in a line (values here hold no quotes)
field() { printf '%s' "$1" | sed -n "s/.*\"$2\":\"\([^\"]*\)\".*/\1/p"; }
say '{"kind":"ready"}'
turn=0; running=0; sub=0; queued=""; queued_id=""
while IFS= read -r line; do
  case "$line" in
    '{"stop":true}'*)
      if [ "$running" = 1 ]; then say '{"kind":"ended","ending":"stopped","text":""}'; running=0; fi
      if [ "$sub" = 1 ]; then exit 0; fi ;;
    '{"steer":'*)
      id=$(field "$line" id); text=$(printf '%s' "$line" | sed -n 's/^{"steer":"\([^"]*\)".*/\1/p')
      if [ "$running" = 1 ]; then
        say "{\"kind\":\"steer\",\"id\":\"$id\",\"status\":\"accepted\",\"turn\":$turn,\"text\":\"$text\"}"
        say "{\"kind\":\"steer\",\"id\":\"$id\",\"status\":\"applied\",\"turn\":$turn,\"step\":2}"
        say '{"kind":"reply","text":"Taking that into account."}'
      else
        say "{\"kind\":\"steer\",\"id\":\"$id\",\"status\":\"rejected\",\"turn\":$turn,\"reason\":\"no_active_turn\"}"
      fi ;;
    '{"interrupt":'*)
      if [ "$running" = 1 ]; then say '{"kind":"ended","ending":"stopped","text":""}'; running=0; fi ;;
    '{"queue":"add"'*)
      queued_id=$(printf '%s' "$line" | sed -n 's/^{"queue":"add","id":"\([^"]*\)".*/\1/p')
      queued=$(printf '%s' "$line" | sed -n 's/^{"queue":"add","id":"[^"]*","text":"\([^"]*\)".*/\1/p')
      say "{\"kind\":\"queue\",\"paused\":false,\"reason\":\"\",\"active\":\"\",\"items\":[{\"id\":\"$queued_id\",\"text\":\"$queued\"}]}" ;;
    '{"queue":"promote"'*)
      if [ "$running" = 0 ] && [ -n "$queued" ]; then
        turn=$((turn + 1))
        say "{\"kind\":\"turn\",\"turn\":$turn,\"source\":\"queue\",\"id\":\"$queued_id\",\"text\":\"$queued\"}"
        say "{\"kind\":\"queue\",\"paused\":false,\"reason\":\"\",\"active\":\"$queued_id\",\"items\":[]}"
        say '{"kind":"reply","text":"Follow-up done."}'
        say '{"kind":"ended","ending":"finished","text":"Follow-up done."}'
        queued=""
      fi ;;
    '{"queue":'*) ;;
    '{"task":'*)
      turn=$((turn + 1)); running=1
      case "$line" in
        *'-agent-'*)
          sub=1
          say "{\"kind\":\"turn\",\"turn\":$turn,\"source\":\"message\",\"id\":\"\",\"text\":\"\"}"
          say '{"kind":"reply","text":"Looking at the tests first."}'
          say '{"kind":"command","command":"ls","exit":0,"timed_out":false,"output":"a_test.coax"}' ;;
        *)
          text=$(printf '%s' "$line" | sed -n 's/^{"task":"\([^"]*\)".*/\1/p')
          say "{\"kind\":\"turn\",\"turn\":$turn,\"source\":\"message\",\"id\":\"\",\"text\":\"$text\"}"
          say '{"kind":"thought","text":"A sub-agent can survey the tests."}'
          say '{"kind":"delegate","tasks":[{"level":"hydrogen","task":"Survey the tests"}],"ticket":1}'
          # the round answered early, the chat waits on its still-running sub-agent again (#13766): no new call
          say '{"kind":"delegate","tasks":[{"level":"hydrogen","task":"Wait for the survey","resume":1}],"ticket":2}' ;;
      esac ;;
  esac
done
