import { requireNativeViewManager } from "expo-modules-core"
import type * as React from "react"

/**
 * OpenAgents mobile (GL-2 #8648, #8597) — JS bindings for the SwiftUI Liquid
 * Glass chrome islands (circular icon button, pill, floating composer).
 *
 * Props IN are serializable projections of the Effect Native program's state;
 * each named event OUT is converted by the app shell into a typed Effect
 * Native intent (never an untyped callback into app logic). iOS only; the
 * shell renders honest fallbacks elsewhere.
 */

export interface GlassIconButtonProps {
  readonly symbol: string
  readonly accessibilityLabelText: string
  readonly onTap?: () => void
  readonly style?: Record<string, unknown>
}

export interface GlassPillProps {
  readonly label: string
  readonly symbol?: string
  readonly onTap?: () => void
  readonly style?: Record<string, unknown>
}

export interface GlassComposerProps {
  readonly placeholder: string
  readonly onTapComposer?: () => void
  readonly onTapMic?: () => void
  readonly onTapPlus?: () => void
  readonly style?: Record<string, unknown>
}

const load = <Props,>(viewName: string): React.ComponentType<Props> | undefined => {
  try {
    return requireNativeViewManager<Props>("OpenAgentsLiquidGlass", viewName)
  } catch {
    // Native module absent (Expo Go, Android, tests) — caller renders an
    // honest fallback instead of crashing.
    return undefined
  }
}

export const loadGlassIconButton = (): React.ComponentType<GlassIconButtonProps> | undefined =>
  load<GlassIconButtonProps>("GlassIconButton")

export const loadGlassPill = (): React.ComponentType<GlassPillProps> | undefined =>
  load<GlassPillProps>("GlassPill")

export const loadGlassComposer = (): React.ComponentType<GlassComposerProps> | undefined =>
  load<GlassComposerProps>("GlassComposer")
