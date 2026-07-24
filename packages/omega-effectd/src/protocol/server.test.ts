import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { createOmegaEffectdService } from "../service.ts"
import { openFullAutoRunRegistry } from "../engine/full-auto-run-registry.ts"
import { resolveFullAutoRegistryPath, resolveFullAutoRunsPath } from "../paths.ts"
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

  test("FA-04 capacity, guardrail immunity, missing-thread stall, and redacted attention", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      const init = await server.handleLine(request("1", 0, "initialize", { generation: 1 }))
      expect(init?.ok).toBe(true)
      expect((init?.result as { capabilities: string[] }).capabilities).toContain("get_capacity")
      expect((init?.result as { capabilities: string[] }).capabilities).toContain("decide_attention")

      const capacity = await server.handleLine(request("2", 1, "get_capacity"))
      expect(capacity?.ok).toBe(true)
      const cap = capacity?.result as {
        activeRunLimit: number
        nonOverridableGuardrails: string[]
        ownerConfigurableGuardrails: string[]
        enabledThreadsNeverEvicted: boolean
        lanes: Array<{ lane: string; state: string }>
      }
      expect(cap.activeRunLimit).toBe(8)
      expect(cap.nonOverridableGuardrails).toEqual([
        "workspace_binding",
        "own_capacity_only",
        "no_rate_limit_reset_triggering",
      ])
      expect(cap.ownerConfigurableGuardrails).toEqual([
        "maxWallClockMs",
        "maxTurns",
        "maxPerTurnFailures",
        "tokenBudgetRef",
      ])
      expect(cap.enabledThreadsNeverEvicted).toBe(true)
      expect(cap.lanes.length).toBeGreaterThan(0)
      expect(cap.lanes.some(lane => lane.lane === "codex-local")).toBe(true)

      const started = await server.handleLine(
        request("3", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-04 missing thread",
          objective: "SECRET_OBJECTIVE_SHOULD_NOT_LEAK_INTO_ATTENTION",
          doneCondition: "SECRET_DONE_CONDITION",
          turnCap: 8,
          guardrails: {
            maxTurns: 8,
            workspace_binding: false,
            own_capacity_only: false,
            no_rate_limit_reset_triggering: true,
          },
        }),
      )
      expect(started?.ok).toBe(true)
      const run = (started?.result as { run: { runRef: string; threadRef: string; state: string } }).run
      expect(run.threadRef).toMatch(/^thread\.omega\./)

      // Drop the thread record on disk (simulates cache eviction / vanished host
      // thread). Enabled Full Auto records are never silently dropped by the
      // registry eviction policy — this is the falsifier class for FA-04.
      const registryPath = resolveFullAutoRegistryPath({ dataRoot: root })
      writeFileSync(
        registryPath,
        JSON.stringify({ schema: "openagents.desktop.full_auto_registry.v1", records: [] }, null, 2),
      )

      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } })
      const serverB = createOmegaEffectdFramedServer(serviceB, { dataRoot: root })
      await serverB.handleLine(request("10", 0, "initialize", { generation: 2 }))
      const detail = await serverB.handleLine(request("11", 2, "get_run", { runRef: run.runRef }))
      expect(detail?.ok).toBe(true)
      const stalled = (
        detail?.result as {
          run: { state: string; stallCause: string | null; recoveryAction: string; objective: string }
        }
      ).run
      expect(stalled.state).toBe("stalled")
      expect(stalled.stallCause).toBe("host_thread_missing")
      expect(stalled.recoveryAction).toBe("stop_only")
      expect(stalled.objective).toContain("SECRET_OBJECTIVE")

      const attention = await serverB.handleLine(
        request("12", 2, "decide_attention", {
          runRef: run.runRef,
          permissionGranted: true,
        }),
      )
      expect(attention?.ok).toBe(true)
      const note = (
        attention?.result as {
          attention: { notify: boolean; title: string; body: string; dedupKey: string } | null
        }
      ).attention
      expect(note).not.toBeNull()
      expect(note?.notify).toBe(true)
      expect(note?.title).toContain("stalled")
      expect(JSON.stringify(note)).not.toContain("SECRET_OBJECTIVE")
      expect(JSON.stringify(note)).not.toContain("SECRET_DONE")
    })
  })

  test("FA-05 report, receipt redaction, mobile intent, and Sync stub", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-05 report",
          objective: "PRIVATE_OBJECTIVE_TEXT_MUST_NOT_ENTER_RECEIPT",
          doneCondition: "PRIVATE_DONE_CONDITION",
          turnCap: 6,
        }),
      )
      expect(started?.ok).toBe(true)
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef

      const report = await server.handleLine(request("3", 1, "get_report", { runRef }))
      expect(report?.ok).toBe(true)
      expect((report?.result as { report: { runRef: string } }).report.runRef).toBe(runRef)

      const receipt = await server.handleLine(request("4", 1, "get_receipt", { runRef }))
      expect(receipt?.ok).toBe(true)
      const publicReceipt = (receipt?.result as { receipt: Record<string, unknown> }).receipt
      expect(typeof publicReceipt.objectiveDigest).toBe("string")
      expect(JSON.stringify(publicReceipt)).not.toContain("PRIVATE_OBJECTIVE")
      expect(JSON.stringify(publicReceipt)).not.toContain("PRIVATE_DONE")

      const paused = await server.handleLine(
        request("5", 1, "apply_control_intent", {
          intentId: "intent.pause.1",
          runRef,
          action: "pause",
        }),
      )
      expect(paused?.ok).toBe(true)
      expect((paused?.result as { outcome: { status: string } }).outcome.status).toBe("applied")

      const resumed = await server.handleLine(
        request("6", 1, "apply_control_intent", {
          intentId: "intent.resume.1",
          runRef,
          action: "resume",
        }),
      )
      expect((resumed?.result as { outcome: { status: string } }).outcome.status).toBe("applied")

      const sync = await server.handleLine(request("7", 1, "get_sync_status"))
      expect(sync?.ok).toBe(true)
      expect((sync?.result as { available: boolean; publishBlocksDispatch: boolean }).available).toBe(
        false,
      )
      expect(
        (sync?.result as { publishBlocksDispatch: boolean }).publishBlocksDispatch,
      ).toBe(false)

      const publish = await server.handleLine(request("8", 1, "publish_projection", { runRef }))
      expect(publish?.ok).toBe(true)
      expect((publish?.result as { status: string }).status).toBe("sync_unavailable")

      const stopped = await server.handleLine(
        request("9", 1, "apply_control_intent", {
          intentId: "intent.stop.1",
          runRef,
          action: "stop",
        }),
      )
      expect((stopped?.result as { outcome: { status: string; resultLifecycleState?: string } }).outcome.status).toBe(
        "applied",
      )
    })
  })
})
