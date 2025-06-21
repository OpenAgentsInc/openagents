/**
 * HTML utilities for components
 */

/**
 * Simple HTML template literal tag
 * Escapes HTML to prevent XSS attacks
 * @param {TemplateStringsArray} strings
 * @param {...any} values
 * @returns {string} HTML string
 */
export function html(strings: TemplateStringsArray, ...values: any[]): string {
  let result = strings[0]
  for (let i = 0; i < values.length; i++) {
    result += String(values[i]) + strings[i + 1]
  }
  return result
}

/**
 * Escape HTML to prevent XSS attacks
 * @param {string} unsafe - Unsafe HTML string
 * @returns {string} Escaped HTML string
 */
export function escapeHtml(unsafe: string): string {
  return unsafe
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;")
}