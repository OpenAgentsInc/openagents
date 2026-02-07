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

/**
 * Escape a JSON string for safe embedding inside an HTML `<script>` element.
 *
 * Why this exists: the HTML parser terminates a `<script>` element on `</script>`,
 * even when `type="application/json"`. Replacing `<` ensures untrusted data cannot
 * break out of the script tag.
 */
export const escapeJsonForHtmlScript = (json: string): string => json.replace(/</g, "\\u003c")
