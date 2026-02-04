import { defineConfig } from 'vite';
import tsConfigPaths from 'vite-tsconfig-paths';
import { tanstackStart } from '@tanstack/react-start/plugin/vite';
import viteReact from '@vitejs/plugin-react';
import { cloudflare } from '@cloudflare/vite-plugin';
import * as dotenv from 'dotenv';

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
    // Single React instance so hooks work after optimizing deps (e.g. @ai-sdk/provider-utils).
    dedupe: ['react', 'react-dom', 'react/jsx-runtime'],
  },
  ssr: {
    // Force one React in SSR bundle to avoid "Invalid hook call" / useState null.
    noExternal: ['react', 'react-dom', 'react/jsx-runtime'],
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
