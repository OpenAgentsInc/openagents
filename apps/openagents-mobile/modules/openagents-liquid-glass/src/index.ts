import { requireNativeViewManager } from "expo-modules-core"
import type * as React from "react"

/**
 * OpenAgents mobile (#8597) — JS binding for the SwiftUI Liquid Glass island.
 *
 * Props IN are serializable values derived from the Effect Native view
 * program's state; the single `onGlassTap` event OUT is converted by the app
 * shell into a typed Effect Native intent (never an untyped callback into app
 * logic). iOS only; the shell renders an honest fallback elsewhere.
 */
export interface OpenAgentsLiquidGlassViewProps {
  readonly title: string
  readonly subtitle: string
  readonly buttonLabel: string
  readonly tapCount: number
  readonly onGlassTap?: () => void
  readonly style?: Record<string, unknown>
}

export const loadLiquidGlassView = ():
  | React.ComponentType<OpenAgentsLiquidGlassViewProps>
  | undefined => {
  try {
    return requireNativeViewManager<OpenAgentsLiquidGlassViewProps>(
      "OpenAgentsLiquidGlass",
    )
  } catch {
    // Native module absent (Expo Go, Android, tests) — caller renders an
    // honest fallback instead of crashing.
    return undefined
  }
}
