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

import type { ChatWorldFlags } from "./chat-world-scene.js"

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

// MMO character selection (#verse/mmo-characters-per-account). One account can
// field MANY characters; this app launch picks ONE via OA_CHARACTER. Default
// "main" so a single instance behaves exactly as before. Two instances on the
// same account with OA_CHARACTER=main and OA_CHARACTER=alt become two distinct,
// mutually-visible avatars.
//
// Plumbing note: OA_CHARACTER is set on the Bun LAUNCHER process at runtime and
// is NOT VITE_-prefixed, so it is absent from both `import.meta.env` (a
// build-time Vite define) and the renderer's (non-existent) `process.env`. The
// Bun host therefore injects the resolved value into the webview as the global
// `globalThis.__OA_CHARACTER` (see src/bun/index.ts), and we read that FIRST.
// The env paths remain as fallbacks for Vite builds/tests. See
// docs/game/2026-06-21-mmo-characters-per-account-verse-presence.md.
export const chatWorldCharacterId = (): string => {
  const injected = (globalThis as { __OA_CHARACTER?: unknown }).__OA_CHARACTER
  if (typeof injected === "string") {
    const trimmedInjected = injected.trim()
    if (trimmedInjected.length > 0) return trimmedInjected
  }
  const raw = envValue("OA_CHARACTER") ?? envValue("VITE_OA_CHARACTER")
  const trimmed = raw?.trim()
  return trimmed !== undefined && trimmed.length > 0 ? trimmed : "main"
}

// One switch for visible Verse HUD/actions. Default OFF for the current launch
// pass: the world remains navigable, but bottom bars and global shortcuts stay
// dark unless explicitly enabled.
export const chatWorldHudFlag = (): boolean =>
  !verseDisabled() && envFlag("VITE_CHAT_WORLD_HUD", false)

export const chatWorldGameLayerFlags = (): {
  readonly hotbar: boolean
  readonly reputation: boolean
  readonly manaHud: boolean
  readonly handTracking: boolean
} => ({
  hotbar: chatWorldHudFlag() && envFlag("VITE_CHAT_WORLD_HOTBAR"),
  reputation: envFlag("VITE_CHAT_WORLD_REPUTATION"),
  manaHud: envFlag("VITE_CHAT_WORLD_MANA_HUD"),
  handTracking: envFlag("VITE_CHAT_WORLD_HAND_TRACKING"),
})
