// Theme tokens for the Pylon TUI (issue #4737).
//
// Ports the shape of opencode's theme JSON (`defs` palette + semantic slots,
// ref-name resolution) with a single `openagents` theme. Every color in the
// view layer comes through these tokens — inline parseColor literals in
// component code are forbidden.

import { parseColor, SyntaxStyle, type RGBA } from "@opentui/core"

export type PylonThemeJson = {
  name: string
  defs: Record<string, string>
  theme: {
    border: string
    title: string
    text: string
    textMuted: string
    separator: string
    accent: string
    online: string
    error: string
    logText: string
    logError: string
    banner: string
  }
  syntax: Record<string, { ref: string; bold?: boolean; italic?: boolean; underline?: boolean; bgRef?: string }>
}

export const openagentsThemeJson: PylonThemeJson = {
  name: "openagents",
  defs: {
    skyBlue: "#73C2FB",
    iceBlue: "#D7E5FA",
    slate: "#5C7080",
    deepBlue: "#3B5B82",
    cyan: "#66D9EF",
    linkBlue: "#58A6FF",
    red: "#EF4444",
    fgDefault: "#E6EDF3",
    fgBright: "#F0F6FC",
    coral: "#FF7B72",
    lightBlue: "#A5D6FF",
    gray: "#8B949E",
    blue: "#79C0FF",
    purple: "#D2A8FF",
    orange: "#FFA657",
    cyanBright: "#00D7FF",
    bgRaw: "#161B22",
    grayDim: "#6E7681",
  },
  theme: {
    border: "skyBlue",
    title: "skyBlue",
    text: "iceBlue",
    textMuted: "slate",
    separator: "deepBlue",
    accent: "cyan",
    online: "linkBlue",
    error: "red",
    logText: "slate",
    logError: "red",
    banner: "deepBlue",
  },
  syntax: {
    default: { ref: "fgDefault" },
    keyword: { ref: "coral", bold: true },
    string: { ref: "lightBlue" },
    comment: { ref: "gray", italic: true },
    number: { ref: "blue" },
    function: { ref: "purple" },
    type: { ref: "orange" },
    variable: { ref: "fgDefault" },
    property: { ref: "blue" },
    "markup.heading": { ref: "cyanBright", bold: true },
    "markup.bold": { ref: "fgBright", bold: true },
    "markup.italic": { ref: "fgBright", italic: true },
    "markup.list": { ref: "coral" },
    "markup.quote": { ref: "gray", italic: true },
    "markup.raw": { ref: "lightBlue", bgRef: "bgRaw" },
    "markup.link": { ref: "linkBlue", underline: true },
    "markup.link.url": { ref: "linkBlue", underline: true },
    conceal: { ref: "grayDim" },
  },
}

export type PylonThemeColorSlot = keyof PylonThemeJson["theme"]

export interface PylonTheme {
  name: string
  colors: Record<PylonThemeColorSlot, RGBA>
  syntaxStyle: SyntaxStyle
}

export function resolveThemeHex(json: PylonThemeJson, slot: PylonThemeColorSlot): string {
  const ref = json.theme[slot]
  const hex = json.defs[ref]
  if (!hex) throw new Error(`theme "${json.name}": slot "${slot}" references unknown def "${ref}"`)
  return hex
}

export function resolveTheme(json: PylonThemeJson): PylonTheme {
  const colors = Object.fromEntries(
    (Object.keys(json.theme) as PylonThemeColorSlot[]).map((slot) => [
      slot,
      parseColor(resolveThemeHex(json, slot)),
    ]),
  ) as Record<PylonThemeColorSlot, RGBA>

  const syntaxEntries = Object.fromEntries(
    Object.entries(json.syntax).map(([scope, def]) => {
      const hex = json.defs[def.ref]
      if (!hex) throw new Error(`theme "${json.name}": syntax scope "${scope}" references unknown def "${def.ref}"`)
      const bgHex = def.bgRef ? json.defs[def.bgRef] : undefined
      if (def.bgRef && !bgHex) {
        throw new Error(`theme "${json.name}": syntax scope "${scope}" references unknown bg def "${def.bgRef}"`)
      }
      return [
        scope,
        {
          fg: parseColor(hex),
          ...(bgHex ? { bg: parseColor(bgHex) } : {}),
          ...(def.bold ? { bold: true } : {}),
          ...(def.italic ? { italic: true } : {}),
          ...(def.underline ? { underline: true } : {}),
        },
      ]
    }),
  )

  return {
    name: json.name,
    colors,
    syntaxStyle: SyntaxStyle.fromStyles(syntaxEntries),
  }
}

export const theme: PylonTheme = resolveTheme(openagentsThemeJson)
