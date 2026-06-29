import { Schema as S } from "effect"
import {
  OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  OpenAgentsInputBinding as OpenAgentsInputBindingSchema,
  openAgentsDefaultInputProfile,
  openAgentsInputActionSpecById,
  openAgentsInputActionSpecs,
  parseOpenAgentsInputProfileOrDefault,
  type OpenAgentsInputBinding,
  type OpenAgentsInputModifiers,
  type OpenAgentsInputProfile,
} from "@openagentsinc/input-bindings"

export const INPUT_PROFILE_STORAGE_KEY = "autopilot-desktop.input-bindings.v1"

const decodeInputBindingOption = S.decodeUnknownOption(OpenAgentsInputBindingSchema)

const storage = (): Storage | null => {
  try {
    return typeof localStorage === "undefined" ? null : localStorage
  } catch {
    return null
  }
}

export const loadInputProfile = (): OpenAgentsInputProfile => {
  const store = storage()
  if (store === null) return openAgentsDefaultInputProfile
  const raw = (() => {
    try {
      return store.getItem(INPUT_PROFILE_STORAGE_KEY)
    } catch {
      return null
    }
  })()
  if (raw === null || raw.trim() === "") return openAgentsDefaultInputProfile
  try {
    return parseOpenAgentsInputProfileOrDefault(JSON.parse(raw))
  } catch {
    return openAgentsDefaultInputProfile
  }
}

export const saveInputProfile = (profile: OpenAgentsInputProfile): void => {
  const store = storage()
  if (store === null) return
  try {
    store.setItem(
      INPUT_PROFILE_STORAGE_KEY,
      JSON.stringify(parseOpenAgentsInputProfileOrDefault(profile)),
    )
  } catch {
    // Local keybinding persistence is a convenience, not app authority.
  }
}

export const decodeInputBindingOrNull = (
  value: unknown,
): OpenAgentsInputBinding | null => {
  const decoded = decodeInputBindingOption(value)
  return decoded._tag === "Some" ? decoded.value : null
}

export const inputProfileWithBinding = (
  value: unknown,
  actionId: string,
  slot: number,
  binding: OpenAgentsInputBinding,
): OpenAgentsInputProfile => {
  const profile = parseOpenAgentsInputProfileOrDefault(value)
  const current = [...(profile.bindings[actionId] ?? [])]
  const index = slot <= 0 ? 0 : current.length === 0 ? 0 : Math.min(slot, current.length)
  if (index < current.length) {
    current[index] = binding
  } else {
    current.push(binding)
  }
  return localProfile({
    ...profile.bindings,
    [actionId]: current,
  })
}

export const inputProfileWithResetAction = (
  value: unknown,
  actionId: string,
): OpenAgentsInputProfile => {
  const profile = parseOpenAgentsInputProfileOrDefault(value)
  const spec = openAgentsInputActionSpecById.get(actionId)
  if (spec === undefined) return profile
  return localProfile({
    ...profile.bindings,
    [actionId]: spec.defaultBindings.map((binding) => ({ ...binding })),
  })
}

export const inputProfileWithResetCategory = (
  value: unknown,
  category: string,
): OpenAgentsInputProfile => {
  const profile = parseOpenAgentsInputProfileOrDefault(value)
  const next = { ...profile.bindings }
  for (const spec of openAgentsInputActionSpecs) {
    if (spec.category !== category) continue
    next[spec.id] = spec.defaultBindings.map((binding) => ({ ...binding }))
  }
  return localProfile(next)
}

export const inputProfileWithResetAll = (): OpenAgentsInputProfile =>
  openAgentsDefaultInputProfile

export const capturedKeyboardBindingFromKey = (
  key: string,
  modifiers: Readonly<{
    shiftKey: boolean
    ctrlKey: boolean
    altKey: boolean
    metaKey: boolean
  }>,
): OpenAgentsInputBinding | null => {
  if (key === "" || key === "Dead") return null
  const inputModifiers = inputModifiersFromKeyEvent(modifiers)
  const code = keyboardCodeFromKey(key)
  return code === null
    ? maybeWithModifiers({ type: "keyboard_key", key }, inputModifiers)
    : maybeWithModifiers({ type: "keyboard_code", code }, inputModifiers)
}

const localProfile = (
  bindings: OpenAgentsInputProfile["bindings"],
): OpenAgentsInputProfile => ({
  schemaVersion: OPENAGENTS_INPUT_BINDINGS_SCHEMA_VERSION,
  profileId: "local-custom",
  bindings,
})

const inputModifiersFromKeyEvent = (
  modifiers: Readonly<{
    shiftKey: boolean
    ctrlKey: boolean
    altKey: boolean
    metaKey: boolean
  }>,
): OpenAgentsInputModifiers | undefined => {
  const next: OpenAgentsInputModifiers = {
    ...(modifiers.ctrlKey || modifiers.metaKey ? { primary: true } : {}),
    ...(modifiers.shiftKey ? { shift: true } : {}),
    ...(modifiers.altKey ? { alt: true } : {}),
  }
  return Object.keys(next).length === 0 ? undefined : next
}

const maybeWithModifiers = <T extends OpenAgentsInputBinding>(
  binding: T,
  modifiers: OpenAgentsInputModifiers | undefined,
): OpenAgentsInputBinding =>
  modifiers === undefined ? binding : { ...binding, modifiers }

const keyboardCodeFromKey = (key: string): string | null => {
  if (/^[a-z]$/i.test(key)) return `Key${key.toUpperCase()}`
  if (/^[0-9]$/.test(key)) return `Digit${key}`
  switch (key) {
    case " ":
    case "Spacebar":
      return "Space"
    case "ArrowUp":
    case "ArrowDown":
    case "ArrowLeft":
    case "ArrowRight":
    case "Tab":
    case "Enter":
    case "Escape":
    case "Backspace":
      return key
    case "Shift":
      return "ShiftLeft"
    default:
      return null
  }
}
