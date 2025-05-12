# Vite Configuration Analysis and Simplification (apps/website)

This document analyzes the Vite configuration (`vite.config.ts`) in the `apps/website` directory as of the `auth` branch, identifies complexities and potential issues, and proposes a simplified configuration.

## Analysis (Pre-Simplification)

1.  **Structure:** Contains standard web app structure (`app/`, `public/`), configuration (`vite.config.ts`, `tsconfig*.json`, `components.json`), Cloudflare specific files (`wrangler.jsonc`, `workers/`, `worker-configuration.d.ts`), and React Router config (`react-router.config.ts`). The presence of `workers` and `wrangler.jsonc` confirms a Cloudflare Workers deployment target.
2.  **`package.json`:**
    *   Uses `@react-router/dev` for build/dev scripts, indicating reliance on React Router's conventions.
    *   Dependencies include UI libraries (`@radix-ui`, `lucide-react`, `tailwind-merge`, `clsx`), state (`zustand`), core logic (`@openagents/core`, `agents`), auth (`better-auth`), and significantly, `better-sqlite3`.
    *   Dev dependencies include Vite, `@cloudflare/vite-plugin`, `wrangler`, Tailwind CSS, TypeScript.
    *   **Potential Issue:** `better-sqlite3` is a native Node.js module and **cannot** run directly in the standard Cloudflare Workers runtime. Its presence suggests it might only be used during local development/build or requires a specific setup (like Hyperdrive) not evident here. This is a likely source of deployment or runtime errors.
3.  **`vite.config.ts`:**
    *   **Complexity:** The primary complexity lies in the custom `external-modules` plugin. This plugin manually intercepts imports for several libraries (`react-native`, `rxdb`, `agents/react`, `@openagents/core`) and replaces them with empty or mock modules (`\0empty-module:`). This is a fragile workaround for libraries that are not SSR/Worker-friendly. It indicates these libraries likely contain code incompatible with the server/worker environment (e.g., accessing `window` or Node-specific APIs not available in Workers).
    *   **Cloudflare Plugin:** Uses `@cloudflare/vite-plugin` with `viteEnvironment: { name: "ssr" }`. While Cloudflare Workers *can* do SSR, they are not a standard Node.js SSR environment. This setting might interact unexpectedly with React Router's SSR expectations or the custom stubbing plugin. The Worker runtime has limitations compared to Node.
    *   **Other Plugins:** `tailwindcss`, `reactRouter`, `tsconfigPaths` are standard for this stack.
    *   **Proxy:** A dev server proxy is configured for `/agents`.
    *   **Missing SSR Config:** Lacks standard Vite `ssr` options (`external`, `noExternal`, `target`). The custom plugin attempts to handle this manually.

## Problems & Simplification Rationale

*   The custom `external-modules` plugin is complex, hard to maintain, and hides the underlying incompatibility issues. It's better to address the incompatibilities directly.
*   `better-sqlite3` is fundamentally incompatible with the default Workers runtime. It needs replacement (e.g., Cloudflare D1 with a compatible adapter like LibSQL/Turso) or careful externalization if only used server-side outside the worker.
*   The interaction between React Router's SSR, Vite's SSR build, the custom plugin, and the Cloudflare Worker environment is likely causing conflicts.
*   The goal is to remove the custom plugin and rely on standard Vite mechanisms (`ssr.external`, `ssr.noExternal`) and code-level solutions (like dynamic imports) for handling environment incompatibilities.

## Proposed Simplified `vite.config.ts`

```typescript
// apps/website/vite.config.ts
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
```

## Next Steps After Applying Simplification

1.  **Address Library Issues:**
    *   **`better-sqlite3`:** Determine how this is used. Replace it with a Worker-compatible solution (Cloudflare D1, KV, R2, or calling an external API). The `better-auth-integration.md` doc suggests using the LibSQL dialect (`@libsql/kysely-libsql`), which is compatible with Turso/Cloudflare D1. Ensure this adapter is correctly configured in `app/lib/auth.ts` and that `better-sqlite3` is fully removed or only used in non-worker contexts.
    *   **Other Libraries (`@openagents/core`, `agents/react`, etc.):** If errors persist related to these after removing the custom plugin, investigate *why*. If they use `window`, `document`, etc., refactor the components using them with dynamic `import()`, `React.lazy`, or conditional checks (`typeof window !== 'undefined'`).
2.  **Test:** Run `yarn dev` and `yarn build` / `yarn deploy` and check for errors.
3.  **Review Cloudflare Plugin:** Double-check `@cloudflare/vite-plugin` documentation for integration with React Router SSR in a Worker environment.
