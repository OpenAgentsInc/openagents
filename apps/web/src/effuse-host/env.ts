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
  /** Convex dev often writes this to .env.local; Worker falls back to it when VITE_CONVEX_URL unset. */
  readonly CONVEX_URL?: string
  /** When set (e.g. "1" or "true"), prelaunch mode: homepage shows countdown, other routes redirect to /. */
  readonly VITE_PRELAUNCH?: string
  /** Secret: when set, ?key={this} on / bypasses prelaunch. Set via wrangler secret put PRELAUNCH_BYPASS_KEY. */
  readonly PRELAUNCH_BYPASS_KEY?: string
  /** OpenRouter API key. When set, primary inference uses OpenRouter (e.g. moonshotai/kimi-k2.5) with Cloudflare Workers AI as fallback. Set via wrangler secret put OPENROUTER_API_KEY. */
  readonly OPENROUTER_API_KEY?: string

  /**
   * E2E auth bypass (prod testing only).
   *
   * When set, `/api/auth/e2e/*` routes are enabled and require
   * `Authorization: Bearer <OA_E2E_BYPASS_SECRET>`.
   */
  readonly OA_E2E_BYPASS_SECRET?: string
  /**
   * Private JWK (JSON string) for minting E2E JWTs accepted by Convex.
   * Stored as a Wrangler secret.
   */
  readonly OA_E2E_JWT_PRIVATE_JWK?: string

  /**
   * DSE admin secret (headless ops).
   *
   * When set, selected `/api/dse/*` endpoints accept:
   * `Authorization: Bearer <OA_DSE_ADMIN_SECRET>`
   */
  readonly OA_DSE_ADMIN_SECRET?: string
}
