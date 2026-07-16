import type { Theme } from "@effect-native/core"
import type { CSSProperties } from "react"

import { px } from "./internal.ts"

export type DesktopThemeCssVariables = CSSProperties & Readonly<Record<`--en-${string}`, string | number>>

/** The same token bridge used by Electron, scoped for embedded web workbenches. */
export const desktopThemeCssVariables = (theme: Theme): DesktopThemeCssVariables => {
  const variables: Record<string, string | number> = {
    backgroundColor: theme.color.background,
    color: theme.color.textPrimary,
  }
  for (const [key, value] of Object.entries(theme.color)) variables[`--en-color-${key}`] = value
  for (const [key, value] of Object.entries(theme.spacing)) variables[`--en-spacing-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.radius)) variables[`--en-radius-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.dimension)) variables[`--en-dimension-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.typeScale)) {
    variables[`--en-type-${key}-fontSize`] = px(value.fontSize)
    variables[`--en-type-${key}-lineHeight`] = px(value.lineHeight)
    variables[`--en-type-${key}-fontWeight`] = value.fontWeight
  }
  for (const [key, value] of Object.entries(theme.control)) {
    variables[`--en-control-${key}-height`] = px(value.height)
    variables[`--en-control-${key}-gutter`] = px(value.gutter)
    variables[`--en-control-${key}-radius`] = px(value.radius)
    variables[`--en-control-${key}-font-size`] = px(value.fontSize)
    variables[`--en-control-${key}-icon`] = px(value.icon)
  }
  variables["--en-motion-fast"] = `${theme.motion.durationFastMs}ms`
  variables["--en-motion-enter"] = `${theme.motion.durationEnterMs}ms`
  variables["--en-motion-exit"] = `${theme.motion.durationExitMs}ms`
  variables["--en-motion-loop"] = `${theme.motion.durationLoopMs}ms`
  variables["--en-ease-basic"] = theme.motion.easeBasic
  variables["--en-ease-enter"] = theme.motion.easeEnter
  variables["--en-ease-exit"] = theme.motion.easeExit
  variables["--en-ease-exit-snappy"] = theme.motion.easeExitSnappy
  variables["--en-ease-move"] = theme.motion.easeMove
  variables["--en-elevation-overlay-shadow"] = theme.elevation.overlayShadow
  variables["--en-elevation-raised-shadow"] = theme.elevation.overlayShadow
  variables["--en-elevation-hairline"] = `0 0 0 ${px(theme.elevation.hairlineWidth)} ${theme.color.borderSubtle}`
  return variables as DesktopThemeCssVariables
}
