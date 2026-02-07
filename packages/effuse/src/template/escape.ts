/**
 * HTML escaping utilities
 */

export const escapeHtml = (text: string): string => {
  // Must be SSR-safe (no DOM). This matches typical HTML text escaping.
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
}
