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
  server: {
    port: Number(process.env.PORT) || 3010,
  },
  environments: {
    ssr: {
      resolve: {
        noExternal: [...routerSsrPackages],
        // The Sarah server chain (src/server.ts -> apps/sarah) imports the
        // Bun builtin (`import { SQL } from 'bun'`). The deployed server runs
        // on Bun, so the builtin must stay external to the SSR bundle.
        external: ['bun'],
      },
    },
  },
  ssr: {
    external: ['bun'],
    noExternal: [...routerSsrPackages],
  },
  build: {
    minify: 'esbuild',
    reportCompressedSize: false,
    rollupOptions: {
      // Keep the Bun builtin external everywhere; only the Bun-run server
      // entry (via apps/sarah) ever imports it at runtime.
      external: ['bun'],
      output: {
        manualChunks: (id) => {
          if (
            id.includes('/node_modules/@tanstack/react-start') ||
            id.includes('/node_modules/@tanstack/start-')
          ) {
            return 'tanstack-start'
          }

          if (
            id.includes('/node_modules/@tanstack/react-router') ||
            id.includes('/node_modules/@tanstack/router-core') ||
            id.includes('/node_modules/@tanstack/history')
          ) {
            return 'tanstack-router'
          }

          if (
            id.includes('/node_modules/@tanstack/react-query') ||
            id.includes('/node_modules/@tanstack/query-core')
          ) {
            return 'tanstack-query'
          }

          if (
            id.includes('/node_modules/react-dom/') ||
            id.includes('/node_modules/react/') ||
            id.includes('/node_modules/scheduler/')
          ) {
            return 'react'
          }

          if (id.includes('/node_modules/lucide-react')) {
            return 'icons'
          }
        },
      },
    },
  },
  plugins: [
    tanstackStart({
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
