/**
 * Release-preflight oracles (CUT-26, #8706).
 *
 * Two layers:
 *  1. Pure-check unit oracles over synthetic inputs — every check must both
 *     pass on good input and FAIL on the specific regression it guards.
 *  2. A real-artifact sweep over the actual built `dist/` (building it if
 *     absent) plus the real UPSTREAM.md/package.json, so a legacy-UI asset,
 *     template-updater remnant, or source-checkout path entering the artifact
 *     turns the normal test sweep red — not just a release-day script.
 */
import { describe, expect, test } from "bun:test"
import { existsSync, readFileSync } from "node:fs"
import path from "node:path"
import {
  FORBIDDEN_LEGACY_UI_MARKERS,
  FORBIDDEN_UPDATER_MARKERS,
  REQUIRED_ARTIFACTS,
  UPSTREAM_PINNED_COMMIT,
  checkAppIdentity,
  checkArtifactSet,
  checkAttributionIntact,
  checkCleanOriginMain,
  checkNoLegacyUiEntrypoints,
  checkNoSourceCheckoutPaths,
  checkNoUpdaterRemnants,
  checkVersionMonotonic,
  gatherArtifactFiles,
} from "../scripts/release-preflight.ts"

const appRoot = path.resolve(import.meta.dir, "..")

describe("pure preflight checks", () => {
  test("clean origin/main passes; dirty tree or drifted HEAD fails", () => {
    const clean = checkCleanOriginMain({
      statusPorcelain: "",
      headSha: "abc123\n",
      originMainSha: "abc123\n",
    })
    expect(clean.ok).toBe(true)

    expect(
      checkCleanOriginMain({
        statusPorcelain: " M src/main.ts\n",
        headSha: "abc123",
        originMainSha: "abc123",
      }).ok,
    ).toBe(false)

    expect(
      checkCleanOriginMain({
        statusPorcelain: "",
        headSha: "abc123",
        originMainSha: "def456",
      }).ok,
    ).toBe(false)

    expect(
      checkCleanOriginMain({ statusPorcelain: "", headSha: "", originMainSha: "" }).ok,
    ).toBe(false)
  })

  test("version monotonicity: strict upgrade passes, downgrade/equal/rc-on-stable fail", () => {
    expect(
      checkVersionMonotonic({ candidate: "0.1.1", latestReleased: "0.1.0", channel: "stable" }).ok,
    ).toBe(true)
    expect(
      checkVersionMonotonic({ candidate: "0.1.0", latestReleased: "0.1.0", channel: "stable" }).ok,
    ).toBe(false)
    expect(
      checkVersionMonotonic({ candidate: "0.0.9", latestReleased: "0.1.0", channel: "rc" }).ok,
    ).toBe(false)
    expect(
      checkVersionMonotonic({ candidate: "0.2.0-rc.1", latestReleased: "0.1.0", channel: "stable" }).ok,
    ).toBe(false)
    expect(
      checkVersionMonotonic({ candidate: "0.2.0-rc.1", latestReleased: "0.1.0", channel: "rc" }).ok,
    ).toBe(true)
    expect(
      checkVersionMonotonic({ candidate: "not-a-version", latestReleased: null, channel: "rc" }).ok,
    ).toBe(false)
    // First release: candidate must still parse.
    expect(
      checkVersionMonotonic({ candidate: "0.0.1", latestReleased: null, channel: "rc" }).ok,
    ).toBe(true)
  })

  test("attribution check requires MIT + upstream repo + pinned commit", () => {
    const good = `MIT ... LuanRoger/electron-shadcn ... ${UPSTREAM_PINNED_COMMIT}`
    expect(checkAttributionIntact(good).ok).toBe(true)
    expect(checkAttributionIntact(good.replace("MIT", "")).ok).toBe(false)
    expect(checkAttributionIntact(good.replace(UPSTREAM_PINNED_COMMIT, "deadbeef")).ok).toBe(false)
    expect(checkAttributionIntact("").ok).toBe(false)
  })

  test("app identity check pins name, productName, entry, and a parseable version", () => {
    const good = {
      name: "@openagentsinc/openagents-desktop",
      productName: "OpenAgents",
      main: "dist/main.js",
      version: "0.0.1",
    }
    expect(checkAppIdentity(good).ok).toBe(true)
    expect(checkAppIdentity({ ...good, productName: "Khala Code" }).ok).toBe(false)
    expect(checkAppIdentity({ ...good, main: "src/main.ts" }).ok).toBe(false)
    expect(checkAppIdentity({ ...good, version: "0.0" }).ok).toBe(false)
  })

  test("artifact-set check reports missing artifacts", () => {
    expect(checkArtifactSet([...REQUIRED_ARTIFACTS]).ok).toBe(true)
    const withoutPreload = REQUIRED_ARTIFACTS.filter((artifact) => artifact !== "preload.cjs")
    const result = checkArtifactSet(withoutPreload)
    expect(result.ok).toBe(false)
    expect(result.detail).toContain("preload.cjs")
    const withoutBuiltinSkill = REQUIRED_ARTIFACTS.filter(
      artifact => artifact !== "builtin-skills/productspec-work/SKILL.md",
    )
    const missingSkill = checkArtifactSet(withoutBuiltinSkill)
    expect(missingSkill.ok).toBe(false)
    expect(missingSkill.detail).toContain("builtin-skills/productspec-work/SKILL.md")
  })

  test("updater-remnant oracle fails on every forbidden marker", () => {
    expect(checkNoUpdaterRemnants([{ relativePath: "main.js", text: "clean bundle" }]).ok).toBe(true)
    for (const marker of FORBIDDEN_UPDATER_MARKERS) {
      const result = checkNoUpdaterRemnants([
        { relativePath: "main.js", text: `something ${marker} something` },
      ])
      expect(result.ok).toBe(false)
      expect(result.detail).toContain(marker)
    }
  })

  test("legacy-UI oracle fails on every forbidden marker", () => {
    expect(checkNoLegacyUiEntrypoints([{ relativePath: "boot.js", text: "clean" }]).ok).toBe(true)
    for (const marker of FORBIDDEN_LEGACY_UI_MARKERS) {
      const result = checkNoLegacyUiEntrypoints([
        { relativePath: "renderer/boot.js", text: `import "${marker}/legacy"` },
      ])
      expect(result.ok).toBe(false)
    }
  })

  test("source-checkout-path oracle fails on absolute developer paths and the repo root", () => {
    const repoRoot = "/repo/openagents"
    expect(
      checkNoSourceCheckoutPaths([{ relativePath: "main.js", text: "relative only" }], repoRoot).ok,
    ).toBe(true)
    for (const bad of ["/Users/someone/work/x.ts", "/home/ci/checkout", `${repoRoot}/apps/x`]) {
      expect(
        checkNoSourceCheckoutPaths([{ relativePath: "main.js", text: `path: ${bad}` }], repoRoot).ok,
      ).toBe(false)
    }
  })
})

