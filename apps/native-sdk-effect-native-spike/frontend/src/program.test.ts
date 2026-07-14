import { IntentRef, resolveIntentRef, StaticPayload } from "@effect-native/core"
import { Effect, Exit, SubscriptionRef } from "@effect-native/core/effect"
import { describe, expect, test } from "vite-plus/test"

import { it } from "./effect-test.ts"
import {
  bridgePayloadLimit,
  decodeNativeIntent,
  projectNativeState,
  resolveNativeDispatch,
} from "./native-bridge.ts"
import { adoptionCounts, nativeSdkComponentAdoption } from "./native-sdk-component-adoption.ts"
import { fixtureSessions, initialSpikeState, makeSpikeRuntime, spikeView } from "./program.ts"
import {
  assertNativeProductionCommandBindings,
  nativeProductionCommandBindings,
  resolveNativeDeferredCommand,
} from "./production-command-parity.ts"
import {
  persistSpikeState,
  restoreSpikeState,
  spikeStorageKey,
  spikeStorageNamespace,
  type SpikeStorage,
} from "./state-storage.ts"

const memoryStorage = (): SpikeStorage & { readonly values: Map<string, string> } => {
  const values = new Map<string, string>()
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value) },
  }
}

const deferredNewChat = (delivery: "dispatch" | "duplicate_rejected" = "dispatch") => ({
  schema: "openagents.desktop.deferred_command.v1",
  requestRef: "command.native-sdk.menu.4",
  commandId: "chat.new",
  arguments: { kind: "none" },
  source: "native_menu",
  delivery,
})

