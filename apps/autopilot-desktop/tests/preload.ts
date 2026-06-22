// CL-53: bun test preload.
//
// The Foldkit module graph (foldkit's index re-exports its Html runtime, which
// loads snabbdom) touches `window`/`requestAnimationFrame` at module-evaluation
// time. The desktop tests run in bun's node-like environment with no DOM, and
// they only exercise PURE logic (helpers + the update reducer) — they never
// render. A minimal global shim lets those modules import without a real DOM.
//
// Shared StyleX-authored packages are also imported as source in these tests,
// outside the web/desktop production compilers. Keep their test-only fallback on
// here so raw `stylex.create(...)` authoring calls do not execute under Bun.

Bun.env.OA_STYLEX_RUNTIME_FALLBACK = "1"

const g = globalThis as unknown as {
  window?: unknown
  requestAnimationFrame?: (cb: FrameRequestCallback) => number
  cancelAnimationFrame?: (handle: number) => void
}

if (g.window === undefined) {
  g.requestAnimationFrame ??= (cb: FrameRequestCallback): number => {
    return setTimeout(() => cb(Date.now()), 0) as unknown as number
  }
  g.cancelAnimationFrame ??= (handle: number): void => {
    clearTimeout(handle as unknown as ReturnType<typeof setTimeout>)
  }
  g.window = globalThis
}
