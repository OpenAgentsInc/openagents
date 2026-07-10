/**
 * Build check (#8574): the three real artifacts — ESM Electron main, CommonJS
 * sandboxed preload, and the bundled Effect Native renderer — build from
 * source in the test sweep, so a broken bundle can never reach `bun run dev`.
 */
import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { existsSync, readFileSync } from "node:fs"
import path from "node:path"

const appRoot = path.resolve(import.meta.dir, "..")

describe("openagents-desktop build", () => {
  test("bundles main, preload, and the EN renderer into dist/", () => {
    // Run the real build in its own process with the app as cwd — module
    // resolution for the vendored EN workspace packages is cwd-sensitive
    // when Bun.build runs inside `bun test` from the repo root.
    const result = Bun.spawnSync([process.execPath, "scripts/build.ts"], {
      cwd: appRoot,
      stdout: "pipe",
      stderr: "pipe",
    })
    expect(result.stderr.toString()).toBe("")
    expect(result.exitCode).toBe(0)

    const dist = path.join(appRoot, "dist")

    for (const artifact of [
      "main.js",
      "preload.cjs",
      "renderer/boot.js",
      "renderer/index.html",
      "renderer/app.css",
      "assets/openagents-icon.png",
    ]) {
      expect(existsSync(path.join(dist, artifact))).toBe(true)
    }

    // Electron stays external in the main bundle; the renderer bundle carries
    // the vendored EN catalog and never references Electron.
    const main = readFileSync(path.join(dist, "main.js"), "utf8")
    expect(main).toContain('from "electron"')

    const renderer = readFileSync(path.join(dist, "renderer/boot.js"), "utf8")
    expect(renderer).not.toContain('require("electron")')
    expect(renderer).toContain("openagents-desktop-root")

    const fingerprint = (icon: Buffer) => createHash("sha256").update(icon).digest("hex")
    const mobileIcon = readFileSync(path.join(appRoot, "..", "openagents-mobile", "assets", "images", "icon.png"))
    const desktopIcon = readFileSync(path.join(dist, "assets", "openagents-icon.png"))
    expect(fingerprint(desktopIcon)).toBe(fingerprint(mobileIcon))
  }, 60_000)
})
