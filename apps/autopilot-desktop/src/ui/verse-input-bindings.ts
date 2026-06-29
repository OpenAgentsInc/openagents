import {
  openAgentsDefaultInputProfile,
  parseOpenAgentsInputProfileOrDefault,
  type OpenAgentsInputBinding,
  type OpenAgentsInputModifiers,
  type OpenAgentsInputProfile,
} from "@openagentsinc/input-bindings"
import type {
  TrainingRunKeyboardTargeting,
  TrainingRunKeyboardTargetingBinding,
  WasdAction,
  WasdKeyboardBindingMap,
} from "@openagentsinc/three-effect/core"

export type VerseInputBindingProjection = Readonly<{
  activeContext: "verse_explore" | "verse_code_overlay"
  keyboardTargeting: Pick<TrainingRunKeyboardTargeting, "bindings">
  lastResolvedAction: string | null
  movement: WasdKeyboardBindingMap
  profileId: string
  schemaVersion: string
}>

const movementActions: ReadonlyArray<readonly [string, WasdAction]> = [
  ["movement.backward", "backward"],
  ["movement.fall", "fall"],
  ["movement.forward", "forward"],
  ["movement.strafe_left", "left"],
  ["movement.strafe_right", "right"],
  ["movement.jump", "rise"],
  ["movement.sprint", "sprint"],
]

const hasPrimaryModifier = (
  modifiers: OpenAgentsInputModifiers | undefined,
): boolean => modifiers?.primary === true

const normalizedModifierFlag = (
  modifiers: OpenAgentsInputModifiers | undefined,
  key: "alt" | "ctrl" | "meta" | "shift",
): boolean => modifiers?.[key] === true

const movementCodesForAction = (
  profile: OpenAgentsInputProfile,
  actionId: string,
): readonly string[] =>
  (profile.bindings[actionId] ?? []).flatMap((binding) => {
    if (binding.type !== "keyboard_code") return []
    if (hasPrimaryModifier(binding.modifiers)) return []
    if (
      normalizedModifierFlag(binding.modifiers, "alt") ||
      normalizedModifierFlag(binding.modifiers, "ctrl") ||
      normalizedModifierFlag(binding.modifiers, "meta") ||
      normalizedModifierFlag(binding.modifiers, "shift")
    ) {
      return []
    }
    return [binding.code]
  })

const targetingBindingFromInputBinding = (
  binding: OpenAgentsInputBinding,
): TrainingRunKeyboardTargetingBinding | null => {
  if (
    binding.type !== "keyboard_code" &&
    binding.type !== "keyboard_key"
  ) {
    return null
  }
  if (hasPrimaryModifier(binding.modifiers)) {
    return null
  }
  return {
    ...(binding.type === "keyboard_code"
      ? { code: binding.code }
      : { key: binding.key }),
    altKey: normalizedModifierFlag(binding.modifiers, "alt"),
    ctrlKey: normalizedModifierFlag(binding.modifiers, "ctrl"),
    metaKey: normalizedModifierFlag(binding.modifiers, "meta"),
    shiftKey: normalizedModifierFlag(binding.modifiers, "shift"),
  }
}

const targetingBindingsForAction = (
  profile: OpenAgentsInputProfile,
  actionId: string,
): readonly TrainingRunKeyboardTargetingBinding[] =>
  (profile.bindings[actionId] ?? [])
    .map(targetingBindingFromInputBinding)
    .filter((binding): binding is TrainingRunKeyboardTargetingBinding => binding !== null)

export const verseInputBindingProjection = (
  value: unknown,
  activeContext: VerseInputBindingProjection["activeContext"],
): VerseInputBindingProjection => {
  const profile = safeInputProfileValue(value)
  const movement = Object.fromEntries(
    movementActions.map(([actionId, wasdAction]) => [
      wasdAction,
      movementCodesForAction(profile, actionId),
    ]),
  ) as WasdKeyboardBindingMap

  return {
    activeContext,
    keyboardTargeting: {
      bindings: {
        next: targetingBindingsForAction(profile, "target.next"),
        previous: targetingBindingsForAction(profile, "target.previous"),
      },
    },
    lastResolvedAction: null,
    movement,
    profileId: profile.profileId,
    schemaVersion: profile.schemaVersion,
  }
}

export const defaultVerseInputBindingProjection = (
  activeContext: VerseInputBindingProjection["activeContext"] = "verse_explore",
): VerseInputBindingProjection =>
  verseInputBindingProjection(openAgentsDefaultInputProfile, activeContext)

export const safeInputProfileValue = (
  value: unknown,
): OpenAgentsInputProfile => parseOpenAgentsInputProfileOrDefault(value)
