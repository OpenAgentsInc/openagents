import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Build check (#8574): the three real artifacts — ESM Electron main, CommonJS
 * sandboxed preload, and the bundled Effect Native renderer — build from
 * source in the test sweep, so a broken bundle can never reach `bun run dev`.
 */
import { describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { Worker } from "node:worker_threads"

import {
  REQUIRED_ARTIFACTS,
  checkArtifactSet,
  checkNoLegacyUiEntrypoints,
  checkNoSourceCheckoutPaths,
  checkNoUpdaterRemnants,
  gatherArtifactFiles,
} from "../scripts/release-preflight.ts"

const appRoot = path.resolve(import.meta.dirname, "..")

describe("openagents-desktop build", () => {
  test("bundles main, workers, preload, and the EN renderer into dist/", async () => {
    // Run the real build in its own process with the app as cwd — module
    // resolution for the vendored EN workspace packages is cwd-sensitive
    // when Runtime.build runs inside `bun test` from the repo root.
    const result = Runtime.spawnSync(["node", "--import", "tsx", "scripts/build.ts"], {
      cwd: appRoot,
      stdout: "pipe",
      stderr: "pipe",
    })
    expect(result.stderr.toString()).toBe("")
    expect(result.exitCode).toBe(0)

    const dist = path.join(appRoot, "dist")

    for (const artifact of [
      "main.js",
      "workers/codex-history-worker.js",
      "workers/workspace-search-worker.js",
      "preload.cjs",
      "renderer/boot.js",
      "renderer/index.html",
      "renderer/app.css",
      "assets/openagents-icon.png",
      "builtin-skills/manifest.json",
      "builtin-skills/productspec-work/SKILL.md",
      "builtin-skills/assurancespec-work/SKILL.md",
      ...(process.platform === "darwin" ? [
        `native/${process.arch}/oa-desktop-audio`,
        `native/${process.arch}/manifest.json`,
      ] : []),
    ]) {
      expect(existsSync(path.join(dist, artifact))).toBe(true)
    }

    // Keep the real-artifact release oracles in the one test that owns the
    // destructive dist/ build. A second test file spawning the same build can
    // race this process between rm(dist) and asset staging.
    const present = REQUIRED_ARTIFACTS.filter(artifact => existsSync(path.join(dist, artifact)))
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

    if (process.platform === "darwin") {
      const helper = path.join(dist, "native", process.arch, "oa-desktop-audio")
      const manifest = JSON.parse(readFileSync(path.join(dist, "native", process.arch, "manifest.json"), "utf8")) as Record<string, unknown>
      expect(statSync(helper).mode & 0o111).not.toBe(0)
      expect(manifest).toEqual({
        protocolVersion: 1,
        helperVersion: "0.1.0",
        architecture: process.arch,
        sha256: createHash("sha256").update(readFileSync(helper)).digest("hex"),
      })
    }

    // Electron stays external in the main bundle; the renderer bundle carries
    // the vendored EN catalog and never references Electron. Match is
    // whitespace-tolerant so the minified build (scripts/build.ts `minify`,
    // which drops the space in `from "electron"`) still proves externalization.
    const main = readFileSync(path.join(dist, "main.js"), "utf8")
    expect(main).toMatch(/from\s*"electron"/)
    const mainSource = readFileSync(path.join(appRoot, "src", "main.ts"), "utf8")
    expect(mainSource).toContain(': path.join(here, "builtin-skills")')
    expect(mainSource).not.toContain(': path.join(here, "..", "builtin-skills")')

    const renderer = readFileSync(path.join(dist, "renderer/boot.js"), "utf8")
    expect(renderer).not.toMatch(/require\(\s*"electron"\s*\)/)
    expect(renderer).toContain("openagents-desktop-root")
    expect(renderer).toContain("data-en-react-surface")
    expect(renderer).toContain("data-en-react-backend")
    expect(renderer).toContain("compatibility")
    expect(renderer).not.toContain("process.env.NODE_ENV")
    expect(renderer).not.toContain("react-dom-client.development.js")

    const rendererCss = readFileSync(path.join(dist, "renderer/app.css"), "utf8")
    expect(rendererCss).toContain("[data-en-react-surface=true]{background-color:var(--en-color-background);color:var(--en-color-textPrimary)}")
    expect(rendererCss).not.toContain(".shadow{")
    expect(rendererCss).not.toContain(".text-\\[")

    const fingerprint = (icon: Buffer) => createHash("sha256").update(icon).digest("hex")
    const mobileIcon = readFileSync(path.join(appRoot, "..", "openagents-mobile", "assets", "images", "icon.png"))
    const desktopIcon = readFileSync(path.join(dist, "assets", "openagents-icon.png"))
    expect(fingerprint(desktopIcon)).toBe(fingerprint(mobileIcon))

    const builtSkillRoot = path.join(dist, "builtin-skills")
    const builtSkillManifest = JSON.parse(
      readFileSync(path.join(builtSkillRoot, "manifest.json"), "utf8"),
    ) as { skills: Array<{ name: string; sha256: string }> }
    for (const name of ["productspec-work", "assurancespec-work"]) {
      const builtSkill = readFileSync(path.join(builtSkillRoot, name, "SKILL.md"))
      expect(builtSkillManifest.skills).toContainEqual(expect.objectContaining({
        name,
        sha256: createHash("sha256").update(builtSkill).digest("hex"),
      }))
    }

    const workspaceRoot = mkdtempSync(path.join(tmpdir(), "openagents-workspace-search-build-"))
    try {
      writeFileSync(path.join(workspaceRoot, "README.md"), "built worker needle")
      const workerResult = await new Promise<unknown>((resolve, reject) => {
        const worker = new Worker(path.join(dist, "workers", "workspace-search-worker.js"), {
          workerData: {
            root: workspaceRoot,
            grantRef: "workspace.grant.build-test",
            request: { query: "needle", mode: "content", epoch: 7 },
          },
        })
        worker.once("message", resolve)
        worker.once("error", reject)
      })
      expect(workerResult).toMatchObject({
        ok: true,
        result: {
          state: "available",
          grantRef: "workspace.grant.build-test",
          matches: [{ pathRef: "README.md", kind: "content", line: 1 }],
          cache: { epoch: 7, freshness: "current" },
        },
      })
      expect(JSON.stringify(workerResult)).not.toContain(workspaceRoot)
    } finally {
      rmSync(workspaceRoot, { recursive: true, force: true })
    }
  }, 60_000)
})
