#!/usr/bin/env bun
/**
 * Bounded local-Codex real-task smoke (issue #4790, epic #4793). See
 * docs/codex-agent-task-smoke.md for the runbook.
 *
 * Live (default):    PYLON_AGENT_TOKEN=... bun run smoke:codex-agent-task
 *                      [--base-url https://openagents.com]
 * CI-safe (opt-out): bun run smoke:codex-agent-task -- --ci-safe
 */
import {
  runCodexAgentTaskCiSmoke,
  runCodexAgentTaskLiveSmoke,
} from "../src/codex-agent-task-smoke"

const args = process.argv.slice(2)
// Live execution is the DEFAULT; pass --ci-safe for the hermetic fixture path.
const live = !args.includes("--ci-safe")
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
