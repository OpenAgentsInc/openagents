/**
 * No-op `ExecutionContext.tracing` for non-Workers execution surfaces.
 *
 * `@cloudflare/workers-types` 4.20260702+ made `tracing` a REQUIRED member of
 * `ExecutionContext`. The OpenAgents API runs on Cloud Run (CFG-9), where the
 * Workers tracing runtime does not exist, and unit tests construct synthetic
 * contexts by hand. Both use this inert implementation: `enterSpan` /
 * `startActiveSpan` invoke their callbacks immediately with an untraced span
 * and record nothing. Nothing in this codebase consumes `ctx.tracing`; this
 * exists purely so honest (cast-free) `ExecutionContext` values stay
 * constructible off-Workers.
 */

class NoopSpan {
  get isTraced(): boolean {
    return false
  }
  setAttribute(_key: string, _value?: boolean | number | string): void {}
  end(): void {}
}

export const noopExecutionContextTracing: Tracing = {
  enterSpan: (_name, callback, ...args) =>
    callback(new NoopSpan() as unknown as Span, ...args),
  startActiveSpan: (_name, callback, ...args) =>
    callback(new NoopSpan() as unknown as Span, ...args),
  // `Span` is a declared abstract class in workers-types; off-Workers the
  // no-op concrete class stands in for it.
  Span: NoopSpan as unknown as typeof Span,
}
