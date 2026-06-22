import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  openAgentsDefaultInputProfile,
  parseOpenAgentsInputProfileOrDefault,
  type OpenAgentsInputProfile,
} from "@openagentsinc/input-bindings"

import { PersistInputProfile, PersistPreferences } from "../src/ui/commands"
import { initialRuntimeState } from "../src/ui/initial-state"
import {
  CapturedInputBinding,
  ChangedDefaultAdapter,
  ChangedDefaultLane,
  ChangedGatewayInferenceFallback,
  ChangedThemePreference,
  ResetAllInputBindings,
  ResetInputBinding,
  ResetInputBindingCategory,
  SettledPersistPreferences,
  StartedInputBindingCapture,
  ToggledNotificationPanel,
} from "../src/ui/message"
import { initialModel, Model } from "../src/ui/model"
import {
  INPUT_PROFILE_STORAGE_KEY,
  capturedKeyboardBindingFromKey,
  inputProfileWithBinding,
  loadInputProfile,
  saveInputProfile,
} from "../src/ui/input-profile-preferences"
import {
  PREFERENCES_STORAGE_KEY,
  defaultPreferences,
  loadPreferences,
  savePreferences,
  themeAttr,
} from "../src/ui/preferences"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

// #5472: functional Settings preferences — persistence + reducer + apply paths.
//
// The desktop test env has no DOM (preload.ts shims only window/rAF). For the
// roundtrip tests we install a tiny in-memory localStorage so the preferences
// module's storage path is exercised exactly as it is in the webview; tests that
// assert the no-DOM fallback simply leave it absent.

type Store = { store: Map<string, string> }

const installLocalStorage = (): Store => {
  const store = new Map<string, string>()
  ;(globalThis as { localStorage?: unknown }).localStorage = {
    getItem: (k: string) => (store.has(k) ? (store.get(k) as string) : null),
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear(),
    key: () => null,
    length: 0,
  }
  return { store }
}

const removeLocalStorage = (): void => {
  delete (globalThis as { localStorage?: unknown }).localStorage
}

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const localProfile = (
  bindings: OpenAgentsInputProfile["bindings"],
): OpenAgentsInputProfile => ({
  schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  profileId: "test-local",
  bindings,
})

describe("#5472 preferences persistence", () => {
  afterEach(() => removeLocalStorage())

  test("loadPreferences returns the dark/codex/auto defaults with NO DOM", () => {
    removeLocalStorage()
    expect(loadPreferences()).toEqual(defaultPreferences)
    expect(defaultPreferences).toEqual({
      theme: "dark",
      defaultAdapter: "codex",
      defaultLane: "auto",
      showNotificationPanel: true,
      // #5485: gateway fallback defaults to "auto" (the conversion-friendly path).
      gatewayInferenceFallback: "auto",
    })
  })

  test("save → load round-trips every field", () => {
    installLocalStorage()
    const chosen = {
      theme: "light" as const,
      defaultAdapter: "claude_agent" as const,
      defaultLane: "cloud-gcp" as const,
      showNotificationPanel: false,
      gatewayInferenceFallback: "off" as const,
    }
    savePreferences(chosen)
    expect(loadPreferences()).toEqual(chosen)
  })

  test("corrupt / partial stored blobs fall back to defaults (never throw)", () => {
    const { store } = installLocalStorage()
    store.set(PREFERENCES_STORAGE_KEY, "{ not json")
    expect(loadPreferences()).toEqual(defaultPreferences)
    store.set(PREFERENCES_STORAGE_KEY, JSON.stringify({ theme: "neon" }))
    expect(loadPreferences()).toEqual(defaultPreferences)
  })

  test("the persisted record is refs-only — no identity/home/token/account fields", () => {
    installLocalStorage()
    savePreferences(defaultPreferences)
    const raw = (globalThis as { localStorage: Storage }).localStorage.getItem(
      PREFERENCES_STORAGE_KEY,
    )
    const parsed = JSON.parse(raw ?? "{}") as Record<string, unknown>
    expect(Object.keys(parsed).sort()).toEqual([
      "defaultAdapter",
      "defaultLane",
      "gatewayInferenceFallback",
      "showNotificationPanel",
      "theme",
    ])
  })

  test("themeAttr maps the preference to its data-theme value", () => {
    expect(themeAttr("dark")).toBe("dark")
    expect(themeAttr("light")).toBe("light")
  })
})

