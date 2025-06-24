// HTML template tag for syntax highlighting and future processing
export function html(strings: TemplateStringsArray, ...values: Array<any>): string {
  return strings.reduce((result, str, i) => {
    const value = values[i - 1]
    return result + (value !== undefined ? String(value) : "") + str
  })
}

// CSS template tag for syntax highlighting and future processing
export function css(strings: TemplateStringsArray, ...values: Array<any>): string {
  return strings.reduce((result, str, i) => {
    const value = values[i - 1]
    return result + (value !== undefined ? String(value) : "") + str
  })
}

// Helper to create a basic HTML document
export function document(options: {
  title?: string
  styles?: string
  body: string
  meta?: Record<string, string>
  head?: string
}): string {
  const metaTags = options.meta
    ? Object.entries(options.meta)
      .map(([name, content]) => `<meta name="${name}" content="${content}">`)
      .join("\n        ")
    : ""

  return html`
    <!DOCTYPE html>
    <html>
      <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>${options.title || "Psionic App"}</title>
        <link rel="icon" type="image/x-icon" href="/favicon.ico">
        ${metaTags}
        ${options.head || ""}
        ${options.styles ? `<style>${options.styles}</style>` : ""}
      </head>
      <body>
        ${options.body}
      </body>
    </html>
  `
}
