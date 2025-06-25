/**
 * Simple HTML builder for safe HTML generation without template literals
 *
 * This utility ensures all user content is properly escaped to prevent:
 * - Template literal syntax conflicts (backticks)
 * - XSS vulnerabilities (script injection)
 * - HTML structure breaking (unclosed tags)
 */
export class HtmlBuilder {
  private parts: Array<string> = []

  /**
   * Add raw HTML (use with caution - only for trusted static HTML)
   */
  add(html: string): this {
    this.parts.push(html)
    return this
  }

  /**
   * Add text content that will be escaped
   */
  addText(text: string): this {
    this.parts.push(this.escape(text))
    return this
  }

  /**
   * Add an HTML element with optional attributes and content
   */
  element(tag: string, attributes?: Record<string, string>, content?: string | (() => void)): this {
    this.add(`<${tag}`)

    if (attributes) {
      for (const [key, value] of Object.entries(attributes)) {
        this.add(` ${key}="`)
        this.addText(value)
        this.add("\"")
      }
    }

    if (content === undefined || content === null) {
      // Self-closing tag
      this.add(" />")
    } else {
      this.add(">")

      if (typeof content === "string") {
        this.addText(content)
      } else {
        // Execute the content function to add nested elements
        content()
      }

      this.add(`</${tag}>`)
    }

    return this
  }

  /**
   * Escape text for safe inclusion in HTML
   * Escapes: & < > " ' `
   */
  private escape(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;")
      .replace(/`/g, "&#96;") // Critical for preventing template literal issues!
  }

  /**
   * Get the final HTML string
   */
  toString(): string {
    return this.parts.join("")
  }

  /**
   * Clear the builder
   */
  clear(): this {
    this.parts = []
    return this
  }
}

/**
 * Convenience function to create a new HTML builder
 */
export function html(): HtmlBuilder {
  return new HtmlBuilder()
}
