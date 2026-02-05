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
      // Force single Arwes instances (avoid mixed animator systems)
      '@arwes/animator': path.resolve(
        __dirname,
        './node_modules/@arwes/animator/build/esm/index.js',
      ),
      '@arwes/react-animator': path.resolve(
        __dirname,
        './node_modules/@arwes/react-animator/build/esm/index.js',
      ),
      '@arwes/react-text': path.resolve(
        __dirname,
        './node_modules/@arwes/react-text/build/esm/index.js',
      ),
      '@arwes/text': path.resolve(
        __dirname,
        './node_modules/@arwes/text/build/esm/index.js',
      ),
    },
    // Single instances to avoid mixed animator systems.
    dedupe: [
      'react',
      'react-dom',
      'react/jsx-runtime',
      '@arwes/animator',
      '@arwes/react-animator',
      '@arwes/react-text',
    ],
  },
  optimizeDeps: {
    include: [
      '@arwes/animator',
      '@arwes/react-animator',
      '@arwes/react-text',
      '@arwes/text',
      '@arwes/animated',
      '@arwes/react-tools',
      '@arwes/tools',
    ],
  },
  ssr: {
    // Force one React in SSR bundle to avoid "Invalid hook call" / useState null.
    // Bundle local @openagentsinc/hud so it resolves and runs in SSR.
    noExternal: [
      'react',
      'react-dom',
      'react/jsx-runtime',
      '@openagentsinc/hud',
      '@arwes/animator',
      '@arwes/react-animator',
      '@arwes/react-text',
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
