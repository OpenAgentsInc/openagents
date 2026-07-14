import react from '@vitejs/plugin-react'
import path from 'node:path'
import { defineConfig } from 'vite-plus/test/config'

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
        // Server-chain modules may import the Bun builtin, which Vite cannot
        // resolve in the vitest environment. See src/test/bun-builtin-stub.ts.
        find: /^bun$/,
        replacement: path.resolve(__dirname, './src/test/bun-builtin-stub.ts'),
      },
    ],
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    setupFiles: [
      path.resolve(import.meta.dirname, 'src/test/vitest-cwd-setup.ts'),
      path.resolve(import.meta.dirname, '../../../../scripts/vp3-vitest-setup.ts'),
    ],
  },
})
