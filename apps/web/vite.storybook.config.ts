import path from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig } from "vite"
import tsConfigPaths from "vite-tsconfig-paths"

const __dirname = path.dirname(fileURLToPath(import.meta.url))

/**
 * UI-only storybook (no Wrangler / no Worker).
 *
 * Why:
 * - faster local iteration for UI
 * - no Worker bindings / env required
 * - no Convex/Auth/AI side effects
 *
 * The Worker-hosted storybook routes still exist for prod-parity and the
 * effuse-test visual suite, but `npm run storybook` should be UI-only.
 */
export default defineConfig({
  root: path.resolve(__dirname, "storybook"),
  publicDir: path.resolve(__dirname, "public"),
  resolve: {
    alias: {
      "@openagentsinc/effuse": path.resolve(__dirname, "../../packages/effuse"),
      "@openagentsinc/effuse-ui": path.resolve(__dirname, "../../packages/effuse-ui"),
      "@openagentsinc/hud": path.resolve(__dirname, "../../packages/hud"),
    },
  },
  plugins: [
    tsConfigPaths({
      projects: [path.resolve(__dirname, "tsconfig.json")],
    }),
  ],
  server: {
    host: "127.0.0.1",
    port: 6006,
    strictPort: true,
    // Allow the storybook root to import app sources from the parent directory.
    fs: {
      allow: [__dirname, path.resolve(__dirname, "..")],
    },
  },
})
