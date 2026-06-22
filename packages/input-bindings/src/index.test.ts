import { describe, expect, test } from "bun:test"

import {
  OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  createOpenAgentsKeyboardControls,
  decodeOpenAgentsInputProfile,
  detectOpenAgentsInputConflicts,
  openAgentsDefaultInputProfile,
  openAgentsInputActionMapFromProfile,
  openAgentsInputActionSpecById,
  openAgentsInputActionSpecs,
  openAgentsInputBindingKey,
  openAgentsInputBindingLabel,
  openAgentsKeyboardEventMatchesBinding,
  openAgentsNativeReservedBindings,
  parseOpenAgentsInputProfileOrDefault,
  resolveOpenAgentsKeyboardEventActionBindings,
  type OpenAgentsKeyboardControlsChange,
  type OpenAgentsKeyboardEventLike,
  type OpenAgentsKeyboardEventListener,
  type OpenAgentsKeyboardEventType,
  type OpenAgentsInputProfile,
} from "./index.js"

describe("@openagentsinc/input-bindings", () => {
  test("exports a default profile matching current Verse movement defaults", () => {
    expect(openAgentsDefaultInputProfile.schemaVersion).toBe(
      "openagents.input-bindings.v1",
    )
    expect(openAgentsDefaultInputProfile.profileId).toBe("default")
    expect(openAgentsDefaultInputProfile.bindings["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyW" },
      { type: "keyboard_code", code: "ArrowUp" },
    ])
    expect(openAgentsDefaultInputProfile.bindings["movement.backward"]).toEqual([
      { type: "keyboard_code", code: "KeyS" },
      { type: "keyboard_code", code: "ArrowDown" },
    ])
    expect(openAgentsDefaultInputProfile.bindings["movement.sprint"]).toEqual([
      { type: "keyboard_code", code: "ShiftLeft" },
      { type: "keyboard_code", code: "ShiftRight" },
    ])
    expect(openAgentsDefaultInputProfile.bindings["movement.jump"]).toEqual([
      { type: "keyboard_code", code: "Space" },
    ])
    expect(openAgentsDefaultInputProfile.bindings["target.next"]).toEqual([
      { type: "keyboard_code", code: "Tab" },
    ])
  })

  test("covers app, palette, targeting, interaction, hud, and action-bar actions", () => {
    const ids = new Set(openAgentsInputActionSpecs.map((spec) => spec.id))
    expect(ids.has("app.command_palette")).toBe(true)
    expect(ids.has("palette.run")).toBe(true)
    expect(ids.has("target.previous")).toBe(true)
    expect(ids.has("interact.primary")).toBe(true)
    expect(ids.has("tip.selected_pylon")).toBe(true)
    expect(ids.has("hud.toggle_code_overlay")).toBe(true)
    expect(ids.has("action_bar.slot_10")).toBe(true)
    expect(openAgentsInputActionSpecById.get("movement.forward")?.kind).toBe("hold")
  })

  test("decodes valid profiles and falls back on corrupt persisted profiles", () => {
    const profile = decodeOpenAgentsInputProfile({
      schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
      profileId: "local",
      bindings: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
      },
    })
    expect(profile.bindings["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
    ])

    expect(parseOpenAgentsInputProfileOrDefault({ bad: "shape" })).toBe(
      openAgentsDefaultInputProfile,
    )
  })

  test("normalizes partial profiles against defaults", () => {
    const profile = parseOpenAgentsInputProfileOrDefault({
      schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
      profileId: "local",
      bindings: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
      },
    })

    expect(profile.bindings["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
    ])
    expect(profile.bindings["movement.backward"]).toEqual([
      { type: "keyboard_code", code: "KeyS" },
      { type: "keyboard_code", code: "ArrowDown" },
    ])
  })

  test("formats stable binding keys and human-readable labels", () => {
    expect(openAgentsInputBindingKey({ type: "keyboard_code", code: "KeyW" }))
      .toBe("code:KeyW")
    expect(openAgentsInputBindingKey({
      type: "keyboard_key",
      key: "v",
      modifiers: { primary: true, shift: true },
    })).toBe("primary+shift+key:v")
    expect(openAgentsInputBindingLabel({ type: "keyboard_code", code: "KeyW" }))
      .toBe("W")
    expect(openAgentsInputBindingLabel({
      type: "keyboard_key",
      key: "v",
      modifiers: { primary: true, shift: true },
    })).toBe("Cmd/Ctrl+Shift+V")
    expect(openAgentsInputBindingLabel({ type: "keyboard_code", code: "ShiftLeft" }))
      .toBe("Left Shift")
    expect(openAgentsInputBindingLabel({ type: "wheel", direction: "up" }))
      .toBe("Wheel Up")
  })

  test("allows same key in disjoint contexts", () => {
    const conflicts = detectOpenAgentsInputConflicts(openAgentsDefaultInputProfile)
      .filter((conflict) => conflict.actionIds.includes("app.pane_previous"))
    expect(conflicts).toEqual([])
  })

  test("detects hard conflicts inside the same context", () => {
    const profile: OpenAgentsInputProfile = {
      schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
      profileId: "conflict",
      bindings: {
        ...openAgentsDefaultInputProfile.bindings,
        "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
        "movement.backward": [{ type: "keyboard_code", code: "KeyI" }],
      },
    }
    const conflicts = detectOpenAgentsInputConflicts(profile)
    expect(conflicts).toContainEqual({
      severity: "hard",
      bindingKey: "code:KeyI",
      bindingLabel: "I",
      actionIds: ["movement.forward", "movement.backward"],
      contexts: ["verse_explore", "verse_pointer_locked"],
      reason: "Two actions use the same binding in the same context",
    })
  })

  test("detects native-reserved bindings in protected contexts", () => {
    expect(openAgentsNativeReservedBindings.length).toBeGreaterThan(0)
    const profile: OpenAgentsInputProfile = {
      schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
      profileId: "reserved",
      bindings: {
        ...openAgentsDefaultInputProfile.bindings,
        "app.submit": [
          { type: "keyboard_key", key: "v", modifiers: { primary: true } },
        ],
      },
    }

    const conflicts = detectOpenAgentsInputConflicts(profile)
    expect(conflicts).toContainEqual({
      severity: "reserved",
      bindingKey: "primary+key:v",
      bindingLabel: "Cmd/Ctrl+V",
      actionIds: ["app.submit"],
      contexts: ["text_entry"],
      reason: "Reserved for native text paste",
    })
  })

  test("builds an action map from a normalized profile", () => {
    const actionMap = openAgentsInputActionMapFromProfile({
      schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
      profileId: "local",
      bindings: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
      },
    })

    expect(actionMap["movement.forward"]).toEqual([
      { type: "keyboard_code", code: "KeyI" },
    ])
    expect(actionMap["movement.backward"]).toEqual([
      { type: "keyboard_code", code: "KeyS" },
      { type: "keyboard_code", code: "ArrowDown" },
    ])
  })

  test("matches keyboard code and key bindings", () => {
    expect(openAgentsKeyboardEventMatchesBinding(
      { type: "keyboard_code", code: "KeyW" },
      { code: "KeyW", key: "w", shiftKey: true },
    )).toBe(true)
    expect(openAgentsKeyboardEventMatchesBinding(
      { type: "keyboard_key", key: "v", modifiers: { primary: true } },
      { code: "KeyV", key: "v", ctrlKey: true },
      { allowExtraModifiers: false },
    )).toBe(true)
    expect(openAgentsKeyboardEventMatchesBinding(
      { type: "keyboard_key", key: "v", modifiers: { primary: true } },
      { code: "KeyV", key: "v", metaKey: true },
      { allowExtraModifiers: false },
    )).toBe(true)
    expect(openAgentsKeyboardEventMatchesBinding(
      { type: "keyboard_key", key: "v", modifiers: { primary: true } },
      { code: "KeyV", key: "v" },
    )).toBe(false)
    expect(openAgentsKeyboardEventMatchesBinding(
      { type: "keyboard_code", code: "KeyW" },
      { code: "KeyW", key: "w", shiftKey: true },
      { allowExtraModifiers: false },
    )).toBe(false)
  })

  test("resolves named actions from a scoped keyboard event", () => {
    const matches = resolveOpenAgentsKeyboardEventActionBindings(
      {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
        "app.command_palette": [
          { type: "keyboard_key", key: "k", modifiers: { primary: true } },
        ],
      },
      { code: "KeyK", key: "k", metaKey: true },
    )

    expect(matches).toEqual([
      {
        actionId: "app.command_palette",
        binding: {
          type: "keyboard_key",
          key: "k",
          modifiers: { primary: true },
        },
      },
    ])
  })

  test("tracks pressed state and subscriptions by named action", () => {
    const source = new FakeKeyboardEventSource()
    const changes: Array<OpenAgentsKeyboardControlsChange> = []
    const controls = createOpenAgentsKeyboardControls({
      eventSource: source,
      actionMap: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
      },
      onChange: (change) => changes.push(change),
    })

    source.dispatch("keydown", { code: "KeyW", key: "w" })
    expect(controls.isPressed("movement.forward")).toBe(true)
    expect(controls.getState()).toEqual({ "movement.forward": true })

    source.dispatch("keyup", { code: "KeyW", key: "w" })
    expect(controls.isPressed("movement.forward")).toBe(false)
    expect(changes.map((change) => [change.actionId, change.pressed])).toEqual([
      ["movement.forward", true],
      ["movement.forward", false],
    ])
  })

  test("ignores unmapped keys and dedupes repeated keydown", () => {
    const source = new FakeKeyboardEventSource()
    const changes: Array<OpenAgentsKeyboardControlsChange> = []
    const controls = createOpenAgentsKeyboardControls({
      eventSource: source,
      actionMap: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
      },
      onChange: (change) => changes.push(change),
    })

    source.dispatch("keydown", { code: "KeyQ", key: "q" })
    source.dispatch("keydown", { code: "KeyW", key: "w" })
    source.dispatch("keydown", { code: "KeyW", key: "w", repeat: true })

    expect(controls.isPressed("movement.forward")).toBe(true)
    expect(changes).toHaveLength(1)
    expect(changes[0]?.repeat).toBe(false)
  })

  test("prevents default for matching actions when configured", () => {
    const source = new FakeKeyboardEventSource()
    let prevented = 0
    const controls = createOpenAgentsKeyboardControls({
      eventSource: source,
      actionMap: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
      },
      preventDefault: (change) => change.actionId === "movement.forward",
    })

    source.dispatch("keydown", {
      code: "KeyW",
      key: "w",
      preventDefault: () => {
        prevented += 1
      },
    })

    controls.dispose()
    expect(prevented).toBe(1)
  })

  test("supports subscribe unsubscribe and cleanup", () => {
    const source = new FakeKeyboardEventSource()
    const changes: Array<OpenAgentsKeyboardControlsChange> = []
    const controls = createOpenAgentsKeyboardControls({
      eventSource: source,
      actionMap: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
      },
    })
    const unsubscribe = controls.subscribe((change) => changes.push(change))

    source.dispatch("keydown", { code: "KeyW", key: "w" })
    unsubscribe()
    source.dispatch("keyup", { code: "KeyW", key: "w" })
    controls.dispose()
    source.dispatch("keydown", { code: "KeyW", key: "w" })

    expect(changes.map((change) => change.pressed)).toEqual([true])
    expect(source.listenerCount()).toBe(0)
  })

  test("updates binding maps and clears stale held state", () => {
    const source = new FakeKeyboardEventSource()
    const changes: Array<OpenAgentsKeyboardControlsChange> = []
    const controls = createOpenAgentsKeyboardControls({
      eventSource: source,
      actionMap: {
        "movement.forward": [{ type: "keyboard_code", code: "KeyW" }],
      },
      onChange: (change) => changes.push(change),
    })

    source.dispatch("keydown", { code: "KeyW", key: "w" })
    controls.updateBindings({
      "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
    })
    source.dispatch("keydown", { code: "KeyW", key: "w" })
    expect(controls.isPressed("movement.forward")).toBe(false)
    source.dispatch("keydown", { code: "KeyI", key: "i" })
    expect(controls.isPressed("movement.forward")).toBe(true)

    expect(changes.map((change) => [change.source, change.pressed])).toEqual([
      ["keydown", true],
      ["bindings_updated", false],
      ["keydown", true],
    ])
  })
})

class FakeKeyboardEventSource {
  private readonly listeners: Record<
    OpenAgentsKeyboardEventType,
    Set<OpenAgentsKeyboardEventListener>
  > = {
    keydown: new Set(),
    keyup: new Set(),
  }

  addEventListener = (
    type: OpenAgentsKeyboardEventType,
    listener: OpenAgentsKeyboardEventListener,
  ): void => {
    this.listeners[type].add(listener)
  }

  removeEventListener = (
    type: OpenAgentsKeyboardEventType,
    listener: OpenAgentsKeyboardEventListener,
  ): void => {
    this.listeners[type].delete(listener)
  }

  dispatch = (
    type: OpenAgentsKeyboardEventType,
    event: OpenAgentsKeyboardEventLike,
  ): void => {
    for (const listener of this.listeners[type]) {
      listener(event)
    }
  }

  listenerCount = (): number =>
    this.listeners.keydown.size + this.listeners.keyup.size
}
