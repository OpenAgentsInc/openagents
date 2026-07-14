import { resolve } from 'node:path'
import { configDefaults, defineConfig } from 'vite-plus/test/config'

export default defineConfig({
  test: {
    environment: 'happy-dom',
    exclude: [...configDefaults.exclude, 'repos/**'],
    setupFiles: [
      './src/vitest-setup.ts',
      resolve(import.meta.dirname, '../../../../scripts/vp3-vitest-setup.ts'),
    ],
    server: {
      deps: {
        inline: ['foldkit'],
      },
    },
  },
})
