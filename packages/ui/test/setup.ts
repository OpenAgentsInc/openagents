// Minimal DOM-global shim for `bun test`.
//
// Foldkit's vdom layer pulls in snabbdom's `style` module, which references
// `window.requestAnimationFrame` at module-evaluation time. Under `bun test`
// there is no DOM, so importing any kit module (transitively `foldkit/html`)
// throws `ReferenceError: window is not defined` before any test body runs.
//
// The UI kit tests here only assert on plain data or Foldkit virtual nodes and
// never render to a real DOM, so a tiny global shim is enough. The web app keeps
// a full happy-dom environment via Vitest for render tests.
const globalScope = globalThis as Record<string, unknown>

if (typeof globalScope.window === 'undefined') {
  const raf = (cb: (time: number) => void): number => {
    cb(0)

    return 0
  }
  const caf = (): void => {}

  globalScope.window = {
    requestAnimationFrame: raf,
    cancelAnimationFrame: caf,
  }
  globalScope.requestAnimationFrame = raf
  globalScope.cancelAnimationFrame = caf
}
