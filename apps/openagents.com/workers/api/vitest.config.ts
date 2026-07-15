import { resolve } from 'node:path'
import { defineConfig } from 'vite-plus/test/config'

export default defineConfig({
  test: {
    setupFiles: [
      resolve(import.meta.dirname, 'src/test/vitest-cwd-setup.ts'),
      resolve(import.meta.dirname, '../../../../scripts/vp3-vitest-setup.ts'),
    ],
  },
})
