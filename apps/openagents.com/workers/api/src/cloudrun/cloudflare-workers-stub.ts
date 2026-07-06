/**
 * CFG-9 (#8524): runtime stub for the `cloudflare:workers` built-in module.
 *
 * The Bun monolith loads the exact same import graph as the Worker, and two
 * dependencies (`@cloudflare/containers`, `effect-cf`) import
 * `cloudflare:workers` at module scope for BASE CLASSES only. Outside
 * workerd those classes are never instantiated on the Cloud Run paths (the
 * DO/Container bindings are either absent or typed-unavailable shims), so a
 * structural stub is sufficient — the same approach as the vitest alias
 * `src/test/cloudflare-workers.ts`.
 *
 * Registered as a Bun virtual module by `preload.ts`.
 */

export class WorkerEntrypoint<Env = unknown> {
  protected readonly ctx: ExecutionContext
  protected readonly env: Env

  constructor(ctx: ExecutionContext, env: Env) {
    this.ctx = ctx
    this.env = env
  }
}

export class DurableObject<Env = unknown> {
  protected readonly ctx: DurableObjectState
  protected readonly env: Env

  constructor(ctx: DurableObjectState, env: Env) {
    this.ctx = ctx
    this.env = env
  }
}

export class RpcTarget {}

export class RpcStub {}

export class WorkflowEntrypoint<Env = unknown> {
  protected readonly ctx: ExecutionContext
  protected readonly env: Env

  constructor(ctx: ExecutionContext, env: Env) {
    this.ctx = ctx
    this.env = env
  }
}

/** workerd exposes a module-scope `env` mirror; nothing should use it here. */
export const env: Record<string, unknown> = {}
