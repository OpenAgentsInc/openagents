import * as React from "react"
import * as ReactNative from "react-native"

import {
  createEffectNativeSurface,
  type EffectNativeSurfaceProps,
  type ReactNativeDependencies,
} from "@effect-native/render-rn"

/**
 * EN-3 (#8568) mount point — the Khala-mobile embed path for Effect Native.
 *
 * `@effect-native/render-rn` renders a typed `View` stream into real React
 * Native host components. `createEffectNativeSurface` takes the host deps
 * (React + React Native) EXPLICITLY, so we bind them once here from normal ESM
 * imports (Metro-friendly) instead of the package's `require()` fallback, and
 * build the surface component a SINGLE time at module scope — a stable
 * component identity so remounts don't churn the surface's view stream
 * subscription.
 *
 * This is the desktop `runMainDesktop`/DOM-mount analogue for RN: any Expo /
 * React-Navigation screen can now render an Effect Native `ViewProgram` by
 * dropping `<EffectNativeHost viewStream=... report=... theme=... />` into its
 * tree. Adapter #1 is proven the moment one screen mounts through it.
 */
const dependencies = {
  React: React as unknown as ReactNativeDependencies["React"],
  ReactNative: ReactNative as unknown as ReactNativeDependencies["ReactNative"],
} satisfies ReactNativeDependencies

const KhalaEffectNativeSurface = createEffectNativeSurface(dependencies)

export const EffectNativeHost = (
  props: EffectNativeSurfaceProps,
): React.ReactElement =>
  React.createElement(
    KhalaEffectNativeSurface as unknown as React.FunctionComponent<EffectNativeSurfaceProps>,
    props,
  )
