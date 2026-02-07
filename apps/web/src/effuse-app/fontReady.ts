/**
 * Ensures Square721 (hatchery/display font) is loaded before we show
 * elements that use it, to avoid a flash of fallback font (FOUC).
 * Adds `fonts-ready` to document.documentElement when the font is available.
 */
export function initFontReady(): void {
  if (typeof document === "undefined" || !document.fonts) {
    document.documentElement.classList.add("fonts-ready")
    return
  }

  const addReady = () => document.documentElement.classList.add("fonts-ready")

  if (document.fonts.check("1em Square721")) {
    addReady()
    return
  }

  document.fonts
    .load("1em Square721")
    .then(addReady)
    .catch(() => addReady())

  // Fallback: show content even if font fails to load (e.g. network error)
  const timeout = 3000
  setTimeout(addReady, timeout)
}
