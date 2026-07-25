import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { IntentRef, StaticPayload, resolveIntentRef } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"
import { describe, expect, test } from "vite-plus/test"

import { startFullAutoControlServer } from "./full-auto-control-server.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts"
import { openFullAutoRunReportStore } from "./full-auto-run-report.ts"
import {
  activeFullAutoEnabled,
  desktopShellIntents,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  type DesktopShellState,
} from "./renderer/shell.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

describe("Full Auto control enable -> later renderer hydration (#8928)", () => {
  test("a real control-server enable survives registry-backed renderer attachment, and the renderer cannot write it back", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-hydration-integration-"))
    const workspaceRef = path.join(root, "workspace")
    const thread = {
      id: "thread.control-enabled-before-window",
      title: "Control-enabled",
      updatedAt: "2026-07-16T16:00:00.000Z",
      notes: [],
    } as const
    const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
    const runRegistry = openFullAutoRunRegistry(path.join(root, "full-auto", "runs.json"))
    const reportStore = openFullAutoRunReportStore(path.join(root, "full-auto", "reports.json"))
    const server = await startFullAutoControlServer({
      capabilities: {
        registry,
        runRegistry,
        reportStore,
        resolveWorkspaceRef: () => workspaceRef,
        triggerReconciliation: async () => {},
        liveState: () => null,
        listTurns: () => [],
        appendSystemNote: () => {},
        createThread: () => thread.id,
      },
      controlFilePath: path.join(root, "full-auto", "control.json"),
    })

    try {
      // The actual bearer-gated HTTP surface enables the actual durable
      // registry while no renderer state or handler exists yet.
      const enabled = await fetch(`${server.url}/v1/full-auto/${thread.id}/enable`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${server.credential.token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ workspaceRef }),
      })
      expect(enabled.status).toBe(200)
      expect(registry.get(thread.id)).toBe(true)

      // Same durable READ semantics main's IPC handler exposes. The
      // authority-granting `set` companion was removed on 2026-07-25
      // (omega#26, gate 8 restated), so the renderer surface is read-only and
      // this composed test can no longer write through it. This test
      // deliberately stops short of booting Electron/preload; the real-window
      // attachment remains an explicit higher-rung gap.
      const fullAutoHost = {
        get: async (input: { threadRef: string }) => ({ enabled: registry.get(input.threadRef) }),
      }
      const state = await Effect.runPromise(SubscriptionRef.make<DesktopShellState>({
        ...initialDesktopShellState("electron/darwin", "11:00"),
        harnessLanes: {
          claude: { available: true, reason: null },
          codex: { available: true, reason: null },
        },
        threads: [thread],
        activeThreadId: null,
        fullAutoByThread: {},
      }))
      const chatHost = {
        listThreads: async () => [thread],
        newThread: async () => null,
        openThread: async (id: string) => id === thread.id ? thread : null,
        sendMessage: async () => ({ ok: false as const, error: "unused" }),
      }
      const args: Parameters<typeof makeDesktopShellHandlers> = [
        state, () => "11:00", undefined, chatHost,
        undefined, undefined, undefined, undefined, undefined, undefined,
        undefined, undefined, undefined, undefined, undefined, undefined,
        undefined, undefined, undefined, undefined, undefined, undefined,
        undefined, undefined, undefined, undefined, undefined, fullAutoHost,
      ]
      const intents = await Effect.runPromise(makeIntentRegistry(
        desktopShellIntents,
        makeDesktopShellHandlers(...args),
      ))

      await Effect.runPromise(intents.dispatch(resolveIntentRef(
        IntentRef("DesktopChatSelected", StaticPayload(thread.id)),
      )))
      expect(activeFullAutoEnabled(await Effect.runPromise(SubscriptionRef.get(state)))).toBe(true)
      // Attachment is read-only: hydration never writes the renderer's initial
      // false default over main's API-enabled row, and there is no renderer
      // path that could.
      expect(registry.get(thread.id)).toBe(true)
      expect(Object.keys(fullAutoHost)).toEqual(["get"])
      expect(desktopShellIntents.map(intent => intent.name)).not.toContain("DesktopFullAutoToggled")
    } finally {
      await server.stop()
      rmSync(root, { recursive: true, force: true })
    }
  })
})
