#!/usr/bin/env bash
# agent-gate: keeps Claude Code sessions in this repository from draining the
# shared Claude login (#11430, docs/audits/2026-10-12-claude-usage-burn).
#
# Claude Code runs it as hooks (.claude/settings.json):
#   pre    PreToolUse on Agent|Workflow: refuses a new agent or workflow when
#          the capacity book says claude is out (`openagents capacity check
#          claude`), when a usage limit parked agents and nobody resumed, or
#          when OA_AGENT_MAX agents (default 4) are already running.
#   start  SubagentStart: counts a running agent.
#   stop   SubagentStop: uncounts it.
#   limit  StopFailure / PostToolUseFailure: when the failure is a usage
#          limit, records it in the capacity book and parks new agents until
#          a person runs `agent-gate.sh resume` (a limit is a stop, not a nap).
# By hand:
#   status   what the gate sees now
#   resume   clear the park after a limit (the capacity book still applies)
#
# State lives in ~/.openagents/agent-gate (OA_AGENT_GATE_DIR). Turn the gate
# off with OA_AGENT_GATE=off.

set -uo pipefail

DIR=${OA_AGENT_GATE_DIR:-$HOME/.openagents/agent-gate}
MAX=${OA_AGENT_MAX:-4}
STALE_MIN=${OA_AGENT_STALE_MIN:-360}
OPENAGENTS=${OA_OPENAGENTS_BIN:-$(command -v openagents || echo "$HOME/.openagents/bin/openagents")}
mkdir -p "$DIR/active"

deny() {
  jq -cn --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}'
  exit 0
}

active_count() {
  # Agents that never reported stopping (a killed session) age out.
  find "$DIR/active" -type f -mmin +"$STALE_MIN" -delete 2>/dev/null
  find "$DIR/active" -type f | wc -l | tr -d ' '
}

agent_key() {
  jq -r '.agent_id // .subagent_id // .tool_use_id // .session_id // empty' 2>/dev/null | tr -c 'A-Za-z0-9_.-' '_' | cut -c1-120
}

mode=${1:-status}
[ "${OA_AGENT_GATE:-on}" = off ] && [ "$mode" != status ] && [ "$mode" != resume ] && exit 0

case "$mode" in
  pre)
    if [ -f "$DIR/parked" ]; then
      deny "A Claude usage limit stopped agents here ($(cat "$DIR/parked")). New agents wait until a person runs scripts/dev/agent-gate.sh resume."
    fi
    if [ -x "$OPENAGENTS" ] && ! out=$("$OPENAGENTS" capacity check claude 2>&1); then
      deny "The shared Claude login is out of capacity: ${out:-a recorded limit holds}. No new agents until it resets."
    fi
    n=$(active_count)
    if [ "$n" -ge "$MAX" ]; then
      deny "$n agents are already running (the limit is $MAX, OA_AGENT_MAX). Wait for one to finish."
    fi
    exit 0
    ;;
  start)
    key=$(agent_key)
    [ -n "$key" ] && touch "$DIR/active/$key"
    exit 0
    ;;
  stop)
    key=$(agent_key)
    [ -n "$key" ] && rm -f "$DIR/active/$key"
    exit 0
    ;;
  limit)
    payload=$(cat)
    text=$(printf '%s' "$payload" | jq -r '[.error, .error_message, .message, .reason, (.tool_response|tostring?)] | map(select(. != null)) | join(" ")' 2>/dev/null)
    if printf '%s' "$text" | grep -qiE "hit your (session|usage|weekly) limit|usage limit|rate_limit"; then
      reset=$(printf '%s' "$text" | grep -oiE "resets [0-9:apm]+ ?(\([^)]*\))?" | head -1)
      if [ -x "$OPENAGENTS" ]; then
        if [ -n "$reset" ]; then "$OPENAGENTS" capacity record claude --reset "$reset" >/dev/null 2>&1
        else "$OPENAGENTS" capacity record claude >/dev/null 2>&1; fi
      fi
      printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%MZ)" "${reset:-usage limit}" >"$DIR/parked"
      rm -f "$DIR/active/"* 2>/dev/null
    fi
    exit 0
    ;;
  resume)
    rm -f "$DIR/parked"
    echo "agent-gate: resumed; the capacity book still applies."
    ;;
  status)
    echo "running agents: $(active_count) of $MAX"
    [ -f "$DIR/parked" ] && echo "parked: $(cat "$DIR/parked") (resume: scripts/dev/agent-gate.sh resume)"
    [ -x "$OPENAGENTS" ] && "$OPENAGENTS" capacity check claude 2>&1 | sed 's/^/capacity: /'
    ;;
  *)
    echo "usage: agent-gate.sh pre|start|stop|limit|status|resume" >&2
    exit 2
    ;;
esac
