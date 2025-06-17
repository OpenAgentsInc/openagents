import DOMPurify from "dompurify"
import { Context, Effect, Layer } from "effect"
import matter from "gray-matter"
import { JSDOM } from "jsdom"
import MarkdownIt from "markdown-it"

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

// Simple service implementation without complex Effect dependencies
const parseMarkdown = (content: string): ParsedMarkdown => {
  const parsed = matter(content)
  return {
    content: parsed.content,
    data: parsed.data,
    excerpt: parsed.excerpt
  }
}

const renderMarkdown = (markdown: string): string => {
  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true
  })

  const rendered = md.render(markdown)

  // Create a JSDOM instance for DOMPurify
  const dom = new JSDOM("")
  const window = dom.window as any
  const purify = DOMPurify(window)

  // Configure DOMPurify to allow iframes and specific attributes
  return purify.sanitize(rendered, {
    ADD_TAGS: ["iframe"],
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
      "alt"
    ],
    ALLOW_DATA_ATTR: false
  })
}

const renderMarkdownWithMetadata = (content: string): { html: string; metadata: BlogPostMetadata } => {
  const parsed = parseMarkdown(content)
  const html = renderMarkdown(parsed.content)

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
    readonly render: (markdown: string) => Effect.Effect<string, MarkdownParseError | MarkdownSanitizationError>
    readonly renderWithMetadata: (content: string) => Effect.Effect<{
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

      render: (markdown: string) =>
        Effect.try({
          try: () => renderMarkdown(markdown),
          catch: (error) => new MarkdownSanitizationError(String(error))
        }),

      renderWithMetadata: (content: string) =>
        Effect.try({
          try: () => renderMarkdownWithMetadata(content),
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

// Cache implementation
const markdownCache = new Map<string, { html: string; metadata: BlogPostMetadata; timestamp: number }>()
const CACHE_TTL = 15 * 60 * 1000 // 15 minutes

export const getCachedOrRender = (
  key: string,
  content: string
): Effect.Effect<
  { html: string; metadata: BlogPostMetadata },
  MarkdownParseError | MarkdownSanitizationError,
  MarkdownService
> =>
  Effect.gen(function*() {
    const cached = markdownCache.get(key)
    const now = Date.now()

    if (cached && now - cached.timestamp < CACHE_TTL) {
      return { html: cached.html, metadata: cached.metadata }
    }

    const service = yield* MarkdownService
    const result = yield* service.renderWithMetadata(content)

    markdownCache.set(key, {
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

// Direct export functions for simpler usage
export { parseMarkdown, renderMarkdown, renderMarkdownWithMetadata }
