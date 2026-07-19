import { readFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { describe, expect, test } from "vite-plus/test"

const read = (relative: string): Promise<string> =>
  readFile(new URL(relative, import.meta.url), "utf8")

describe("Desktop Pierre Trees package boundary", () => {
  test("pins the audited beta and keeps production imports behind one owned adapter", async () => {
    const appPackage = JSON.parse(await read("../package.json")) as { dependencies?: Record<string, string> }
    const [adapter, surface] = await Promise.all([
      read("../src/renderer/ide/pierre-tree-adapter.tsx"),
      read("../src/renderer/react-workspace-surfaces.tsx"),
    ])

    expect(appPackage.dependencies?.["@pierre/trees"]).toBe("1.0.0-beta.5")
    expect(adapter).toContain('from "@pierre/trees/react"')
    expect(adapter).not.toContain("@pierre/path-store")
    expect(adapter).not.toContain("unsafeCSS")
    expect(surface).toContain('from "./ide/pierre-tree-adapter.tsx"')
    expect(surface).not.toContain('from "@pierre/trees')
  })

  test("retains the Apache license and upstream NOTICE in the installed package closure", async () => {
    const entry = fileURLToPath(import.meta.resolve("@pierre/trees"))
    const packageRoot = path.resolve(path.dirname(entry), "..")
    const [license, notice] = await Promise.all([
      readFile(path.join(packageRoot, "LICENSE.md"), "utf8"),
      readFile(path.join(packageRoot, "NOTICE.md"), "utf8"),
    ])

    expect(license).toContain("Apache License")
    expect(license).toContain("Copyright 2025 Pierre Computer Company")
    expect(notice).toContain("headless-tree/core")
    expect(notice).toContain("MIT License")
  })
})
