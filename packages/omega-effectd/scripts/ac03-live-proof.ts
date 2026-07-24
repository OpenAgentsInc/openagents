/**
 * OMEGA-AC-03 live proof driver: one Agent Computer turn via omega-effectd engine.
 * Prints only public-safe fields. Never logs the bearer token or raw objective.
 */
import { createHash } from "node:crypto"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  openAgentComputerSessionStore,
  runAgentComputerTurn,
} from "../src/engine/agent-computer-sessions.ts"
import { resolveAgentComputerSessionsPath } from "../src/paths.ts"

const bearerToken = process.env.OPENAGENTS_AGENT_TOKEN?.trim() ?? ""
if (bearerToken.length === 0) {
  console.error(JSON.stringify({ ok: false, error: "OPENAGENTS_AGENT_TOKEN missing" }))
  process.exit(2)
}

const controlPlaneBaseUrl =
  process.env.OPENAGENTS_CONTROL_PLANE_BASE_URL?.trim() || "https://openagents.com"
const repoRef = process.env.OPENAGENTS_AC03_REPO_REF?.trim() || "OpenAgentsInc/openagents"
const objective =
  process.env.OPENAGENTS_AC03_OBJECTIVE?.trim() ||
  "OMEGA-AC-03 public-safe proof: run only `git rev-parse HEAD` and exit 0. Do not write files. Do not touch secrets."
const adapter = (process.env.OPENAGENTS_AC03_ADAPTER?.trim() || "claude_agent") as
  | "codex"
  | "claude_agent"
const verify = ["git", "rev-parse", "HEAD"] as const
const pollIntervalMs = Number(process.env.OPENAGENTS_AC03_POLL_MS ?? "2000")
const maxPollAttempts = Number(process.env.OPENAGENTS_AC03_MAX_POLLS ?? "300")

const root = mkdtempSync(path.join(tmpdir(), "oa-ac03-proof-"))
const store = openAgentComputerSessionStore(
  resolveAgentComputerSessionsPath({ dataRoot: root }),
)

const main = async () => {
  const startedAt = new Date().toISOString()
  const outcome = await runAgentComputerTurn(store, {
    bearerToken,
    controlPlaneBaseUrl,
    repoRef,
    objective,
    adapter,
    lane: "cloud-gcp",
    verify: [...verify],
    pollIntervalMs,
    maxPollAttempts,
  })
  const finishedAt = new Date().toISOString()
  const sessionsPath = resolveAgentComputerSessionsPath({ dataRoot: root })
  let disk = ""
  try {
    disk = readFileSync(sessionsPath, "utf8")
  } catch {
    disk = ""
  }
  const publicSafe = {
    ok: outcome.ok,
    packet: "OMEGA-AC-03",
    path: "omega-effectd runAgentComputerTurn -> HarnessEnvironment.openagents_cloud",
    controlPlaneBaseUrl,
    repoRef,
    adapter,
    verify,
    pollIntervalMs,
    maxPollAttempts,
    objectiveDigest: createHash("sha256").update(objective).digest("hex"),
    startedAt,
    finishedAt,
    ...(outcome.ok
      ? {
          finishReason: outcome.finishReason,
          eventKinds: outcome.eventKinds,
          session: {
            sessionRef: outcome.session.sessionRef,
            state: outcome.session.state,
            environment: outcome.session.environment,
            placementRef: outcome.session.placementRef,
            artifactRef: outcome.session.artifactRef,
            agentComputerRef: outcome.session.agentComputerRef,
            agentComputerState: outcome.session.agentComputerState,
            lane: outcome.session.lane,
            adapter: outcome.session.adapter,
          },
        }
      : {
          error: { code: outcome.code, message: outcome.message },
        }),
    durableDiskContainsBearer: disk.includes(bearerToken),
    durableDiskContainsObjective: disk.includes(objective),
  }

  const outPath = path.resolve(
    process.cwd(),
    "../../docs/omega/2026-07-24-omega-ac03-live-proof.json",
  )
  writeFileSync(outPath, `${JSON.stringify(publicSafe, null, 2)}\n`)
  console.log(
    JSON.stringify({
      ok: publicSafe.ok,
      outPath,
      sessionRef:
        outcome.ok && "session" in outcome ? outcome.session.sessionRef : null,
      state: outcome.ok && "session" in outcome ? outcome.session.state : null,
      code: outcome.ok ? null : outcome.code,
    }),
  )
  rmSync(root, { recursive: true, force: true })
  process.exit(publicSafe.ok ? 0 : 1)
}

main().catch(error => {
  console.error(
    JSON.stringify({
      ok: false,
      error: error instanceof Error ? error.message : "ac03 proof failed",
    }),
  )
  rmSync(root, { recursive: true, force: true })
  process.exit(1)
})
