import DOMPurify from "dompurify"
import { Context, Effect, Layer } from "effect"
import matter from "gray-matter"
import { JSDOM } from "jsdom"
import MarkdownIt from "markdown-it"
import { createHighlighter, type Highlighter } from "shiki"

// Types
export interface MarkdownOptions {
  readonly enableHtml: boolean
  readonly enableLinkify: boolean
  readonly typographer: boolean
}

export interface ParsedMarkdown {
  readonly content: string
  readonly data: Record<string, unknown>
  readonly excerpt: string | undefined
}

export interface BlogPostMetadata {
  readonly title: string
  readonly date: string
  readonly summary?: string
  readonly image?: string
  readonly slug?: string
  readonly [key: string]: unknown
}

// Errors
export class MarkdownParseError extends Error {
  readonly _tag = "MarkdownParseError"
}

export class MarkdownSanitizationError extends Error {
  readonly _tag = "MarkdownSanitizationError"
}

// Theme mappings for our WebTUI themes - using themes available in Shiki v3.6.0
const THEME_MAPPINGS: Record<string, string> = {
  zinc: "github-dark",
  catppuccin: "catppuccin-mocha",
  gruvbox: "gruvbox-dark-medium",
  nord: "nord"
}

// Highlighter instance (lazy loaded)
let highlighterPromise: Promise<Highlighter> | null = null

const getHighlighter = async (): Promise<Highlighter> => {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: Object.values(THEME_MAPPINGS),
      langs: [
        "typescript",
        "javascript",
        "jsx",
        "tsx",
        "json",
        "html",
        "css",
        "bash",
        "shell",
        "yaml",
        "markdown",
        "python",
        "rust",
        "go",
        "sql",
        "dockerfile",
        "text"
      ]
    })
  }
  return highlighterPromise
}

// Simple service implementation without complex Effect dependencies
const parseMarkdown = (content: string): ParsedMarkdown => {
  const parsed = matter(content)
  return {
    content: parsed.content,
    data: parsed.data,
    excerpt: parsed.excerpt
  }
}

// Create a custom markdown-it plugin for Shiki highlighting
const createShikiPlugin = (highlighter: Highlighter, theme: string = "github-dark") => {
  return (md: MarkdownIt) => {
    const defaultFence = md.renderer.rules.fence

    md.renderer.rules.fence = (tokens, idx, options, env, renderer) => {
      const token = tokens[idx]
      const lang = token.info.trim() || "text"
      const code = token.content

      try {
        // Use Shiki to highlight the code
        const highlighted = highlighter.codeToHtml(code, {
          lang,
          theme,
          transformers: [
            {
              pre(node) {
                // Add our custom classes for WebTUI styling
                node.properties["is-"] = "pre"
                node.properties["box-"] = "square"
                node.properties["data-language"] = lang
              }
            }
          ]
        })
        return highlighted
      } catch {
        // Fallback to default rendering if highlighting fails
        if (defaultFence) {
          return defaultFence(tokens, idx, options, env, renderer)
        }
        return `<pre is-="pre" box-="square"><code>${md.utils.escapeHtml(code)}</code></pre>`
      }
    }
  }
}

const renderMarkdown = async (markdown: string, theme: string = "zinc"): Promise<string> => {
  const highlighter = await getHighlighter()
  const shikiTheme = THEME_MAPPINGS[theme] || "github-dark"

  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true
  })

  // Add Shiki plugin
  md.use(createShikiPlugin(highlighter, shikiTheme))

  const rendered = md.render(markdown)

  // Create a JSDOM instance for DOMPurify
  const dom = new JSDOM("")
  const window = dom.window as any
  const purify = DOMPurify(window)

  // Configure DOMPurify to allow iframes, embeds, and social media content
  return purify.sanitize(rendered, {
    ADD_TAGS: ["iframe", "script", "blockquote", "div", "pre", "code", "span"],
    ADD_ATTR: [
      "allow",
      "allowfullscreen",
      "frameborder",
      "referrerpolicy",
      "src",
      "width",
      "height",
      "title",
      "class",
      "alt",
      "style",
      "async",
      "charset",
      "data-media-max-width",
      "lang",
      "dir",
      "href",
      "ref_src",
      "is-",
      "box-"
    ],
    ALLOW_DATA_ATTR: true
  })
}

const renderMarkdownWithMetadata = async (
  content: string,
  theme: string = "zinc"
): Promise<{ html: string; metadata: BlogPostMetadata }> => {
  const parsed = parseMarkdown(content)
  const html = await renderMarkdown(parsed.content, theme)

  // Validate metadata
  const metadata = parsed.data as BlogPostMetadata
  if (!metadata.title || !metadata.date) {
    throw new MarkdownParseError("Missing required metadata: title and date")
  }

  return {
    html,
    metadata
  }
}