describe("#5949 input profile persistence", () => {
  afterEach(() => removeLocalStorage())

  test("loadInputProfile defaults with no DOM and falls back on corruption", () => {
    removeLocalStorage()
    expect(loadInputProfile()).toBe(openAgentsDefaultInputProfile)

    const { store } = installLocalStorage()
    store.set(INPUT_PROFILE_STORAGE_KEY, "{ not json")
    expect(loadInputProfile()).toBe(openAgentsDefaultInputProfile)
    store.set(INPUT_PROFILE_STORAGE_KEY, JSON.stringify({ bad: "shape" }))
    expect(loadInputProfile()).toBe(openAgentsDefaultInputProfile)
  })

  test("input profile persistence uses its own versioned key", () => {
    const { store } = installLocalStorage()
    const profile = inputProfileWithBinding(
      openAgentsDefaultInputProfile,
      "movement.forward",
      0,
      { type: "keyboard_code", code: "KeyI" },
    )

    saveInputProfile(profile)

    expect(store.has(INPUT_PROFILE_STORAGE_KEY)).toBe(true)
    expect(store.has(PREFERENCES_STORAGE_KEY)).toBe(false)
    expect(loadInputProfile().bindings["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
      { type: "keyboard_code", code: "ArrowUp" },
    ])
  })

  test("capturedKeyboardBindingFromKey creates keyboard-code bindings", () => {
    expect(capturedKeyboardBindingFromKey("i", {
      altKey: false,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
    })).toEqual({ type: "keyboard_code", code: "KeyI" })
    expect(capturedKeyboardBindingFromKey("Tab", {
      altKey: false,
      ctrlKey: false,
      metaKey: false,
      shiftKey: true,
    })).toEqual({
      type: "keyboard_code",
      code: "Tab",
      modifiers: { shift: true },
    })
  })
})

describe("#5472 preferences reducer", () => {
  test("ChangedThemePreference updates the model + emits PersistPreferences", () => {
    const [next, commands] = update(initialModel, ChangedThemePreference({ theme: "light" }))
    expect(next.themePreference).toBe("light")
    expect(commands.map((c) => c.name)).toEqual(["PersistPreferences"])
  })

  test("ChangedDefaultAdapter ALSO seeds the live spawn adapter (takes effect now)", () => {
    const [next, commands] = update(
      initialModel,
      ChangedDefaultAdapter({ adapter: "claude_agent" }),
    )
    expect(next.defaultAdapter).toBe("claude_agent")
    // The whole point of the issue: spawn/composer/chat read spawnAdapter.
    expect(next.spawnAdapter).toBe("claude_agent")
    expect(commands.map((c) => c.name)).toEqual(["PersistPreferences"])
  })

  test("ChangedDefaultLane ALSO seeds the live spawn lane", () => {
    const [next] = update(initialModel, ChangedDefaultLane({ lane: "cloud-gcp" }))
    expect(next.defaultLane).toBe("cloud-gcp")
    expect(next.spawnLane).toBe("cloud-gcp")
  })

  test("ToggledNotificationPanel persists the in-app feed visibility", () => {
    const [off, commands] = update(initialModel, ToggledNotificationPanel({ show: false }))
    expect(off.showNotificationPanel).toBe(false)
    expect(commands.map((c) => c.name)).toEqual(["PersistPreferences"])
    const [on] = update(off, ToggledNotificationPanel({ show: true }))
    expect(on.showNotificationPanel).toBe(true)
  })

  test("ChangedGatewayInferenceFallback persists the routing intent (#5485)", () => {
    const [off, commands] = update(
      initialModel,
      ChangedGatewayInferenceFallback({ value: "off" }),
    )
    expect(off.gatewayInferenceFallback).toBe("off")
    expect(commands.map((c) => c.name)).toEqual(["PersistPreferences"])
    const [auto] = update(off, ChangedGatewayInferenceFallback({ value: "auto" }))
    expect(auto.gatewayInferenceFallback).toBe("auto")
  })

  test("the PersistPreferences command carries exactly the preference fields", () => {
    const next = Model.make({
      ...initialModel,
      themePreference: "light",
      defaultAdapter: "apple_fm",
      defaultLane: "local",
      showNotificationPanel: false,
      gatewayInferenceFallback: "off",
    })
    const [, commands] = update(next, ChangedThemePreference({ theme: "light" }))
    const persist = commands[0] as unknown as { args: Record<string, unknown> }
    expect(persist.args).toEqual({
      theme: "light",
      defaultAdapter: "apple_fm",
      defaultLane: "local",
      showNotificationPanel: false,
      gatewayInferenceFallback: "off",
    })
  })

  test("SettledPersistPreferences is a no-op (model unchanged, no further command)", () => {
    const before = Model.make({ ...initialModel, themePreference: "light" })
    const [after, commands] = update(before, SettledPersistPreferences())
    expect(after).toEqual(before)
    expect(commands).toEqual([])
  })

  test("running the PersistPreferences effect actually writes localStorage", () => {
    installLocalStorage()
    const command = PersistPreferences({
      theme: "light",
      defaultAdapter: "claude_agent",
      defaultLane: "cloud-shc",
      showNotificationPanel: false,
      gatewayInferenceFallback: "off",
    })
    const result = Effect.runSync(command.effect)
    expect(result._tag).toBe("SettledPersistPreferences")
    expect(loadPreferences()).toEqual({
      theme: "light",
      defaultAdapter: "claude_agent",
      defaultLane: "cloud-shc",
      showNotificationPanel: false,
      gatewayInferenceFallback: "off",
    })
    removeLocalStorage()
  })
})

