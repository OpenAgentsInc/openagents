import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import os from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import type { FullAutoRunActionContext } from "./full-auto-run-actions.ts"
import { createOmegaFullAutoSync, projectOmegaFullAutoRunForSync } from "./full-auto-sync.ts"
import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts"

const withRoot = async (run: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(os.tmpdir(), "omega-effectd-sync-"))
  try {
    await run(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const unknownRunContext = (onGet: () => void): FullAutoRunActionContext =>
  ({
    capabilities: {
      runRegistry: {
        get: () => {
          onGet()
          return null
        },
      },
    },
    now: () => new Date("2026-07-24T12:00:00.000Z"),
    actor: "mobile",
    callerLabel: "mobile control intent",
  }) as unknown as FullAutoRunActionContext

const pendingIntent = {
  schema: "full_auto_run.control_intent.v1",
  intentId: "intent.mobile.pause.1",
  idempotencyKey: "intent.mobile.pause.1",
  runRef: "run.missing",
  action: "pause",
  surface: "mobile",
  createdAt: "2026-07-24T12:00:00.000Z",
  status: "pending",
  appliedAt: null,
  rejectionReason: null,
  resultLifecycleState: null,
} as const

describe("omega Full Auto Sync", () => {
  test("projects only the bounded mobile schema and removes the raw workspace path", async () => {
    await withRoot(async (root) => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"), () =>
        new Date("2026-07-24T12:00:00.000Z"),
      )
      const result = registry.startNew({
        title: "Sync projection",
        objective: "Implement the owner-visible task",
        doneCondition: "The owner-visible checks pass",
        objectiveSource: "user",
        workspaceRef: "/Users/owner/private/project",
        threadRef: "thread.1",
        profile: { lane: "codex-local" },
        actor: "owner_ui",
        reason: "Owner started the run.",
      })
      expect(result.ok).toBe(true)
      if (!result.ok) return

      const projection = projectOmegaFullAutoRunForSync(result.run)
      expect(projection?.workspaceLabel).toBe("project")
      const serialized = JSON.stringify(projection)
      expect(serialized).not.toContain("/Users/owner")
      expect(serialized).not.toContain("objectiveHistory")
      expect(serialized).not.toContain("executionHistory")
      expect(serialized).not.toContain("accessToken")
    })
  })

  test("publishes the shared projection without credential or local path material", async () => {
    await withRoot(async (root) => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"), () =>
        new Date("2026-07-24T12:00:00.000Z"),
      )
      const started = registry.startNew({
        title: "Published projection",
        objective: "Visible mobile objective",
        doneCondition: "Visible mobile completion condition",
        objectiveSource: "user",
        workspaceRef: "/private/workspaces/omega",
        threadRef: "thread.publish",
        profile: { lane: "codex-local" },
        actor: "owner_ui",
        reason: "Owner started the run.",
      })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      let publishedBody = ""
      const sync = createOmegaFullAutoSync({
        resolveSession: async () => ({
          baseUrl: "https://openagents.example",
          accessToken: "secret-runtime-bearer",
        }),
        actionContext: () => unknownRunContext(() => undefined),
        outcomesPath: path.join(root, "sync-outcomes.json"),
        fetchImpl: async (_request, init) => {
          publishedBody = String(init?.body ?? "")
          return Response.json({
            ok: true,
            projection: {
              schema: "full_auto_run.mobile_projection.v1",
              privateMaterialExcluded: true,
              generatedAt: "2026-07-24T12:00:01.000Z",
              run: JSON.parse(publishedBody).run,
            },
          })
        },
      })
      expect(await sync.publish(started.run)).toBe("published")
      expect(publishedBody).toContain('"workspaceLabel":"omega"')
      expect(publishedBody).not.toContain("/private/workspaces")
      expect(publishedBody).not.toContain("secret-runtime-bearer")
    })
  })

  test("persists an outcome before reporting and replays it after a lost ack and restart", async () => {
    await withRoot(async (root) => {
      const outcomesPath = path.join(root, "sync-outcomes.json")
      let actionApplications = 0
      let outcomeReports = 0
      const fetchImpl = async (request: URL | RequestInfo, init?: RequestInit): Promise<Response> => {
        const url = new URL(String(request))
        if (url.pathname.endsWith("/control-intents") && init?.method === "GET") {
          return Response.json({ ok: true, intents: [pendingIntent] })
        }
        if (url.pathname.endsWith("/control-intents") && init?.method === "POST") {
          outcomeReports += 1
          if (outcomeReports === 1) return new Response("lost", { status: 503 })
          return Response.json({
            ok: true,
            intent: {
              ...pendingIntent,
              status: "rejected",
              appliedAt: "2026-07-24T12:00:01.000Z",
              rejectionReason: "run_not_found",
            },
          })
        }
        return new Response("unexpected", { status: 500 })
      }
      const dependencies = {
        resolveSession: async () => ({ baseUrl: "https://openagents.example", accessToken: "runtime-only" }),
        actionContext: () => unknownRunContext(() => {
          actionApplications += 1
        }),
        outcomesPath,
        fetchImpl,
      }

      await createOmegaFullAutoSync(dependencies).tick([])
      expect(actionApplications).toBe(1)
      expect(readFileSync(outcomesPath, "utf8")).not.toContain("runtime-only")

      await createOmegaFullAutoSync(dependencies).tick([])
      expect(actionApplications).toBe(1)
      expect(outcomeReports).toBe(2)
    })
  })

  test("keeps local dispatch independent when the host has no admitted session", async () => {
    await withRoot(async (root) => {
      const sync = createOmegaFullAutoSync({
        resolveSession: async () => null,
        actionContext: () => unknownRunContext(() => undefined),
        outcomesPath: path.join(root, "sync-outcomes.json"),
      })
      await sync.tick([])
      expect(sync.status()).toEqual({
        available: false,
        publishBlocksDispatch: false,
        reason: "omega_khala_sync_session_unavailable",
      })
    })
  })
})
