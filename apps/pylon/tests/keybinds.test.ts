import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"

import { keybindsFileName, loadKeybindOverrides, parseKeybindsConfig } from "../src/node/keybinds.js"

const homes: string[] = []

async function tempHome(): Promise<string> {
  const home = await mkdtemp(join(tmpdir(), "pylon-keybinds-"))
  homes.push(home)
  return home
}

afterEach(async () => {
  await Promise.all(homes.splice(0).map(home => rm(home, { force: true, recursive: true })))
})

describe("Pylon keybind config boundary", () => {
  test("parses keybind overrides through Effect Schema", () => {
    expect(parseKeybindsConfig('{"bindings":{"palette.open":"ctrl+p"}}')).toEqual({
      "palette.open": "ctrl+p",
    })
  })

  test("reports malformed local state as invalid", async () => {
    const home = await tempHome()
    await mkdir(home, { recursive: true })
    await writeFile(join(home, keybindsFileName), "{bad")

    const result = await loadKeybindOverrides(home)

    expect(result.state).toBe("invalid")
    expect(result.overrides).toEqual({})
    expect(result.error).toContain("Boundary JSON could not be parsed")
  })
})
