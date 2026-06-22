const globalScope = globalThis as Record<string, unknown>

Bun.env.OA_STYLEX_RUNTIME_FALLBACK = "1"

if (typeof globalScope.window === "undefined") {
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
