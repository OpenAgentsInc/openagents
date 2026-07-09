import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

describe("retired Khala Code Desktop release boundary", () => {
  test("package release commands fail through the retirement guard", () => {
    const packageJson = JSON.parse(
      readFileSync(join(import.meta.dir, "..", "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> }

    expect(packageJson.scripts?.["build:rc"]).toBe(
      "bun run build:ui && electrobun build --env=rc",
    )
    expect(packageJson.scripts?.["build:stable"]).toBe(
      "bun run build:ui && electrobun build --env=stable",
    )
    expect(packageJson.scripts?.["release:plan"]).toBe(
      "bun scripts/retired-release-guard.ts",
    )
    expect(packageJson.scripts?.["release:macos"]).toBe(
      "bun scripts/retired-release-guard.ts",
    )
  })

  test("Electrobun updates point at the Khala product feed", () => {
    const configSource = readFileSync(
      join(import.meta.dir, "..", "electrobun.config.ts"),
      "utf8",
    )

    expect(configSource).toContain(
      'baseUrl: "https://updates.openagents.com/desktop/khala-code-desktop"',
    )
  })

  test("the directly invoked historical macOS script also fails closed", () => {
    const script = readFileSync(
      join(import.meta.dir, "..", "scripts", "release-macos.sh"),
      "utf8",
    )
    expect(script).toContain("legacy release writes are disabled")
    expect(script.indexOf("exit 78")).toBeLessThan(
      script.indexOf('APP_DIR="$(cd'),
    )
  })
})
