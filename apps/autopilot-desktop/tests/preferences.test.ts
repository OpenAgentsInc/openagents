import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"

import { PersistPreferences } from "../src/ui/commands"
import { initialRuntimeState } from "../src/ui/initial-state"
import {
  ChangedDefaultAdapter,
  ChangedDefaultLane,
  ChangedThemePreference,
  SettledPersistPreferences,
  ToggledNotificationPanel,
} from "../src/ui/message"
import { initialModel, Model } from "../src/ui/model"
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
    })
  })

  test("save → load round-trips every field", () => {
    installLocalStorage()
    const chosen = {
      theme: "light" as const,
      defaultAdapter: "claude_agent" as const,
      defaultLane: "cloud-gcp" as const,
      showNotificationPanel: false,
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
      "showNotificationPanel",
      "theme",
    ])
  })

  test("themeAttr maps the preference to its data-theme value", () => {
    expect(themeAttr("dark")).toBe("dark")
    expect(themeAttr("light")).toBe("light")
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

  test("the PersistPreferences command carries exactly the four preference fields", () => {
    const next = Model.make({
      ...initialModel,
      themePreference: "light",
      defaultAdapter: "apple_fm",
      defaultLane: "local",
      showNotificationPanel: false,
    })
    const [, commands] = update(next, ChangedThemePreference({ theme: "light" }))
    const persist = commands[0] as unknown as { args: Record<string, unknown> }
    expect(persist.args).toEqual({
      theme: "light",
      defaultAdapter: "apple_fm",
      defaultLane: "local",
      showNotificationPanel: false,
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
    })
    const result = Effect.runSync(command.effect)
    expect(result._tag).toBe("SettledPersistPreferences")
    expect(loadPreferences()).toEqual({
      theme: "light",
      defaultAdapter: "claude_agent",
      defaultLane: "cloud-shc",
      showNotificationPanel: false,
    })
    removeLocalStorage()
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
    })
    const [model] = initialRuntimeState()
    expect(model.themePreference).toBe("light")
    expect(model.defaultAdapter).toBe("claude_agent")
    expect(model.defaultLane).toBe("cloud-gcp")
    expect(model.showNotificationPanel).toBe(false)
    // Defaults seed the live spawn fields so they take effect from app entry.
    expect(model.spawnAdapter).toBe("claude_agent")
    expect(model.spawnLane).toBe("cloud-gcp")
    // Init never reaches into identity / Pylon home from preferences.
    expect(model.identityChoiceState).toBe(null)
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
})