// Service using Effect
export class MarkdownService extends Context.Tag("@openagentsinc/psionic/MarkdownService")<
  MarkdownService,
  {
    readonly parse: (content: string) => Effect.Effect<ParsedMarkdown, MarkdownParseError>
    readonly render: (
      markdown: string,
      theme?: string
    ) => Effect.Effect<string, MarkdownParseError | MarkdownSanitizationError>
    readonly renderWithMetadata: (content: string, theme?: string) => Effect.Effect<{
      html: string
      metadata: BlogPostMetadata
    }, MarkdownParseError | MarkdownSanitizationError>
  }
>() {
  static readonly Live = Layer.succeed(
    this,
    {
      parse: (content: string) =>
        Effect.try({
          try: () => parseMarkdown(content),
          catch: (error) => new MarkdownParseError(String(error))
        }),

      render: (markdown: string, theme: string = "zinc") =>
        Effect.tryPromise({
          try: () => renderMarkdown(markdown, theme),
          catch: (error) => new MarkdownSanitizationError(String(error))
        }),

      renderWithMetadata: (content: string, theme: string = "zinc") =>
        Effect.tryPromise({
          try: () => renderMarkdownWithMetadata(content, theme),
          catch: (error) => {
            if (error instanceof MarkdownParseError) {
              return error
            }
            return new MarkdownParseError(String(error))
          }
        })
    }
  )
}

// Cache implementation - now includes theme in cache key
const markdownCache = new Map<string, { html: string; metadata: BlogPostMetadata; timestamp: number }>()
const CACHE_TTL = 15 * 60 * 1000 // 15 minutes

export const getCachedOrRender = (
  key: string,
  content: string,
  theme: string = "zinc"
): Effect.Effect<
  { html: string; metadata: BlogPostMetadata },
  MarkdownParseError | MarkdownSanitizationError,
  MarkdownService
> =>
  Effect.gen(function*() {
    const cacheKey = `${key}:${theme}`
    const cached = markdownCache.get(cacheKey)
    const now = Date.now()

    if (cached && now - cached.timestamp < CACHE_TTL) {
      return { html: cached.html, metadata: cached.metadata }
    }

    const service = yield* MarkdownService
    const result = yield* service.renderWithMetadata(content, theme)

    markdownCache.set(cacheKey, {
      html: result.html,
      metadata: result.metadata,
      timestamp: now
    })

    return result
  })

export const clearCache = (key?: string) => {
  if (key) {
    markdownCache.delete(key)
  } else {
    markdownCache.clear()
  }
}

// Utility function to generate slug from filename
export const generateSlug = (filename: string): string => {
  return filename
    .replace(/\.md$/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
}

// Utility function to format date for display
export const formatDate = (dateString: string): string => {
  try {
    return new Date(dateString).toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric"
    })
  } catch {
    return dateString
  }
}

// Synchronous wrapper for backward compatibility - uses default zinc theme
export const renderMarkdownWithMetadataSync = (content: string): { html: string; metadata: BlogPostMetadata } => {
  const parsed = parseMarkdown(content)

  // Validate metadata
  const metadata = parsed.data as BlogPostMetadata
  if (!metadata.title || !metadata.date) {
    throw new MarkdownParseError("Missing required metadata: title and date")
  }

  // For now, return non-highlighted version to maintain sync API
  // This will be replaced when routes are updated to async
  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true
  })

  const rendered = md.render(parsed.content)

  // Create a JSDOM instance for DOMPurify
  const dom = new JSDOM("")
  const window = dom.window as any
  const purify = DOMPurify(window)

  const html = purify.sanitize(rendered, {
    ADD_TAGS: ["iframe", "script", "blockquote", "div", "pre", "code"],
    ADD_ATTR: [
      "allow",
      "allowfullscreen",
      "frameborder",
      "referrerpolicy",
      "src",
      "width",
      "height",
      "title",
      "class",
      "alt",
      "style",
      "async",
      "charset",
      "data-media-max-width",
      "lang",
      "dir",
      "href",
      "ref_src",
      "is-",
      "box-"
    ],
    ALLOW_DATA_ATTR: true
  })

  return { html, metadata }
}

// Direct export functions for simpler usage
export { parseMarkdown }

// Export sync version for backward compatibility
export { renderMarkdownWithMetadataSync as renderMarkdownWithMetadata }

// Export async version for syntax highlighting
export const renderMarkdownWithHighlighting = renderMarkdownWithMetadata as (
  content: string,
  theme?: string
) => Promise<{ html: string; metadata: BlogPostMetadata }>

// Export the core markdown rendering function for direct usage
export { renderMarkdown }
