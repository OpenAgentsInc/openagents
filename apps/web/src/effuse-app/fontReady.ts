/**
 * Ensures Square721 (hatchery/display font) and Berkeley Mono (deck title) are
 * loaded before we show elements that use them, to avoid a flash of fallback font (FOUC).
 * Adds `fonts-ready` to document.documentElement when the fonts are available.
 */
export function initFontReady(): void {
  if (typeof document === "undefined" || !document.fonts) {
    document.documentElement.classList.add("fonts-ready")
    return
  }

  const addReady = () => document.documentElement.classList.add("fonts-ready")

  const squareReady = document.fonts.check("1em Square721")
  const berkeleyReady = document.fonts.check("1em Berkeley Mono")
  if (squareReady && berkeleyReady) {
    addReady()
    return
  }

  Promise.all([
    squareReady ? Promise.resolve() : document.fonts.load("1em Square721"),
    berkeleyReady ? Promise.resolve() : document.fonts.load("1em Berkeley Mono"),
  ])
    .then(addReady)
    .catch(() => addReady())

  // Fallback: show content even if font fails to load (e.g. network error)
  const timeout = 3000
  setTimeout(addReady, timeout)
}
