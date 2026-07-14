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
