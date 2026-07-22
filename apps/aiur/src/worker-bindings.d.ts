/**
 * Minimal local type stub for the Worker binding types this app's env shape
 * references (#9169). Type-only: it reintroduces NO Cloudflare Workers
 * runtime, deploy target, or dependency — it only lets the existing `env`
 * type annotations typecheck. See apps/openagents.com/CLAUDE.md on the
 * retired-Workers policy.
 */
interface Fetcher {
  readonly fetch: (input: Request | string, init?: RequestInit) => Promise<Response>;
}
