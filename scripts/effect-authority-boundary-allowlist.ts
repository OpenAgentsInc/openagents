export type EffectAuthorityBoundaryAllowlistEntry = {
  readonly category:
    | "json-parse-cast"
    | "direct-env-read"
    | "bare-catch"
    | "raw-fetch"
    | "effect-run-promise"
  readonly path: string
  readonly pattern: string
  readonly reason: string
}

export const effectAuthorityBoundaryAllowlist = [
  {
    category: "effect-run-promise",
    path: "apps/openagents.com/workers/api/src/index.ts",
    pattern: "Effect.runPromise",
    reason:
      "Worker request entrypoint executes the already-layered Effect program at the platform edge.",
  },
  {
    category: "direct-env-read",
    path: "apps/pylon/src/index.ts",
    pattern: "Bun.env",
    reason:
      "CLI process entrypoint reads local process configuration before passing typed options into command handlers.",
  },
  {
    category: "direct-env-read",
    path: "apps/pylon/src/index.ts",
    pattern: "process.env",
    reason:
      "CLI process entrypoint reads local process configuration before passing typed options into command handlers.",
  },
] satisfies readonly EffectAuthorityBoundaryAllowlistEntry[]
