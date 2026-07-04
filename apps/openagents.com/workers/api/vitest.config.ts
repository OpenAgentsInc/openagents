import { resolve } from 'node:path'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      'cloudflare:workers': resolve(
        import.meta.dirname,
        'src/test/cloudflare-workers.ts',
      ),
      // The Khala Sync client store is a Bun (`bun:sqlite`) module; the
      // KS-4.4 stitch-seam suite runs it over node:sqlite via this adapter.
      'bun:sqlite': resolve(import.meta.dirname, 'src/test/bun-sqlite.ts'),
    },
  },
  ssr: {
    noExternal: ['effect-cf'],
  },
})
