#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Bounded local-Claude real-task smoke (issue #4720). See
 * docs/claude-agent-task-smoke.md for the runbook.
 *
 * Live (default):    PYLON_AGENT_TOKEN=... pnpm run smoke:claude-agent-task
 *                      [--base-url https://openagents.com]
 * CI-safe (opt-out): pnpm run smoke:claude-agent-task -- --ci-safe
 */
import {
  runClaudeAgentTaskCiSmoke,
  runClaudeAgentTaskLiveSmoke,
} from "../src/claude-agent-task-smoke"

const args = process.argv.slice(2)
// Live execution is the DEFAULT; pass --ci-safe for the hermetic fixture path.
const live = !args.includes("--ci-safe")
const flag = (name: string, fallback?: string) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const result = live
  ? await runClaudeAgentTaskLiveSmoke({
      baseUrl: flag("--base-url", "https://openagents.com") as string,
      ...(Runtime.env.PYLON_AGENT_TOKEN === undefined
        ? {}
        : { agentToken: Runtime.env.PYLON_AGENT_TOKEN }),
    })
  : await runClaudeAgentTaskCiSmoke()

process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
process.exit(result.ok ? 0 : 1)
