import { readFileSync } from "node:fs"
import path from "node:path"

import { describe, expect, it } from "vite-plus/test"

const css = readFileSync(
  path.resolve(import.meta.dirname, "../desktop-workbench.css"),
  "utf8",
)

describe("DesktopComposerFrame focus presentation", () => {
  it("keeps one visible focus frame around native inputs in every host", () => {
    const outerFocusRule = css.match(
      /\.oa-react-composer:focus-within\s*\{([\s\S]*?)\n\}/,
    )?.[1]
    const nestedInputRule = css.match(
      /\.oa-react-composer-input textarea:focus,[\s\S]*?\{([\s\S]*?)\n\}/,
    )?.[1]

    expect(outerFocusRule).toContain("border-color: var(--en-color-accent)")
    expect(outerFocusRule).toContain("box-shadow: 0 0 0 1px var(--en-color-focus)")
    expect(nestedInputRule).toContain("outline: none !important")
    expect(nestedInputRule).toContain("box-shadow: none !important")
  })
})
