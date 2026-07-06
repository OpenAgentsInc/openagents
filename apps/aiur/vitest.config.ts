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
    ],
  },
  test: {
    // Most of this app's current tests construct raw `Request` objects with
    // a `cookie` header (server-side/Workers-style auth logic) — happy-dom
    // enforces the Fetch-spec forbidden-header list (like a real browser)
    // and silently drops `cookie`, which breaks that pattern. Use `node` by
    // default; component tests that need a DOM can opt in per-file with a
    // `// @vitest-environment happy-dom` docblock.
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
})
