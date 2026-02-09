import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig } from "vite"
import tsConfigPaths from "vite-tsconfig-paths"

const __dirname = path.dirname(fileURLToPath(import.meta.url))

const publicDecksDir = path.resolve(__dirname, "public/decks")
const outDecksDir = path.resolve(__dirname, "dist/effuse-client/decks")

function syncDecksFile(filename: string) {
  const src = path.join(publicDecksDir, filename)
  const rel = path.relative(publicDecksDir, src)
  if (rel.startsWith("..") || path.isAbsolute(rel)) return
  if (!fs.existsSync(src) || !fs.statSync(src).isFile()) return
  fs.mkdirSync(outDecksDir, { recursive: true })
  fs.copyFileSync(src, path.join(outDecksDir, filename))
}

/** Watch public/decks and sync to dist so deck JSON edits show up without full rebuild. */
function watchDecksPlugin() {
  let watcher: fs.FSWatcher | null = null
  return {
    name: "watch-decks",
    buildStart() {
      if (!fs.existsSync(publicDecksDir)) return
      fs.mkdirSync(outDecksDir, { recursive: true })
      for (const name of fs.readdirSync(publicDecksDir)) {
        const p = path.join(publicDecksDir, name)
        if (fs.statSync(p).isFile()) syncDecksFile(name)
      }

      // Only start a filesystem watcher during watch builds (`vite build --watch`).
      // In non-watch builds, this keeps the process alive and causes `npm run build` to hang.
      // Rollup exposes this via `this.meta.watchMode`.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const watchMode = Boolean((this as any)?.meta?.watchMode)
      if (!watchMode || watcher) return
      try {
        watcher = fs.watch(publicDecksDir, { recursive: false }, (_event, filename) => {
          if (filename) syncDecksFile(filename)
        })
      } catch {
        // watch can fail in some environments
      }
    },
  }
}

/**
 * Dedicated build for the Effuse-only client bootstrap.
 *
 * We intentionally produce stable filenames so the Worker SSR host can link
 * without needing a Vite manifest in Phase 5.
 */
export default defineConfig({
  envPrefix: ["VITE_", "CONVEX_"],
  // Ensure static assets (favicon, robots.txt, etc.) are available to the Worker
  // via the `assets.directory` binding.
  publicDir: "public",
  resolve: {
    alias: {
      "@openagentsinc/effuse": path.resolve(__dirname, "../../packages/effuse"),
      "@openagentsinc/effuse-ui": path.resolve(__dirname, "../../packages/effuse-ui"),
      "@openagentsinc/hud": path.resolve(__dirname, "../../packages/hud"),
    },
  },
  plugins: [
    watchDecksPlugin(),
    tsConfigPaths({
      projects: ["./tsconfig.json"],
    }),
  ],
  build: {
    outDir: "dist/effuse-client",
    emptyOutDir: true,
    copyPublicDir: true,
    cssCodeSplit: false,
    lib: {
      entry: "src/effuse-app/client.ts",
      formats: ["es"],
      fileName: () => "effuse-client.js",
    },
    rollupOptions: {
      output: {
        assetFileNames: (assetInfo) => {
          if (assetInfo.name && assetInfo.name.endsWith(".css")) return "effuse-client.css"
          return "[name][extname]"
        },
      },
    },
  },
})
