import { resolve } from 'node:path'
import { defineConfig } from 'vitest/config'

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
})
