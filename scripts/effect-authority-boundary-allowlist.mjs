// Intentional raw authority-boundary edges.
//
// Each entry must explain why the raw operation belongs at an edge instead of
// being migrated to an Effect service, Schema decoder, config service, or
// typed retry/timeout wrapper. Keep this list small: prefer moving the boundary
// inward over adding broad allowlist patterns.

export const allowedEdges = [
  // Root CLI commands are allowed to bridge the final Effect program into the
  // platform runtime. Domain modules should return Effect values instead.
  {
    kind: 'effect-run-promise',
    path: 'apps/pylon/src/index.ts',
    includes: 'Effect.runPromise',
    reason: 'Pylon CLI entrypoint owns the final Effect-to-process bridge.',
  },

  // Worker route entry files may issue the final Effect run for a request after
  // request-scoped layers have been constructed. Inner services stay Effectful.
  {
    kind: 'effect-run-promise',
    path: 'apps/openagents.com/workers/api/src/index.ts',
    includes: 'Effect.runPromise',
    reason: 'Cloudflare Worker entrypoint owns the final per-request bridge.',
  },
]
