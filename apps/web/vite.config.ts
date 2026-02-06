import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import tsConfigPaths from 'vite-tsconfig-paths';
import { tanstackStart } from '@tanstack/react-start/plugin/vite';
import viteReact from '@vitejs/plugin-react';
import { cloudflare } from '@cloudflare/vite-plugin';
import * as dotenv from 'dotenv';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Load .env.local (TanStack Start/Vite convention)
dotenv.config({ path: '.env.local', quiet: true });
// Production: load .env.production for build (WorkOS, etc.; Convex is separate)
if (process.env.NODE_ENV === 'production') {
  dotenv.config({ path: '.env.production', quiet: true });
}
// Also load .env as fallback
dotenv.config({ quiet: true });

export default defineConfig({
  envPrefix: ['VITE_', 'CONVEX_'],
  server: {
    port: 3000,
    proxy: {
      // Local dev: forward Agents SDK websocket + REST to the worker (wrangler dev).
      '/agents': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: true,
        ws: true,
      },
    },
  },
  resolve: {
    alias: {
      // Resolve local workspace packages so TS source is bundled in Vite/SSR.
      '@openagentsinc/effuse': path.resolve(__dirname, '../../packages/effuse'),
      '@openagentsinc/effuse-ui': path.resolve(__dirname, '../../packages/effuse-ui'),
      '@openagentsinc/hud': path.resolve(__dirname, '../../packages/hud'),
    },
    // Single instances to avoid mixed animator systems.
    dedupe: ['react', 'react-dom', 'react/jsx-runtime'],
  },
  ssr: {
    // Force one React in SSR bundle to avoid "Invalid hook call" / useState null.
    // Bundle local @openagentsinc/hud so it resolves and runs in SSR.
    noExternal: [
      'react',
      'react-dom',
      'react/jsx-runtime',
      '@openagentsinc/effuse',
      '@openagentsinc/effuse-ui',
      '@openagentsinc/hud',
    ],
  },
  plugins: [
    cloudflare({ viteEnvironment: { name: 'ssr' } }),
    tsConfigPaths({
      projects: ['./tsconfig.json'],
    }),
    tanstackStart(),
    viteReact(),
  ],
});