describe("#5949 input profile reducer", () => {
  test("capture updates a movement binding and persists the active profile", () => {
    const [capturing] = update(
      initialModel,
      StartedInputBindingCapture({ actionId: "movement.forward", slot: 0 }),
    )
    expect(capturing.inputBindingCapture).toEqual({
      actionId: "movement.forward",
      slot: 0,
    })

    const [next, commands] = update(
      capturing,
      CapturedInputBinding({
        actionId: "movement.forward",
        slot: 0,
        binding: { type: "keyboard_code", code: "KeyI" },
      }),
    )

    expect(next.inputBindingCapture).toBe(null)
    expect(parseOpenAgentsInputProfileOrDefault(next.inputProfile).bindings[
      "movement.forward"
    ]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
      { type: "keyboard_code", code: "ArrowUp" },
    ])
    expect(commands.map((command) => command.name)).toEqual([
      "PersistInputProfile",
    ])
  })

  test("restore row, category, and all return to defaults", () => {
    const custom = localProfile({
      ...openAgentsDefaultInputProfile.bindings,
      "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
      "target.next": [{ type: "keyboard_code", code: "KeyE" }],
    })
    const model = Model.make({ ...initialModel, inputProfile: custom })

    const [rowReset] = update(
      model,
      ResetInputBinding({ actionId: "movement.forward" }),
    )
    expect(parseOpenAgentsInputProfileOrDefault(rowReset.inputProfile).bindings[
      "movement.forward"
    ]).toEqual(openAgentsDefaultInputProfile.bindings["movement.forward"])

    const [categoryReset] = update(
      Model.make({ ...initialModel, inputProfile: custom }),
      ResetInputBindingCategory({ category: "Movement" }),
    )
    const categoryProfile = parseOpenAgentsInputProfileOrDefault(
      categoryReset.inputProfile,
    )
    expect(categoryProfile.bindings["movement.forward"]).toEqual(
      openAgentsDefaultInputProfile.bindings["movement.forward"],
    )
    expect(categoryProfile.bindings["target.next"]).toEqual([
      { type: "keyboard_code", code: "KeyE" },
    ])

    const [allReset] = update(
      Model.make({ ...initialModel, inputProfile: custom }),
      ResetAllInputBindings(),
    )
    expect(parseOpenAgentsInputProfileOrDefault(allReset.inputProfile)).toEqual(
      openAgentsDefaultInputProfile,
    )
  })

  test("running the PersistInputProfile effect writes localStorage", () => {
    installLocalStorage()
    const profile = inputProfileWithBinding(
      openAgentsDefaultInputProfile,
      "movement.forward",
      0,
      { type: "keyboard_code", code: "KeyI" },
    )
    const result = Effect.runSync(PersistInputProfile({ profile }).effect)

    expect(result._tag).toBe("SettledPersistInputProfile")
    expect(loadInputProfile().bindings["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
      { type: "keyboard_code", code: "ArrowUp" },
    ])
  })
})

