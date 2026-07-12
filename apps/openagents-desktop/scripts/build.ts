/**
 * Build (#8574): bundles the Electron main process, the sandboxed CommonJS
 * preload, and the Effect Native renderer into `dist/` with Bun. Plain
 * TypeScript in, three artifacts out — no Vite/Forge pipeline in this exit
 * (packaging/signing is a later #8574 exit; see UPSTREAM.md).
 */
import { cp, mkdir, rename, rm } from "node:fs/promises"
import path from "node:path"

const appRoot = path.resolve(import.meta.dir, "..")
// Minification is on by default; set OA_DESKTOP_BUILD_MINIFY=0 for an A/B
// unminified build (startup-bench comparison). See the startup-speed audit.
const BUILD_MINIFY = process.env.OA_DESKTOP_BUILD_MINIFY !== "0"

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
      outdir: dist,
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
    }),
  )

  assertSuccess(
    "workspace-search-worker",
    await Bun.build({
      entrypoints: [path.join(appRoot, "src/workspace-search-worker.ts")],
      outdir: dist,
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
  // Consume the checked-in mobile source icon rather than maintaining an
  // approximate sibling asset. A future macOS package can derive `.icns` from
  // this same PNG without introducing a second brand source.
  await mkdir(path.join(dist, "assets"), { recursive: true })
  await cp(
    path.join(appRoot, "..", "openagents-mobile", "assets", "images", "icon.png"),
    path.join(dist, "assets", "openagents-icon.png"),
  )
  return dist
}

if (import.meta.main) {
  await buildDesktop()
  console.log("[openagents-desktop] built dist/ (main.js, preload.cjs, workers, renderer/)")
}
