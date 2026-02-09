export type WorkerAssets = {
  readonly fetch: (request: Request) => Promise<Response>
}

/**
 * Cloudflare Worker bindings for the Effuse host.
 *
 * Note: `@cloudflare/workers-types` provides `DurableObjectNamespace`, `Ai`, etc.
 * We keep this type local (not global) so the rest of `apps/web` can remain
 * framework-agnostic while TanStack Start is still the default host.
 */
export type WorkerEnv = {
  readonly ASSETS?: WorkerAssets
  readonly AI?: Ai

  // Vite-style env var name is used on both client + worker to keep config simple.
  readonly VITE_CONVEX_URL?: string
  /** When set (e.g. "1" or "true"), prelaunch mode: homepage shows countdown, other routes redirect to /. */
  readonly VITE_PRELAUNCH?: string
}
