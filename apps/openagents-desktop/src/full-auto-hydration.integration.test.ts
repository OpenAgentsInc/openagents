import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { IntentRef, StaticPayload, resolveIntentRef } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"
import { describe, expect, test } from "vite-plus/test"

import { startFullAutoControlServer } from "./full-auto-control-server.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import {
  activeFullAutoEnabled,
  desktopShellIntents,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  type DesktopShellState,
} from "./renderer/shell.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

describe("Full Auto control enable -> later renderer hydration (#8928)", () => {
  test("a real control-server enable survives registry-backed renderer attachment until an explicit toggle", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-hydration-integration-"))
    const workspaceRef = path.join(root, "workspace")
    const thread = {
      id: "thread.control-enabled-before-window",
      title: "Control-enabled",
      updatedAt: "2026-07-16T16:00:00.000Z",
      notes: [],
    } as const
    const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
    const server = await startFullAutoControlServer({
      capabilities: {
        registry,
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

      const setCalls: Array<Readonly<{ threadRef: string; enabled: boolean }>> = []
      const fullAutoHost = {
        // Same durable read/write semantics main's IPC handlers expose. This
        // composed test deliberately stops short of booting Electron/preload;
        // the real-window attachment remains an explicit higher-rung gap.
        get: async (input: { threadRef: string }) => ({ enabled: registry.get(input.threadRef) }),
        set: async (input: { threadRef: string; enabled: boolean }) => {
          setCalls.push(input)
          registry.set(
            input.threadRef,
            input.enabled,
            input.enabled
              ? { workspaceRef }
              : { disabledBy: "ui_toggle" },
          )
          return { ok: true }
        },
      }
      const state = await Effect.runPromise(SubscriptionRef.make<DesktopShellState>({
        ...initialDesktopShellState("electron/darwin", "11:00"),
        harnessLanes: {
          fable: { available: true, reason: null },
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
      expect(setCalls).toEqual([])
      expect(registry.get(thread.id)).toBe(true)

      await Effect.runPromise(intents.dispatch(resolveIntentRef(
        IntentRef("DesktopFullAutoToggled", StaticPayload(null)),
      )))
      expect(setCalls).toEqual([{ threadRef: thread.id, enabled: false }])
      expect(registry.record(thread.id)).toMatchObject({
        enabled: false,
        disabledBy: "ui_toggle",
      })
    } finally {
      await server.stop()
      rmSync(root, { recursive: true, force: true })
    }
  })
})
