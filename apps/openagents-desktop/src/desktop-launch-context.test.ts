import { describe, expect, test } from "vite-plus/test"

import {
  desktopDocumentOpenRendererArgument,
  desktopLaunchContextFromArgv,
} from "./desktop-launch-context.ts"

describe("desktop launch context", () => {
  test("round-trips one bounded relative Finder-open filename", () => {
    const argument = desktopDocumentOpenRendererArgument("startup file.tsx")
    expect(desktopLaunchContextFromArgv(["electron", argument])).toEqual({
      documentOpenPathRef: "startup file.tsx",
    })
  })

  test("rejects absolute, nested, malformed, and duplicate-authority arguments", () => {
    expect(desktopLaunchContextFromArgv(["--openagents-document-open=%2Fprivate%2Ftmp%2Fsecret.ts"]))
      .toEqual({ documentOpenPathRef: null })
    expect(desktopLaunchContextFromArgv(["--openagents-document-open=src%2Fsecret.ts"]))
      .toEqual({ documentOpenPathRef: null })
    expect(desktopLaunchContextFromArgv(["--openagents-document-open=%E0%A4%A"]))
      .toEqual({ documentOpenPathRef: null })
    expect(desktopLaunchContextFromArgv([])).toEqual({ documentOpenPathRef: null })
  })
})
