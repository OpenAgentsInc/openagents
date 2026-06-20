import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

const desktopSourceFiles = [
  "src/shared/hud-skin.ts",
  "src/ui/helpers.ts",
  "src/ui/hud-status-scene.ts",
  "src/ui/index.html",
  "src/ui/styles.css",
  "src/ui/view.ts",
] as const

const bannedHudFragments = [
  "#00f0ff",
  "#7dd3fc",
  "#4ca3ff",
  "#2979ff",
  "#0b5cad",
  "#0c1929",
  "#8dd3c7",
  "#58a6ff",
  "rgb(88 166 255",
  "rgb(141 211 199",
  "rgb(41 121 255",
  "rgb(214 246 255",
  "hud-corners",
  "createHudFrameCorners",
  "FrameCorners",
] as const

const bannedHudWords = /\b(?:cyan|teal|turquoise|aqua)\b/i

describe("desktop HUD style palette", () => {
  test("keeps desktop HUD chrome white and rectangular", () => {
    const offenders: string[] = []

    for (const file of desktopSourceFiles) {
      const contents = readFileSync(join(process.cwd(), file), "utf8")
      for (const fragment of bannedHudFragments) {
        if (contents.includes(fragment)) offenders.push(`${file}: ${fragment}`)
      }
      const wordMatch = contents.match(bannedHudWords)
      if (wordMatch !== null) offenders.push(`${file}: ${wordMatch[0]}`)
    }

    expect(offenders).toEqual([])
  })
})