describe("#5472 preferences apply at init + render", () => {
  afterEach(() => removeLocalStorage())

  test("initialRuntimeState seeds theme + spawn defaults from saved preferences", () => {
    installLocalStorage()
    savePreferences({
      theme: "light",
      defaultAdapter: "claude_agent",
      defaultLane: "cloud-gcp",
      showNotificationPanel: false,
      gatewayInferenceFallback: "off",
    })
    const [model] = initialRuntimeState()
    expect(model.themePreference).toBe("light")
    expect(model.defaultAdapter).toBe("claude_agent")
    expect(model.defaultLane).toBe("cloud-gcp")
    expect(model.showNotificationPanel).toBe(false)
    // #5485: the saved gateway-fallback intent is applied at app entry.
    expect(model.gatewayInferenceFallback).toBe("off")
    // Defaults seed the live spawn fields so they take effect from app entry.
    expect(model.spawnAdapter).toBe("claude_agent")
    expect(model.spawnLane).toBe("cloud-gcp")
    // Init never reaches into identity / Pylon home from preferences.
    expect(model.identityChoiceState).toBe(null)
  })

  test("initialRuntimeState seeds the active input profile from separate storage", () => {
    installLocalStorage()
    saveInputProfile(
      inputProfileWithBinding(
        openAgentsDefaultInputProfile,
        "movement.forward",
        0,
        { type: "keyboard_code", code: "KeyI" },
      ),
    )

    const [model] = initialRuntimeState()

    expect(parseOpenAgentsInputProfileOrDefault(model.inputProfile).bindings[
      "movement.forward"
    ]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
      { type: "keyboard_code", code: "ArrowUp" },
    ])
  })

  test("initialRuntimeState falls back to dark/codex/auto when nothing is saved", () => {
    removeLocalStorage()
    const [model] = initialRuntimeState()
    expect(model.themePreference).toBe("dark")
    expect(model.spawnAdapter).toBe("codex")
    expect(model.spawnLane).toBe("auto")
  })

  test("the Settings pane renders a mountable Document under both themes", () => {
    for (const theme of ["dark", "light"] as const) {
      const model = Model.make({ ...initialModel, pane: "settings", themePreference: theme })
      const doc = view(model) as { title: string; body: unknown }
      expect(doc.title).toBe("Autopilot")
      expect(doc.body).toBeDefined()
      expect(typeof doc.body).toBe("object")
    }
  })

  test("the Settings pane renders keybinding categories, capture state, and conflicts", () => {
    const conflictProfile = localProfile({
      ...openAgentsDefaultInputProfile.bindings,
      "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
      "movement.backward": [{ type: "keyboard_code", code: "KeyI" }],
    })
    const model = Model.make({
      ...initialModel,
      pane: "settings",
      inputProfile: conflictProfile,
      inputBindingCapture: { actionId: "movement.forward", slot: 1 },
    })
    const tree = serializeView(view(model).body)

    expect(tree).toContain("Keybindings")
    expect(tree).toContain("Movement")
    expect(tree).toContain("Camera")
    expect(tree).toContain("Targeting")
    expect(tree).toContain("Interaction")
    expect(tree).toContain("HUD")
    expect(tree).toContain("App")
    expect(tree).toContain("Code")
    expect(tree).toContain("Action Bar")
    expect(tree).toContain("Move Forward")
    expect(tree).toContain("Press a key")
    expect(tree).toContain("keybinding-row-conflict")
    expect(tree).toContain("I")
    expect(tree).toContain("Restore all")
  })
})
