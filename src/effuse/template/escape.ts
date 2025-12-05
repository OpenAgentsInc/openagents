/**
 * Effuse HTML Escaping Utilities
 *
 * Prevents XSS by escaping special HTML characters.
 */

const ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
}

/**
 * Escape HTML special characters to prevent XSS.
 */
export const escapeHtml = (str: string): string =>
  str.replace(/[&<>"']/g, (char) => ESCAPE_MAP[char] ?? char)

/**
 * Check if a string appears to be already-escaped HTML.
 * Used to avoid double-escaping TemplateResults.
 */
export const isEscapedHtml = (str: string): boolean =>
  /&(?:amp|lt|gt|quot|#39);/.test(str)
