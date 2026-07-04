import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

describe("Khala Code Desktop macOS release script", () => {
  test("package scripts expose build channels and owner-run release lane", () => {
    const packageJson = JSON.parse(
      readFileSync(join(import.meta.dir, "..", "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> }

    expect(packageJson.scripts?.["build:rc"]).toBe(
      "bun run build:ui && electrobun build --env=rc",
    )
    expect(packageJson.scripts?.["build:stable"]).toBe(
      "bun run build:ui && electrobun build --env=stable",
    )
    expect(packageJson.scripts?.["release:plan"]).toBe("bun scripts/release-plan.ts")
    expect(packageJson.scripts?.["release:macos"]).toBe("bash scripts/release-macos.sh")
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

  test("macOS release script preserves the signed app -> recut DMG -> prerelease flow", () => {
    const script = readFileSync(
      join(import.meta.dir, "..", "scripts", "release-macos.sh"),
      "utf8",
    )
    const autopilotNotaryScript = readFileSync(
      join(
        import.meta.dir,
        "..",
        "..",
        "..",
        "apps",
        "autopilot-desktop",
        "scripts",
        "notarize-macos.sh",
      ),
      "utf8",
    )

    expect(script).toContain("scripts/release-plan.ts")
    expect(script).toContain("build:$CHANNEL")
    expect(script).toContain("apps/autopilot-desktop/scripts/notarize-macos.sh")
    expect(autopilotNotaryScript).toContain("codesign --force --deep --options runtime")
    expect(autopilotNotaryScript).toContain("xcrun notarytool submit")
    expect(autopilotNotaryScript).toContain("--wait")
    expect(autopilotNotaryScript).toContain("xcrun stapler staple")
    expect(script).toContain("hdiutil create")
    expect(script).toContain("codesign --force --timestamp --sign")
    expect(script).toContain('xcrun notarytool submit "$DMG_PATH"')
    expect(script).toContain("xcrun stapler staple")
    expect(script).toContain("--product khala-code-desktop")
    expect(script).toContain("--channel \"$CHANNEL\"")
    expect(script).toContain("gs://openagentsgemini-oa-updates/desktop/khala-code-desktop/$CHANNEL/")
    expect(script).toContain("--prerelease")
    expect(script).toContain("--latest=false")
  })
})
