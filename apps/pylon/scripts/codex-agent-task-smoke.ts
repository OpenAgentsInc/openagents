#!/usr/bin/env bun
/**
 * Bounded local-Codex real-task smoke (issue #4790, epic #4793). See
 * docs/codex-agent-task-smoke.md for the runbook.
 *
 * CI-safe (default): bun run smoke:codex-agent-task
 * Live:              PYLON_AGENT_TOKEN=... bun scripts/codex-agent-task-smoke.ts --live \
 *                      [--base-url https://openagents.com]
 */
import {
  runCodexAgentTaskCiSmoke,
  runCodexAgentTaskLiveSmoke,
} from "../src/codex-agent-task-smoke"

const args = process.argv.slice(2)
const live = args.includes("--live")
const flag = (name: string, fallback?: string) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const result = live
  ? await runCodexAgentTaskLiveSmoke({
      baseUrl: flag("--base-url", "https://openagents.com") as string,
      ...(Bun.env.PYLON_AGENT_TOKEN === undefined
        ? {}
        : { agentToken: Bun.env.PYLON_AGENT_TOKEN }),
    })
  : await runCodexAgentTaskCiSmoke()

process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
process.exit(result.ok ? 0 : 1)
