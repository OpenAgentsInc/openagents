import path from 'path';
import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';
import tsConfigPaths from 'vite-tsconfig-paths';
import { tanstackStart } from '@tanstack/react-start/plugin/vite';
import viteReact from '@vitejs/plugin-react';
import { cloudflare } from '@cloudflare/vite-plugin';
import * as dotenv from 'dotenv';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Load .env.local (TanStack Start/Vite convention)
dotenv.config({ path: '.env.local', quiet: true });
// Also load .env as fallback
dotenv.config({ quiet: true });

export default defineConfig({
  envPrefix: ['VITE_', 'CONVEX_'],
  server: {
    port: 3000,
  },
  resolve: {
    alias: {
      // Resolve local workspace package so @openagentsinc/hud and @openagentsinc/hud/react work
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
