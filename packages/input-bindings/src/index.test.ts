import { describe, expect, test } from "bun:test"

import {
  OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  decodeOpenAgentsInputProfile,
  detectOpenAgentsInputConflicts,
  openAgentsDefaultInputProfile,
  openAgentsInputActionSpecById,
  openAgentsInputActionSpecs,
  openAgentsInputBindingKey,
  openAgentsInputBindingLabel,
  openAgentsNativeReservedBindings,
  parseOpenAgentsInputProfileOrDefault,
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
})
