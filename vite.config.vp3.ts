import "vite-plus/test/config"

import { resolve } from "node:path"
import { defineConfig } from "vite-plus"

const setupFile = resolve(import.meta.dirname, "scripts/vp3-vitest-setup.ts")

/**
 * VP-3 destination-oracle configuration.
 *
 * This does not become workspace authority until VP-4. It exists so every
 * retained suite can be exercised by the exact Vite Plus test runtime before
 * the Bun lockfile and root commands are replaced atomically.
 */
export default defineConfig({
  ssr: { noExternal: ["effect-cf"] },
  resolve: {
    alias: {
      "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
      "cloudflare:workers": resolve(import.meta.dirname, "apps/openagents.com/workers/api/src/test/cloudflare-workers.ts"),
    },
  },
  test: {
    projects: [
      {
        ssr: { noExternal: ["effect-cf"] },
        resolve: {
          alias: {
            "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
            "cloudflare:workers": resolve(import.meta.dirname, "apps/openagents.com/workers/api/src/test/cloudflare-workers.ts"),
          },
        },
        test: {
          name: "node",
          environment: "node",
          include: [
            "**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}",
            "**/*.node-suite.ts",
          ],
          exclude: [
            "**/node_modules/**",
            "**/dist/**",
            "**/dist-electron/**",
            "**/.{git,cache,output,temp}/**",
            "projects/**",
            "apps/aiur/**",
            "apps/openagents.com/apps/start/**",
            "apps/openagents.com/apps/web/**",
            "apps/openagents.com/workers/api/**",
            "clients/khala-mobile/**",
            "packages/ui/**",
          ],
          hookTimeout: 240_000,
          setupFiles: [setupFile],
          testTimeout: 240_000,
        },
      },
      {
        test: {
          name: "foldkit-dom",
          environment: "happy-dom",
          include: [
            "packages/ui/**/*.{test,spec}.{ts,tsx}",
          ],
          server: { deps: { inline: ["foldkit"] } },
          hookTimeout: 240_000,
          setupFiles: [setupFile],
          testTimeout: 240_000,
        },
      },
      "./apps/aiur/vitest.config.ts",
      "./apps/openagents.com/apps/start/vitest.config.ts",
      "./apps/openagents.com/apps/web/vitest.config.ts",
      "./apps/openagents.com/workers/api/vitest.config.ts",
      "./clients/khala-mobile/vitest.config.ts",
    ],
  },
})
