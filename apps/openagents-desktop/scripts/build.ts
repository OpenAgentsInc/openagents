import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Build (#8574): bundles the Electron main process, the sandboxed CommonJS
 * preload, and the Effect Native renderer into `dist/`. Runtime bundles host
 * code; Vite, React, and Tailwind CSS build the sandboxed renderer while
 * preserving the fixed signed asset names consumed by Electron.
 */
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { chmodSync, readFileSync, writeFileSync } from "node:fs"
import { cp, copyFile, mkdir, rename, rm } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { build as viteBuild } from "vite"
import { projectAssuranceSpecDocument } from "../src/assurance-spec-document.ts"
import { desktopRendererPlugins, desktopRendererResolve } from "../vite.config.ts"

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
// Minification is on by default; set OA_DESKTOP_BUILD_MINIFY=0 for an A/B
// unminified build (startup-bench comparison). See the startup-speed audit.
const BUILD_MINIFY = process.env.OA_DESKTOP_BUILD_MINIFY !== "0"

const stageDevelopmentVoiceHelper = async (dist: string): Promise<void> => {
  if (process.platform !== "darwin") return
  // Release staging (DIST-03 #8916) builds the target helper with an explicit
  // Rust triple into the staging workspace; the host-arch DEBUG helper is a
  // development convenience it never packages, so staging skips this step.
  if (process.env.OA_DESKTOP_SKIP_DEV_VOICE_HELPER === "1") return
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

const assertSuccess = (label: string, result: Awaited<ReturnType<typeof Runtime.build>>): void => {
  if (!result.success) {
    for (const log of result.logs) console.error(`[build:${label}]`, log)
    throw new Error(`openagents-desktop build failed: ${label}`)
  }
}

export const buildDesktop = async (): Promise<string> => {
  const dist = path.join(appRoot, "dist")
  const mvpAssuranceSpecSource = readFileSync(
    path.resolve(appRoot, "../..", "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md"),
    "utf8",
  )
  const mvpAssuranceSpecProjection = projectAssuranceSpecDocument(
    mvpAssuranceSpecSource,
    "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
  )
  if (mvpAssuranceSpecProjection.state !== "ready") {
    throw new Error("openagents-desktop build failed: checked-in MVP AssuranceSpec is invalid")
  }
  await rm(dist, { recursive: true, force: true })
  await mkdir(path.join(dist, "renderer"), { recursive: true })

  assertSuccess(
    "main",
    await Runtime.build({
      entrypoints: [path.join(appRoot, "src/main.ts")],
      outdir: dist,
      target: "node",
      format: "esm",
      // Minify every artifact (startup-speed optimization; see
      // docs/fable/2026-07-11-desktop-startup-speed-audit.md). The renderer
      // (~3.6 MB unminified IIFE) and preload sit on the first-paint critical
      // path, and the main bundle's parse precedes app.whenReady — smaller
      // bytes parse faster. Measured with `pnpm run startup-bench`.
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
    await Runtime.build({
      entrypoints: [path.join(appRoot, "src/codex-history-worker.ts")],
      outdir: path.join(dist, "workers"),
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
    }),
  )

  assertSuccess(
    "workspace-search-worker",
    await Runtime.build({
      entrypoints: [path.join(appRoot, "src/workspace-search-worker.ts")],
      outdir: path.join(dist, "workers"),
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
    }),
  )

  assertSuccess(
    "ide-language-utility-worker",
    await Runtime.build({
      entrypoints: [path.join(appRoot, "src/ide/language-utility-worker.ts")],
      outdir: path.join(dist, "workers"),
      target: "node",
      format: "esm",
      minify: BUILD_MINIFY,
      banner: 'import { fileURLToPath as __oaFileURLToPath } from "node:url";import { dirname as __oaDirname } from "node:path";const __filename=__oaFileURLToPath(import.meta.url);const __dirname=__oaDirname(__filename);',
    }),
  )

  assertSuccess(
    "preload",
    await Runtime.build({
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

  // Desktop now uses the same renderer toolchain shape as T3 Code: React 19,
  // Vite, and Tailwind CSS 4. Effect Native remains the application contract;
  // React is the DOM renderer beneath that typed View/intent boundary.
  // Vite 8 chooses its JSX dev/prod lowering from the host process environment
  // when resolving a programmatic build. Force production lowering so React
  // does not emit jsxDEV fileName metadata (and therefore local source paths)
  // into the signed renderer artifact. Restore the caller's environment after.
  const previousNodeEnv = process.env.NODE_ENV
  process.env.NODE_ENV = "production"
  try {
    await viteBuild({
      configFile: false,
      mode: "production",
      root: appRoot,
      plugins: desktopRendererPlugins(),
      // Workspace renderer sources have their own optional peer resolution;
      // force the app's one React/React DOM pair into the bundle.
      resolve: desktopRendererResolve,
      define: {
        // The sandboxed renderer has no Node `process`. Select React's
        // production bundles at build time instead of injecting a process shim.
        "process.env.NODE_ENV": JSON.stringify("production"),
        // Dogfood the exact checked-in proposal through the same browser-safe
        // parser used by future editor-opened `.assurance-spec.md` files. The
        // renderer component still owns no filesystem authority.
        __OPENAGENTS_MVP_ASSURANCE_SPEC_SNAPSHOT__: JSON.stringify(JSON.stringify(mvpAssuranceSpecProjection)),
      },
      build: {
        outDir: path.join(dist, "renderer"),
        emptyOutDir: false,
        minify: BUILD_MINIFY,
        reportCompressedSize: false,
        lib: {
          entry: path.join(appRoot, "src/renderer/boot.ts"),
          name: "OpenAgentsDesktopRenderer",
          formats: ["iife"],
          fileName: () => "boot.js",
          cssFileName: "app",
        },
      },
    })
    // IDE-03: the production Monaco graph is an independently fetched ESM
    // island. Chat-only launches never import it, while a Files/Finder open
    // resolves the fixed private-scheme entry and its local module workers.
    // Keeping a fixed entry/CSS name gives the loader an allowlistable target;
    // all transitive chunks and worker assets stay content-hashed below it.
    await viteBuild({
      configFile: false,
      mode: "production",
      root: appRoot,
      base: "./",
      plugins: desktopRendererPlugins(),
      resolve: desktopRendererResolve,
      define: {
        "process.env.NODE_ENV": JSON.stringify("production"),
      },
      build: {
        outDir: path.join(dist, "renderer", "ide-editor"),
        emptyOutDir: true,
        minify: BUILD_MINIFY,
        reportCompressedSize: false,
        sourcemap: true,
        lib: {
          entry: path.join(appRoot, "src", "ide", "editor-runtime-entry.ts"),
          formats: ["es"],
          fileName: () => "editor.js",
          cssFileName: "editor",
        },
        rollupOptions: {
          output: {
            chunkFileNames: "assets/[name]-[hash].js",
            assetFileNames: asset => asset.name === "editor.css"
              ? "editor.css"
              : "assets/[name]-[hash][extname]",
          },
        },
      },
    })
    if (process.env.OPENAGENTS_DESKTOP_IDE_PACKAGE_SPIKE_BUILD === "1") {
      // IDE-01's admission fixture is a separate, opt-in ESM graph. It proves
      // Monaco/Pierre workers and package assets without shipping the fixture
      // or placing an editor module on the ordinary chat-only boot path.
      const idePackageSpikeRoot = path.join(appRoot, "src", "ide", "spike")
      await viteBuild({
        configFile: false,
        mode: "production",
        root: idePackageSpikeRoot,
        base: "./",
        plugins: desktopRendererPlugins(),
        resolve: desktopRendererResolve,
        define: {
          "process.env.NODE_ENV": JSON.stringify("production"),
        },
        build: {
          outDir: path.join(dist, "renderer", "ide-package-spike"),
          emptyOutDir: true,
          minify: BUILD_MINIFY,
          reportCompressedSize: false,
          sourcemap: true,
          manifest: "manifest.json",
          rollupOptions: {
            output: {
              entryFileNames: "assets/[name]-[hash].js",
              chunkFileNames: "assets/[name]-[hash].js",
              assetFileNames: "assets/[name]-[hash][extname]",
            },
          },
        },
      })
    }
  } finally {
    if (previousNodeEnv === undefined) delete process.env.NODE_ENV
    else process.env.NODE_ENV = previousNodeEnv
  }

  await cp(path.join(appRoot, "index.html"), path.join(dist, "renderer/index.html"))
  // Product-owned built-in skills are signed application resources, not
  // ambient user/plugin content. Keep the checked-in manifest and immutable
  // asset together so packaging and release preflight can prove visibility.
  await cp(
    path.join(appRoot, "resources", "builtin-skills"),
    path.join(dist, "builtin-skills"),
    { recursive: true },
  )
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

if (Runtime.isMain(import.meta.url)) {
  await buildDesktop()
  console.log("[openagents-desktop] built dist/ (main.js, preload.cjs, workers, renderer, optional IDE package fixture, built-in skills, native voice helper)")
}
