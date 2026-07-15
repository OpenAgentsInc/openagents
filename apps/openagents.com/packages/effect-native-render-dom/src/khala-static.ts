import { Effect } from "effect"
import type { KhalaFrameDecoration } from "@effect-native/core"
import {
  resolveKhalaMotif,
  type ColorToken,
  type KhalaLuminanceRole,
  type KhalaMotifGeometry,
  type Theme
} from "@effect-native/tokens"

export interface KhalaStaticPathGroup {
  readonly id: string
  readonly role: KhalaLuminanceRole
  readonly color: ColorToken
  readonly width: number
  readonly data: string
}

export interface KhalaStaticDecoration {
  readonly id: string
  readonly geometry: KhalaMotifGeometry
  readonly paths: ReadonlyArray<KhalaStaticPathGroup>
}

const coordinate = (value: number): string => Number(value.toFixed(4)).toString()
const move = (x: number, y: number): string => `M${coordinate(x)} ${coordinate(y)}`
const segment = (x: number, y: number): string => `L${coordinate(x)} ${coordinate(y)}`

export const stableKhalaDomId = (id: string): string => `en-khala-${id.replace(/[^a-zA-Z0-9_-]/g, "-")}`

export const resolveKhalaStaticDecoration = (decoration: KhalaFrameDecoration, theme: Theme): KhalaStaticDecoration => {
  const geometry = Effect.runSync(
    resolveKhalaMotif(
      {
        motif: decoration.motif,
        width: decoration.width,
        height: decoration.height,
        zoom: decoration.zoom ?? 1,
        density: decoration.density ?? "comfortable",
        forcedColors: decoration.forcedColors ?? false
      },
      theme.khalaUi
    )
  )
  const grouped = new Map<string, { readonly role: KhalaLuminanceRole; readonly width: number; data: string }>()
  const append = (role: KhalaLuminanceRole, width: number, data: string): void => {
    const key = `${role}:${width}`
    const current = grouped.get(key)
    if (current === undefined) grouped.set(key, { role, width, data })
    else current.data = `${current.data} ${data}`
  }

  if (geometry.polygon.length > 0) {
    const [first, ...rest] = geometry.polygon
    if (first !== undefined) {
      append(
        decoration.forcedColors === true ? "focus" : "structural",
        theme.khalaUi.edgeWidth.structural,
        `${move(first.x, first.y)} ${rest.map((value) => segment(value.x, value.y)).join(" ")} Z`
      )
    }
  }
  for (const value of geometry.lines) {
    append(value.role, value.width, `${move(value.from.x, value.from.y)} ${segment(value.to.x, value.to.y)}`)
  }

  return {
    id: stableKhalaDomId(decoration.id),
    geometry,
    paths: [...grouped.values()].map((value, index) => ({
      id: `${stableKhalaDomId(decoration.id)}-path-${index}`,
      role: value.role,
      color: theme.khalaUi.luminance[value.role],
      width: value.width,
      data: value.data
    }))
  }
}