describe("real artifact sweep", () => {
  test("the actual built dist/ carries no updater remnants, legacy UI, or checkout paths", () => {
    const dist = path.join(appRoot, "dist")
    if (!existsSync(path.join(dist, "main.js"))) {
      const result = Bun.spawnSync([process.execPath, "scripts/build.ts"], {
        cwd: appRoot,
        stdout: "pipe",
        stderr: "pipe",
      })
      expect(result.exitCode).toBe(0)
    }

    const present = REQUIRED_ARTIFACTS.filter((artifact) => existsSync(path.join(dist, artifact)))
    expect(checkArtifactSet(present).ok).toBe(true)

    const files = gatherArtifactFiles(dist)
    expect(files.length).toBeGreaterThanOrEqual(7)

    const repoRoot = path.resolve(appRoot, "../..")
    for (const check of [
      checkNoUpdaterRemnants(files),
      checkNoLegacyUiEntrypoints(files),
      checkNoSourceCheckoutPaths(files, repoRoot),
    ]) {
      expect({ id: check.id, ok: check.ok, detail: check.ok ? "" : check.detail }).toEqual({
        id: check.id,
        ok: true,
        detail: "",
      })
    }
  })

  test("the real UPSTREAM.md attribution and package identity are intact", () => {
    const upstreamMd = readFileSync(path.join(appRoot, "UPSTREAM.md"), "utf8")
    expect(checkAttributionIntact(upstreamMd).ok).toBe(true)

    const packageJson = JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8"))
    expect(checkAppIdentity(packageJson).ok).toBe(true)
  })
})
