/**
 * OMEGA-AC-03 live proof driver: one Agent Computer turn via omega-effectd.
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

const defaultObjective =
  "Create docs/qa/omega-ac03-agent-computer-qualification.md with exactly three short lines: a heading naming OMEGA-AC-03, a line saying the change was made by an Omega-dispatched Agent Computer turn, and a line saying the shared OpenAgents capacity authority executed it. Stage only that file with git add. Do not commit or push; the Agent Computer runtime owns verification and writeback."

export const runAc03LiveProof = async (bearerTokenInput: string): Promise<number> => {
  const bearerToken = bearerTokenInput.trim()
  if (bearerToken.length === 0) {
    console.error(JSON.stringify({ ok: false, error: "native OpenAgents session missing" }))
    return 2
  }

  const controlPlaneBaseUrl =
    process.env.OPENAGENTS_CONTROL_PLANE_BASE_URL?.trim() || "https://openagents.com"
  const repoRef = process.env.OPENAGENTS_AC03_REPO_REF?.trim() || "OpenAgentsInc/openagents"
  const objective = process.env.OPENAGENTS_AC03_OBJECTIVE?.trim() || defaultObjective
  const adapter = (process.env.OPENAGENTS_AC03_ADAPTER?.trim() || "codex") as
    | "codex"
    | "claude_agent"
  const verify = ["git", "diff", "--cached", "--check"] as const
  const pollIntervalMs = Number(process.env.OPENAGENTS_AC03_POLL_MS ?? "2000")
  const maxPollAttempts = Number(process.env.OPENAGENTS_AC03_MAX_POLLS ?? "300")
  const root = mkdtempSync(path.join(tmpdir(), "oa-ac03-proof-"))
  const store = openAgentComputerSessionStore(
    resolveAgentComputerSessionsPath({ dataRoot: root }),
  )

  try {
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
            session: outcome.session,
          }
        : {
            error: { code: outcome.code, message: outcome.message },
          }),
      durableDiskContainsBearer: disk.includes(bearerToken),
      durableDiskContainsObjective: disk.includes(objective),
    }

    const outPath = path.resolve(
      import.meta.dirname,
      "../../../docs/omega/2026-07-24-omega-ac03-live-proof.json",
    )
    writeFileSync(outPath, `${JSON.stringify(publicSafe, null, 2)}\n`)
    console.log(
      JSON.stringify({
        ok: publicSafe.ok,
        outPath,
        sessionRef: outcome.ok ? outcome.session.sessionRef : null,
        state: outcome.ok ? outcome.session.state : null,
        code: outcome.ok ? null : outcome.code,
      }),
    )
    return publicSafe.ok ? 0 : 1
  } catch (error) {
    console.error(
      JSON.stringify({
        ok: false,
        error: error instanceof Error ? error.message : "ac03 proof failed",
      }),
    )
    return 1
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

if (import.meta.main) {
  const exitCode = await runAc03LiveProof(process.env.OPENAGENTS_AGENT_TOKEN ?? "")
  process.exit(exitCode)
}
