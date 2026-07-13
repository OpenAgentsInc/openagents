/**
 * Build (#8574): bundles the Electron main process, the sandboxed CommonJS
 * preload, and the Effect Native renderer into `dist/` with Bun. Plain
 * TypeScript in, three artifacts out — no Vite/Forge pipeline in this exit
 * (packaging/signing is a later #8574 exit; see UPSTREAM.md).
 */
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { chmodSync, readFileSync, writeFileSync } from "node:fs"
import { cp, copyFile, mkdir, rename, rm } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
// Minification is on by default; set OA_DESKTOP_BUILD_MINIFY=0 for an A/B
// unminified build (startup-bench comparison). See the startup-speed audit.
const BUILD_MINIFY = process.env.OA_DESKTOP_BUILD_MINIFY !== "0"

const stageDevelopmentVoiceHelper = async (dist: string): Promise<void> => {
  if (process.platform !== "darwin") return
  const workspaceRoot = path.resolve(appRoot, "../..")
  execFileSync("cargo", ["build", "-p", "oa-desktop-audio"], { cwd: workspaceRoot, stdio: "pipe" })
  const destinationDirectory = path.join(dist, "native", process.arch)
  const destination = path.join(destinationDirectory, "oa-desktop-audio")
  await mkdir(destinationDirectory, { recursive: true })
  await copyFile(path.join(workspaceRoot, "target", "debug", "oa-desktop-audio"), destination)
  chmodSync(destination, 0o755)
  const sha256 = createHash("sha256").update(readFileSync(destination)).digest("hex")
  writeFileSync(
    path.join(destinationDirectory, "manifest.json"),
    JSON.stringify({ protocolVersion: 1, helperVersion: "0.1.0", architecture: process.arch, sha256 }) + "\n",
    { mode: 0o644 },
  )
}

const assertSuccess = (label: string, result: Awaited<ReturnType<typeof Bun.build>>): void => {
  if (!result.success) {
    for (const log of result.logs) console.error(`[build:${label}]`, log)
    throw new Error(`openagents-desktop build failed: ${label}`)
  }
}

export const buildDesktop = async (): Promise<string> => {
  const dist = path.join(appRoot, "dist")
  await rm(dist, { recursive: true, force: true })
  await mkdir(path.join(dist, "renderer"), { recursive: true })

  assertSuccess(
    "main",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/main.ts")],
      outdir: dist,
      target: "node",
      format: "esm",
      // Minify every artifact (startup-speed optimization; see
      // docs/fable/2026-07-11-desktop-startup-speed-audit.md). The renderer
      // (~3.6 MB unminified IIFE) and preload sit on the first-paint critical
      // path, and the main bundle's parse precedes app.whenReady — smaller
      // bytes parse faster. Measured with `bun run startup-bench`.
      minify: BUILD_MINIFY,
      // The Claude Agent SDK must stay external: it resolves its bundled
      // native `claude` executable relative to its own installed package
      // (require.resolve from sdk.mjs) and is lazy-imported only when the
      // Fable local lane actually runs a turn (#8712). The delegate tool's
      // zod raw shape is resolved through the SDK's OWN installed zod
      // (createRequire from the SDK entry) — zod is deliberately NOT an app
      // dependency (boundary law) and never enters the renderer.
      external: ["electron", "@anthropic-ai/claude-agent-sdk"],
    }),
  )

  assertSuccess(
    "codex-history-worker",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/codex-history-worker.ts")],
      outdir: path.join(dist, "workers"),
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
    }),
  )

  assertSuccess(
    "workspace-search-worker",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/workspace-search-worker.ts")],
      outdir: path.join(dist, "workers"),
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
    }),
  )

  assertSuccess(
    "preload",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/preload.cts")],
      outdir: dist,
      target: "node",
      format: "cjs",
      minify: BUILD_MINIFY,
      external: ["electron"],
    }),
  )
  // Sandboxed preloads must be CommonJS; the package is type:module, so the
  // artifact needs the explicit .cjs extension.
  await rename(path.join(dist, "preload.js"), path.join(dist, "preload.cjs"))

  assertSuccess(
    "renderer",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/renderer/boot.ts")],
      outdir: path.join(dist, "renderer"),
      target: "browser",
      format: "iife",
      minify: BUILD_MINIFY,
    }),
  )

  await cp(path.join(appRoot, "index.html"), path.join(dist, "renderer/index.html"))
  await cp(path.join(appRoot, "src/renderer/app.css"), path.join(dist, "renderer/app.css"))
  // Packaged smoke cannot reach the source checkout (and must not depend on
  // it). Keep the bounded public fixtures beside the already-unpacked static
  // renderer; main copies them into the per-run writable userData directory.
  const smokeFixtures = path.join(dist, "renderer", "smoke-fixtures")
  await mkdir(smokeFixtures, { recursive: true })
  for (const fixture of ["codex-smoke", "claude-smoke"]) {
    await cp(path.join(appRoot, "tests", "fixtures", fixture), path.join(smokeFixtures, fixture), {
      recursive: true,
    })
  }
  // Consume the checked-in mobile source icon rather than maintaining an
  // approximate sibling asset. A future macOS package can derive `.icns` from
  // this same PNG without introducing a second brand source.
  await mkdir(path.join(dist, "assets"), { recursive: true })
  await cp(
    path.join(appRoot, "..", "openagents-mobile", "assets", "images", "icon.png"),
    path.join(dist, "assets", "openagents-icon.png"),
  )
  // `electron .` resolves the native voice runtime from dist/, just as the
  // packaged app resolves it from Resources. The everyday development build
  // must therefore stage a real helper too; otherwise voice.start can only
  // fail after the UI reaches "Connecting voice".
  await stageDevelopmentVoiceHelper(dist)
  return dist
}

if (import.meta.main) {
  await buildDesktop()
  console.log("[openagents-desktop] built dist/ (main.js, preload.cjs, workers, renderer, native voice helper)")
}
