/**
 * CFG-9 (#8524): Bun preload that maps the `cloudflare:workers` built-in to
 * the structural stub, mirroring the vitest alias in vitest.config.ts.
 *
 * Loaded via `bun --preload ./src/cloudrun/preload.ts` (see Dockerfile /
 * package.json `start:cloudrun`). Must be a plugin (not a plain import
 * rewrite) because `cloudflare:workers` is imported by node_modules
 * (`@cloudflare/containers`, `effect-cf`) that we do not patch.
 */
import { plugin } from 'bun'

plugin({
  name: 'cloudflare-workers-stub',
  setup(build) {
    build.module('cloudflare:workers', () => ({
      exports: require('./cloudflare-workers-stub.ts'),
      loader: 'object',
    }))
  },
})
