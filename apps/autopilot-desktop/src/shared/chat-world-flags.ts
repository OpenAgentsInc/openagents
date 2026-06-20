// Chat-world / Verse build-time feature flags (P2.5 wiring · #5730, #5819).
//
// Single source of truth for the two chat-world flags so the VIEW (view.ts)
// and the SUBSCRIPTIONS (chat-world-subscriptions.ts) agree on exactly what is
// on. The launch build defaults the Verse bundle ON via VITE_VERSE_ENABLED
// (implicit true), while retaining the older per-feature Vite env overrides and
// an explicit VITE_DISABLE_VERSE hard kill switch for fallback/debug builds.
//
// The pure mappers in shared/chat-world-scene.ts read globalThis.__OA_FLAGS for
// headless test override; the desktop runtime instead resolves the build flags
// here and passes them EXPLICITLY into the subscription hooks (deps.flags), so
// there is no hidden global to keep in sync.

import type { ChatWorldFlags } from "./chat-world-scene"

const envValue = (name: string): string | undefined => {
  const viteEnv = (import.meta as { env?: Record<string, string | undefined> }).env
  const processEnv = (globalThis as {
    process?: { env?: Record<string, string | undefined> }
  }).process?.env
  return viteEnv?.[name] ?? processEnv?.[name]
}

const envFlag = (name: string, fallback = false): boolean => {
  const raw = envValue(name)
  if (raw === undefined) return fallback
  const normalized = raw.trim().toLowerCase()
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on"
}

const verseDisabled = (): boolean =>
  envFlag("VITE_DISABLE_VERSE") || envFlag("VITE_VERSE_DISABLED")

const verseLaunchDefault = (): boolean =>
  !verseDisabled() && envFlag("VITE_VERSE_ENABLED", true)

const verseFeatureFlag = (name: string): boolean =>
  !verseDisabled() && envFlag(name, verseLaunchDefault())

// Resolve the chat-world flags from the build env. CHAT_WORLD_PAYMENTS implies
// CHAT_WORLD_SCENE — payment beams have nowhere to fly without the scene — so a
// payments-only misconfiguration still yields a coherent (scene-on) state.
export const chatWorldBuildFlags = (): ChatWorldFlags => {
  const scene = verseFeatureFlag("VITE_CHAT_WORLD_SCENE")
  const payments = verseFeatureFlag("VITE_CHAT_WORLD_PAYMENTS")
  return {
    CHAT_WORLD_SCENE: scene || payments,
    CHAT_WORLD_PAYMENTS: payments,
  }
}

export const agentCharacterCreationFlag = (): boolean =>
  verseFeatureFlag("VITE_AGENT_CHARACTER_CREATION")

export const chatWorldMultiplayerFlag = (): boolean =>
  verseFeatureFlag("VITE_CHAT_WORLD_MULTIPLAYER")

export const chatWorldGameLayerFlags = (): {
  readonly hotbar: boolean
  readonly reputation: boolean
  readonly manaHud: boolean
  readonly handTracking: boolean
} => ({
  hotbar: envFlag("VITE_CHAT_WORLD_HOTBAR"),
  reputation: envFlag("VITE_CHAT_WORLD_REPUTATION"),
  manaHud: envFlag("VITE_CHAT_WORLD_MANA_HUD"),
  handTracking: envFlag("VITE_CHAT_WORLD_HAND_TRACKING"),
})
