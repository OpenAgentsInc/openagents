import react from '@vitejs/plugin-react'
import path from 'node:path'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
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
      {
        // The Sarah server chain (imported by src/server.ts) uses the Bun
        // builtin (`import { SQL } from 'bun'`), which Vite cannot resolve
        // in the vitest environment. See src/test/bun-builtin-stub.ts.
        find: /^bun$/,
        replacement: path.resolve(__dirname, './src/test/bun-builtin-stub.ts'),
      },
      {
        // The Sarah server module resolves fs paths + a Bun SQL client at
        // import time and cannot load under happy-dom. The server tests never
        // route /sarah/api/*; see src/test/sarah-server-stub.ts.
        find: /^.*\/sarah\/src\/server\.ts$/,
        replacement: path.resolve(__dirname, './src/test/sarah-server-stub.ts'),
      },
    ],
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
})
