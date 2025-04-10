import { reactRouter } from "@react-router/dev/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig(({ mode, command }) => ({ // Use function form for mode access
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
    proxy: {
      // Proxy agents requests directly to the production server
      '/agents': {
        target: 'https://agents.openagents.com',
        changeOrigin: true,
        secure: true,
        ws: true, // Enable WebSocket proxying
      },
    },
  },
  // --- SSR Configuration ---
  ssr: {
    // Attempt to externalize modules incompatible with Workers/SSR build
    // NOTE: 'better-sqlite3' CANNOT run in standard workers.
    // It must only be used server-side OUTSIDE the worker, or replaced (e.g., with D1).
    // If used only in loaders/actions NOT intended for the edge, ensure it's handled correctly.
    external: [
      "better-sqlite3", // Must be external, cannot be bundled for worker
      // Add other known Node.js-specific or problematic CJS modules if needed
    ],
    // Try to bundle these if they cause issues when externalized,
    // but they might need code changes (dynamic imports) if they use browser APIs.
    noExternal: [
      // Example: If '@openagents/core' or 'agents/react' were causing issues
      // when externalized, try adding them here. BUT, if they fail because
      // they access 'window', they need code changes, not just bundling.
      // '@openagents/core',
      // 'agents/react',
    ],
    // Explicitly target the Workers environment if issues persist,
    // though the cloudflare plugin might handle this.
    // target: 'webworker',
  },
  plugins: [
    // REMOVED the complex custom 'external-modules' plugin.
    // Address library issues with ssr.external/noExternal or dynamic imports in your code.

    // Cloudflare plugin - Ensure this is configured correctly for your Worker setup.
    // The `viteEnvironment` might need adjustment based on React Router / Worker interactions.
    // Consider if you need Pages Functions (`{ type: "functions" }`) or just Worker (`{ type: "worker" }`)
    cloudflare({
       // Example: Explicitly setting type if needed, consult plugin docs
       // type: command === 'build' ? 'worker' : undefined,
       viteEnvironment: { name: "ssr" } // Keep for now, but monitor
    }),
    tailwindcss(),
    reactRouter({
      // React Router options - check if specific adapters are needed for Cloudflare
      // e.g., appDirectory: "./app", entryClientFile: ..., entryServerFile: ...
    }),
    tsconfigPaths()
  ],
  // Ensure build targets are appropriate for Cloudflare Workers
  build: {
     target: 'esnext', // Workers support modern JS
     // ssr: true // Handled by react-router build script / cloudflare plugin? Verify.
  }
}));
