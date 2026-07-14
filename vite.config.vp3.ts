import "vite-plus/test/config"

import { defineConfig } from "vite-plus"

/**
 * VP-3 destination-oracle configuration.
 *
 * This does not become workspace authority until VP-4. It exists so every
 * retained suite can be exercised by the exact Vite Plus test runtime before
 * the Bun lockfile and root commands are replaced atomically.
 */
export default defineConfig({
  test: {
    environment: "node",
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/dist-electron/**",
      "**/.{git,cache,output,temp}/**",
      "projects/**",
    ],
    hookTimeout: 240_000,
    testTimeout: 240_000,
  },
})
