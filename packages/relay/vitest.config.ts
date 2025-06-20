import { defineConfig } from 'vitest/config'
import { resolve } from 'path'

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    setupFiles: ['./test/setup.ts'],
    testTimeout: 30000,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
      '@openagentsinc/nostr': resolve(__dirname, '../nostr/dist/esm/index.js'),
    },
  },
})