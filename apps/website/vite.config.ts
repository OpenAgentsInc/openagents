import { defineConfig } from 'vite'
import { devtools } from '@tanstack/devtools-vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import viteReact from '@vitejs/plugin-react'
import viteTsConfigPaths from 'vite-tsconfig-paths'
import tailwindcss from '@tailwindcss/vite'
import { cloudflare } from '@cloudflare/vite-plugin'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'

const config = defineConfig(({ mode }) => {
  const isTest = mode === 'test'
  return {
    plugins: [
      devtools(),
      ...(isTest ? [] : [cloudflare({ viteEnvironment: { name: 'ssr' } })]),
      // this is the plugin that enables path aliases
      viteTsConfigPaths({
        projects: ['./tsconfig.json'],
      }),
      tailwindcss(),
      wasm(),
      topLevelAwait(),
      tanstackStart(),
      viteReact(),
    ],
    envPrefix: ['VITE_', 'CONVEX_', 'PUBLIC_'],
    optimizeDeps: {
      exclude: ['@breeztech/breez-sdk-spark'],
    },
    build: {
      target: 'esnext',
      chunkSizeWarningLimit: 1500,
      rollupOptions: {
        onwarn(warning, warn) {
          if (warning.code === 'EVAL' && warning.id?.includes('gray-matter')) {
            return
          }
          warn(warning)
        },
      },
    },
    server: {
      headers: {
        'Cross-Origin-Embedder-Policy': 'require-corp',
        'Cross-Origin-Opener-Policy': 'same-origin',
      },
    },
    test: {
      environment: 'node',
      pool: 'threads',
    },
  }
})

export default config
