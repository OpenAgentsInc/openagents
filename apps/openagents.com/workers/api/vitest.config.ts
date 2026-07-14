import { resolve } from 'node:path'
import { defineConfig } from 'vite-plus/test/config'

export default defineConfig({
  resolve: {
    alias: {
      'cloudflare:workers': resolve(
        import.meta.dirname,
        'src/test/cloudflare-workers.ts',
      ),
    },
  },
  ssr: {
    noExternal: ['effect-cf'],
  },
  test: {
    setupFiles: [
      resolve(import.meta.dirname, 'src/test/vitest-cwd-setup.ts'),
      resolve(import.meta.dirname, '../../../../scripts/vp3-vitest-setup.ts'),
    ],
  },
})