describe("Native SDK Effect Native parity spike", () => {
  it.effect("runs the real Desktop composer through the production Effect intent loop", () =>
    Effect.gen(function* () {
      const runtime = yield* makeSpikeRuntime()
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopInputChanged", StaticPayload("Ship the parity slice")), null))
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNoteSubmitted"), null))
      const state = yield* SubscriptionRef.get(runtime.state)
      const events = yield* runtime.registry.events

      expect(state.notes.at(-2)?.text).toBe("Ship the parity slice")
      expect(state.notes.at(-1)?.text).toContain("bounded Native host adapter")
      expect(state.pending).toBe(false)
      expect(state.input).toBe("")
      expect(events).toHaveLength(2)
      expect(events.every((event) => Exit.isSuccess(event.result))).toBe(true)
    }),
  )

  it.effect("uses the production Desktop new-chat and workspace handlers", () =>
    Effect.gen(function* () {
      const runtime = yield* makeSpikeRuntime()
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNoteSubmitted", StaticPayload("   ")), null))
      let state = yield* SubscriptionRef.get(runtime.state)
      expect(state.notes).toHaveLength(2)
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopWorkspaceSelected", StaticPayload("settings")), null))
      state = yield* SubscriptionRef.get(runtime.state)
      expect(state.workspace).toBe("settings")
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNewChat"), null))
      state = yield* SubscriptionRef.get(runtime.state)
      expect(state.activeThreadId).toMatch(/^native\.fixture\.new\./u)
      expect(state.notes).toEqual([])
      expect(state.workspace).toBe("chat")
    }),
  )

  test("projects the real Desktop center pane without a duplicate sidebar", () => {
    const encoded = JSON.stringify(spikeView(initialSpikeState()))

    expect(encoded.startsWith('{"_tag":"Stack"')).toBe(true)
    expect(encoded).toContain('"key":"shell-main"')
    expect(encoded).toContain('"key":"shell-transcript"')
    expect(encoded).toContain('"key":"shell-input"')
    expect(encoded).not.toContain('"key":"shell-sidebar"')
    expect(encoded).not.toContain('"key":"spike-')
  })

  test("decodes and resolves the real Native menu command through production contracts", () => {
    const envelope = decodeNativeIntent({
      protocol: 1,
      sequence: 4,
      intent: { _tag: "DeferredCommand", command: deferredNewChat() },
    })
    expect(resolveNativeDispatch(envelope)).toEqual({
      intentName: "DesktopNewChat",
      payload: null,
      appliedCommand: {
        commandId: "chat.new",
        intentName: "DesktopNewChat",
        source: "native_menu",
      },
    })
    expect(() => resolveNativeDeferredCommand({
      ...deferredNewChat(),
      arguments: { kind: "workspace", workspace: "home" },
    })).toThrow("argument_mismatch")
    expect(() => resolveNativeDeferredCommand(deferredNewChat("duplicate_rejected"))).toThrow("duplicate")
    expect(() => resolveNativeDeferredCommand({ ...deferredNewChat(), extra: true })).toThrow()
  })

  test("decodes only the bounded versioned native protocol", () => {
    expect(decodeNativeIntent({ protocol: 1, sequence: 4, intent: { _tag: "SessionSelected", sessionRef: fixtureSessions[1].ref, commandId: null } }).sequence).toBe(4)
    expect(() => decodeNativeIntent({ protocol: 2, sequence: 4, intent: { _tag: "NewChatRequested", commandId: "chat.new" } })).toThrow()
    expect(() => decodeNativeIntent({ protocol: 1, sequence: 4, intent: { _tag: "NewChatRequested", commandId: "settings.open" } })).toThrow()
    expect(() => decodeNativeIntent({ protocol: 1, sequence: 0, intent: { _tag: "NewChatRequested", commandId: "chat.new" } })).toThrow()
    expect(decodeNativeIntent({
      protocol: 1,
      sequence: 5,
      intent: { _tag: "RendererReloadRequested", commandId: "openagents.spike.reload-effect" },
    }).intent._tag).toBe("RendererReloadRequested")
  })

  test("routes Native Settings through the production workspace intent", () => {
    const envelope = decodeNativeIntent({
      protocol: 1,
      sequence: 2,
      intent: { _tag: "WorkspaceSelected", workspace: "settings", commandId: "settings.open" },
    })
    expect(resolveNativeDispatch(envelope)).toMatchObject({
      intentName: "DesktopWorkspaceSelected",
      payload: "settings",
    })
  })

  test("consumes the real Desktop canonical command identities", () => {
    expect(assertNativeProductionCommandBindings()).toBeUndefined()
    expect(nativeProductionCommandBindings.map((binding) => binding.commandId)).toEqual([
      "chat.new",
      "chat.open",
      "workspace.home",
      "settings.open",
    ])
  })

  test("keeps the native mirror projection small and non-authoritative", () => {
    const state = initialSpikeState()
    const projection = projectNativeState(state, 7, {
      sequence: 4,
      commandId: "chat.new",
      intentName: "DesktopNewChat",
      source: "native_menu",
    })
    expect(projection.selectedSessionRef).toBe(state.activeThreadId)
    expect(projection.messageCount).toBe(2)
    expect(projection.lastAppliedCommand?.intentName).toBe("DesktopNewChat")
    expect(new TextEncoder().encode(JSON.stringify(projection)).length).toBeLessThan(bridgePayloadLimit)
    expect(fixtureSessions.length).toBeLessThanOrEqual(3)
  })

  test("keeps the component-adoption audit explicit and unique", () => {
    const effectTags = nativeSdkComponentAdoption.map((entry) => entry.effectNative)
    expect(new Set(effectTags).size).toBe(effectTags.length)
    expect(adoptionCounts.direct).toBeGreaterThanOrEqual(8)
    expect(nativeSdkComponentAdoption.find((entry) => entry.effectNative === "CodeEditor")?.lane).toBe("unsupported")
    expect(nativeSdkComponentAdoption.find((entry) => entry.effectNative === "Host(webview)")?.lane).toBe("host-only")
  })

  test("restores only the bounded Native rail projection with a monotonic revision", () => {
    const storage = memoryStorage()
    const state = initialSpikeState({ revision: 7, acknowledgedNativeSequence: 3, workspace: "home", selectedSessionRef: fixtureSessions[1].ref })
    expect(persistSpikeState(storage, "run-1", state, 7, 3)).toBe(true)
    expect(restoreSpikeState(storage, "run-1")).toEqual({
      revision: 8,
      acknowledgedNativeSequence: 3,
      workspace: "home",
      selectedSessionRef: fixtureSessions[1].ref,
    })
    storage.values.set(spikeStorageKey("run-2"), "not-json")
    expect(restoreSpikeState(storage, "run-2")).toEqual({
      revision: 1,
      acknowledgedNativeSequence: 0,
      workspace: "chat",
      selectedSessionRef: fixtureSessions[0].ref,
    })
    expect(spikeStorageNamespace("zero://app/index.html#assurance-run=proof.42")).toBe("proof.42")
    expect(spikeStorageNamespace("zero://app/index.html?assurance-run=../../unsafe")).toBe("default")
  })
})
