// Chat-world build-time feature flags (P2.5 wiring · #5730).
//
// Single source of truth for the two chat-world flags so the VIEW (view.ts)
// and the SUBSCRIPTIONS (chat-world-subscriptions.ts) agree on exactly what is
// on. Both are resolved from Vite build env (VITE_CHAT_WORLD_SCENE /
// VITE_CHAT_WORLD_PAYMENTS) and default OFF, so the desktop surface is
// byte-for-byte the current pane unless a flag is explicitly built in.
//
// The pure mappers in shared/chat-world-scene.ts read globalThis.__OA_FLAGS for
// headless test override; the desktop runtime instead resolves the build flags
// here and passes them EXPLICITLY into the subscription hooks (deps.flags), so
// there is no hidden global to keep in sync.

import type { ChatWorldFlags } from "./chat-world-scene"

const envFlag = (name: string): boolean => {
  const env = (import.meta as { env?: Record<string, string | undefined> }).env
  return (env?.[name] ?? "0") === "1"
}

// Resolve the chat-world flags from the build env. CHAT_WORLD_PAYMENTS implies
// CHAT_WORLD_SCENE — payment beams have nowhere to fly without the scene — so a
// payments-only misconfiguration still yields a coherent (scene-on) state.
export const chatWorldBuildFlags = (): ChatWorldFlags => {
  const scene = envFlag("VITE_CHAT_WORLD_SCENE")
  const payments = envFlag("VITE_CHAT_WORLD_PAYMENTS")
  return {
    CHAT_WORLD_SCENE: scene || payments,
    CHAT_WORLD_PAYMENTS: payments,
  }
}

export const agentCharacterCreationFlag = (): boolean =>
  envFlag("VITE_AGENT_CHARACTER_CREATION")

export const chatWorldMultiplayerFlag = (): boolean =>
  envFlag("VITE_CHAT_WORLD_MULTIPLAYER")

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
