import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { createOmegaEffectdService } from "../service.ts"
import { openFullAutoRunRegistry } from "../engine/full-auto-run-registry.ts"
import { resolveFullAutoRunsPath } from "../paths.ts"
import { createOmegaEffectdFramedServer } from "./server.ts"
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts"

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-framed-"))
  try {
    await fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  })

describe("omega-effectd framed protocol", () => {
  test("initialize, health, generation fence, and disk recovery", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })

      const init = await server.handleLine(request("1", 0, "initialize", { generation: 7 }))
      expect(init?.ok).toBe(true)
      expect((init?.result as { generation: number }).generation).toBe(7)

      const health = await server.handleLine(request("2", 7, "health"))
      expect(health?.ok).toBe(true)
      expect((health?.result as { status: string }).status).toBe("running")

      const stale = await server.handleLine(request("3", 6, "health"))
      expect(stale?.ok).toBe(false)
      expect(stale?.error?.code).toBe("stale_generation")

      const runsPath = resolveFullAutoRunsPath({ dataRoot: root })
      const registry = openFullAutoRunRegistry(runsPath)
      const created = registry.createDraft({
        title: "Recovery proof",
        objective: "Prove durable truth survives restart.",
        doneCondition: "Runs list still shows this runRef.",
        objectiveSource: "user",
        workspaceRef: "workspace.omega.supervised",
      })
      expect(created.runRef).toMatch(/^run\.full-auto\./)

      // New process-equivalent: new service + framed server on same data root.
      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } })
      const serverB = createOmegaEffectdFramedServer(serviceB, { dataRoot: root })
      await serverB.handleLine(request("10", 0, "initialize", { generation: 8 }))
      const listed = await serverB.handleLine(request("11", 8, "list_runs"))
      expect(listed?.ok).toBe(true)
      const runs = (listed?.result as { runs: Array<{ runRef: string; title: string }> }).runs
      expect(runs.some(run => run.runRef === created.runRef && run.title === "Recovery proof")).toBe(true)
      expect(JSON.stringify(listed)).not.toContain("Prove durable truth")
    })
  })

  test("start, get_run detail, pause, resume, and stop for FA-03 launcher", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-03 start",
          objective: "Start one run from the framed protocol.",
          doneCondition: "Run exists in get_run with running or paused state.",
          turnCap: 12,
        }),
      )
      expect(started?.ok).toBe(true)
      const run = (started?.result as { run: { runRef: string; objective: string; state: string } }).run
      expect(run.runRef).toMatch(/^run\.full-auto\./)
      expect(run.objective).toContain("framed protocol")

      const detail = await server.handleLine(request("3", 1, "get_run", { runRef: run.runRef }))
      expect(detail?.ok).toBe(true)
      expect((detail?.result as { run: { title: string } }).run.title).toBe("FA-03 start")

      const paused = await server.handleLine(request("4", 1, "pause", { runRef: run.runRef }))
      expect(paused?.ok).toBe(true)
      expect((paused?.result as { run: { state: string } }).run.state).toMatch(/paus/)

      const resumed = await server.handleLine(request("5", 1, "resume", { runRef: run.runRef }))
      expect(resumed?.ok).toBe(true)

      const stopped = await server.handleLine(request("6", 1, "stop", { runRef: run.runRef }))
      expect(stopped?.ok).toBe(true)
      expect((stopped?.result as { run: { state: string } }).run.state).toBe("stopped")

      // list_runs stays redacted (no objective text).
      const listed = await server.handleLine(request("7", 1, "list_runs"))
      expect(JSON.stringify(listed)).not.toContain("framed protocol")
    })
  })
})
