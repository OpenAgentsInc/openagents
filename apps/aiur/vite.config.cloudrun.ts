/**
 * Cloud Run build config (CFG-11, #8526). Identical app build to
 * `vite.config.ts` minus the Cloudflare Worker plugin, plus TanStack Start
 * SPA mode so the build prerenders a static shell (`dist/client/_shell/`)
 * that the Bun server (`src/cloudrun/server.ts`) serves for page routes.
 * Aiur has no server functions — every page decides what to render from
 * `/api/aiur/access` client-side, so shipping the shell statically keeps
 * the Cloud Run runtime a thin static + proxy server.
 */

import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import tailwindcss from '@tailwindcss/vite'
import viteReact from '@vitejs/plugin-react'
import path from 'node:path'
import { defineConfig } from 'vite'

const routerSsrPackages = [
  '@tanstack/history',
  '@tanstack/query-core',
  '@tanstack/react-query',
  '@tanstack/react-router',
  '@tanstack/react-router-ssr-query',
  '@tanstack/react-router/ssr',
  '@tanstack/react-router/ssr/server',
  '@tanstack/router-core',
]

export default defineConfig({
  resolve: {
    alias: [
      {
        find: '@',
        replacement: path.resolve(__dirname, './src'),
      },
      {
        find: '~',
        replacement: path.resolve(__dirname, './src'),
      },
    ],
  },
  environments: {
    ssr: {
      resolve: {
        noExternal: [...routerSsrPackages],
      },
    },
  },
  ssr: {
    external: [],
    noExternal: [...routerSsrPackages],
  },
  build: {
    minify: 'esbuild',
    reportCompressedSize: false,
  },
  plugins: [
    tanstackStart({
      spa: {
        enabled: true,
      },
      server: {
        build: {
          inlineCss: false,
        },
      },
      importProtection: {
        behavior: 'error',
        client: {
          files: ['**/*.server.*', '**/server/**'],
          specifiers: ['@tanstack/react-start/server'],
        },
      },
      router: {
        codeSplittingOptions: {
          defaultBehavior: [
            [
              'component',
              'pendingComponent',
              'errorComponent',
              'notFoundComponent',
              'loader',
            ],
          ],
        },
      },
    }),
    viteReact(),
    tailwindcss(),
  ],
})
