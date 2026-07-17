import * as React from "react"
import * as ReactNative from "react-native"
import * as ExpoClipboard from "expo-clipboard"

import { clipboardWriteError } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import {
  createEffectNativeSurface,
  type EffectNativeSurfaceProps,
  type ReactNativeDependencies,
} from "@effect-native/render-rn"
import { mobileTerminalHostDriver } from "./mobile-terminal-host-driver"

/**
 * OpenAgents mobile (#8597) mount point for Effect Native.
 *
 * `@effect-native/render-rn` renders a typed `View` stream into real React
 * Native host components. `createEffectNativeSurface` takes the host deps
 * (React + React Native) EXPLICITLY, so we bind them once here from normal ESM
 * imports (Metro-friendly) instead of the package's `require()` fallback, and
 * build the surface component a SINGLE time at module scope — a stable
 * component identity so remounts don't churn the surface's view stream
 * subscription.
 *
 * Any screen in this app renders an Effect Native `ViewProgram` by dropping
 * `<EffectNativeHost viewStream=... report=... theme=... />` into its tree.
 * Same embed seam as the other Effect Native hosts (DOM, desktop): the pure
 * view-program layer never imports React or React Native.
 */
const dependencies = {
  React: React as unknown as ReactNativeDependencies["React"],
  ReactNative: ReactNative as unknown as ReactNativeDependencies["ReactNative"],
} satisfies ReactNativeDependencies

const OpenAgentsEffectNativeSurface = createEffectNativeSurface(dependencies)

const clipboard = {
  writeText: (content: string) => Effect.tryPromise({
    try: () => ExpoClipboard.setStringAsync(content),
    catch: () => clipboardWriteError("Native clipboard write failed"),
  }),
}

export const EffectNativeHost = (
  props: EffectNativeSurfaceProps,
): React.ReactElement =>
  React.createElement(
    OpenAgentsEffectNativeSurface as unknown as React.FunctionComponent<EffectNativeSurfaceProps>,
    { ...props, clipboard, hostDrivers: [mobileTerminalHostDriver] },
  )
