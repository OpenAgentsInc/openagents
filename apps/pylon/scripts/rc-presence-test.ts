// Throwaway RC test helper: registers ONE pylon (identity from PYLON_HOME) under
// the agent token in OPENAGENTS_AGENT_TOKEN and heartbeats it online every 20s,
// so the live homepage network viz shows more pylons. NOT committed for prod use;
// stop with the launcher's kill. Reuses the real presence client.
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { registerPylon, sendHeartbeat } from "../src/presence"

const baseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL ?? "https://openagents.com"
const agentToken = Bun.env.OPENAGENTS_AGENT_TOKEN
const name = Bun.env.RC_PYLON_NAME ?? "rc-test"
if (!agentToken) {
  console.error(`[${name}] no OPENAGENTS_AGENT_TOKEN`)
  process.exit(1)
}
const summary = createBootstrapSummary(parseBootstrapArgs(["--json", "--display-name", name]), Bun.env)
const opts = { baseUrl, agentToken, env: Bun.env } as const

try {
  const reg = await registerPylon(summary, opts)
  console.log(`[${name}] registered ${reg.registrationRef ?? ""}`)
} catch (error) {
  console.error(`[${name}] register failed: ${error instanceof Error ? error.message : String(error)}`)
  process.exit(1)
}

const beat = async () => {
  try {
    await sendHeartbeat(summary, opts)
    console.log(`[${name}] heartbeat ${new Date().toISOString()}`)
  } catch (error) {
    console.error(`[${name}] heartbeat err: ${error instanceof Error ? error.message : String(error)}`)
  }
}
await beat()
setInterval(beat, 20_000)
await new Promise(() => {})
