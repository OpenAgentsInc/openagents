/**
 * Client-side markdown rendering
 * For now, this is a simple implementation that just escapes HTML
 * In the future, this can be enhanced with actual markdown parsing
 */

export async function renderMarkdown(text: string): Promise<string> {
  // Basic HTML escaping
  const escapeHtml = (str: string) => {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }

  // For now, just escape and preserve line breaks
  const escaped = escapeHtml(text)
  const withBreaks = escaped.replace(/\n/g, "<br>")

  return withBreaks
}
