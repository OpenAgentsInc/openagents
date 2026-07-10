// Vitest-only stand-in for `apps/sarah/src/server.ts`.
//
// The real Sarah server module resolves filesystem paths and a Bun SQL client
// at import time, which cannot load in the Vite-transformed happy-dom test
// environment. The Start server tests exercise header and shared-surface
// dispatch only and never route `/sarah/api/*`; if a test ever does, this stub
// fails loudly instead of silently faking Sarah behavior.
export const handleSarahRequest = (_request: Request): Promise<Response> => {
  throw new Error(
    'sarah server stub: /sarah/api routing is not available in the vitest environment',
  )
}
