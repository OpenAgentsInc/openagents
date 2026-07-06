import { Effect } from 'effect'

import { readForumTopicDetail } from '../forum/repository'

// Social preview cards (Open Graph + Twitter Card) for forum thread pages at
// `/forum/t/{topicId}`. Social crawlers (Twitterbot, facebookexternalhit,
// Slackbot, Discordbot, Telegram, Signal) do NOT execute JavaScript, so the
// per-thread title/description/image MUST live in the server-rendered initial
// HTML the Worker returns for the route. We fetch the existing SPA shell HTML
// and inject the meta tags into <head>; the SPA still hydrates normally for
// real users because we only add head metadata and never touch the app body.

const SITE_NAME = 'OpenAgents'
const DEFAULT_TITLE = 'OpenAgents Forum'
const DEFAULT_DESCRIPTION =
  'A public forum where humans and agents coordinate work on OpenAgents.'

// Twitter requires a raster image for the card thumbnail; SVG is rendered
// reliably by Slack/Discord/Telegram/Signal. The per-thread image route below
// returns SVG (zero Worker bundle weight, crisp branded title text). The
// `summary_large_image` card type still renders the title + description even
// where a host declines an SVG thumbnail, so the floor (name + description +
// branded image) holds everywhere.
const OG_IMAGE_WIDTH = 1200
const OG_IMAGE_HEIGHT = 630

const SOCIAL_PREVIEW_MARKER = 'data-openagents-social-preview'

export const escapeHtmlAttribute = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

// XML/SVG text escaping. SVG is XML, so the same five entities apply, but we
// must not emit raw control characters into the document either.
const escapeSvgText = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
    .replace(/[\u0000-\u001f]/g, ' ')

export const forumThreadPublicUrl = (topicId: string): string =>
  `https://openagents.com/forum/t/${encodeURIComponent(topicId)}`

export const forumThreadOgImageUrl = (topicId: string): string =>
  `https://openagents.com/og/forum/${encodeURIComponent(topicId)}.svg`

export const FORUM_DEFAULT_OG_IMAGE_URL =
  'https://openagents.com/og/forum/default.svg'

// Collapse a markdown/plain-text post body into a clean, single-line excerpt
// suitable for a social card description. We strip code fences, inline code,
// link/image markup, headings, blockquote/list markers, emphasis runs, and raw
// URLs, then collapse whitespace and truncate on a word boundary near maxLength.
export const buildSocialPreviewExcerpt = (
  bodyText: string | null | undefined,
  maxLength = 200,
): string => {
  if (bodyText === null || bodyText === undefined) {
    return ''
  }

  const withoutCodeFences = bodyText.replace(/```[\s\S]*?```/g, ' ')
  // Keep inline-code text, drop only the backtick delimiters.
  const withoutInlineCode = withoutCodeFences.replace(/`([^`]*)`/g, '$1')
  const withoutImages = withoutInlineCode.replace(/!\[[^\]]*\]\([^)]*\)/g, ' ')
  // Markdown links: keep the visible label, drop the URL target.
  const withLinkLabels = withoutImages.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
  const withoutBareUrls = withLinkLabels.replace(/https?:\/\/\S+/gi, ' ')
  const withoutHtmlTags = withoutBareUrls.replace(/<[^>]+>/g, ' ')

  const cleaned = withoutHtmlTags
    .split('\n')
    .map(line =>
      line
        // Headings, blockquotes, list markers at the start of a line.
        .replace(/^\s{0,3}#{1,6}\s+/, '')
        .replace(/^\s{0,3}>+\s?/, '')
        .replace(/^\s{0,3}(?:[-*+]|\d+\.)\s+/, ''),
    )
    .join(' ')
    // Emphasis / strikethrough markers.
    .replace(/[*_~]+/g, '')
    // Collapse all remaining whitespace.
    .replace(/\s+/g, ' ')
    .trim()

  if (cleaned.length <= maxLength) {
    return cleaned
  }

  const hardSlice = cleaned.slice(0, maxLength)
  const lastSpace = hardSlice.lastIndexOf(' ')
  const wordSafe =
    lastSpace > maxLength * 0.6 ? hardSlice.slice(0, lastSpace) : hardSlice

  return `${wordSafe.trimEnd()}\u2026`
}

export type ForumThreadSocialPreview = Readonly<{
  title: string
  description: string
  url: string
  imageUrl: string
}>

// Derive the card content from a topic detail response. The first visible post
// (posts are ordered ascending by post_number) supplies the description. Forum
// bodies are public, but we still run them through the excerpt cleaner +
// attribute escaper so no raw markup or control characters reach the head.
// The narrow structural view of a topic detail that the card derives from.
// The full ForumTopicDetailResponse satisfies this, so callers pass it
// directly; tests build a minimal object without type assertions.
export type ForumThreadPreviewSource = Readonly<{
  topic: Readonly<{ title: string; topicId?: string }>
  posts: ReadonlyArray<Readonly<{ bodyText?: string | null }>>
}>

export const forumThreadSocialPreviewFromDetail = (
  topicId: string,
  detail: ForumThreadPreviewSource | null,
): ForumThreadSocialPreview => {
  if (detail === null) {
    return {
      description: DEFAULT_DESCRIPTION,
      imageUrl: FORUM_DEFAULT_OG_IMAGE_URL,
      title: DEFAULT_TITLE,
      url: forumThreadPublicUrl(topicId),
    }
  }

  const title = detail.topic.title.trim()
  const firstPost = detail.posts[0]
  const excerpt = buildSocialPreviewExcerpt(firstPost?.bodyText)

  return {
    description: excerpt.length > 0 ? excerpt : DEFAULT_DESCRIPTION,
    imageUrl: forumThreadOgImageUrl(topicId),
    title: title.length > 0 ? title : DEFAULT_TITLE,
    url: forumThreadPublicUrl(detail.topic.topicId ?? topicId),
  }
}

// Render the Open Graph + Twitter Card meta block for a thread. All values are
// HTML-attribute escaped. The marker attribute lets the injector stay
// idempotent and lets tests assert presence.
export const renderForumThreadSocialPreviewMeta = (
  preview: ForumThreadSocialPreview,
): string => {
  const title = escapeHtmlAttribute(preview.title)
  const description = escapeHtmlAttribute(preview.description)
  const url = escapeHtmlAttribute(preview.url)
  const image = escapeHtmlAttribute(preview.imageUrl)
  const siteName = escapeHtmlAttribute(SITE_NAME)

  return [
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:type" content="article">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:site_name" content="${siteName}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:title" content="${title}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:description" content="${description}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:url" content="${url}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:image" content="${image}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:image:width" content="${OG_IMAGE_WIDTH}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} property="og:image:height" content="${OG_IMAGE_HEIGHT}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} name="twitter:card" content="summary_large_image">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} name="twitter:title" content="${title}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} name="twitter:description" content="${description}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} name="twitter:image" content="${image}">`,
    `<meta ${SOCIAL_PREVIEW_MARKER} name="description" content="${description}">`,
  ].join('\n    ')
}

// Inject the meta block immediately before </head>. Idempotent: a shell that
// already carries a social-preview block (e.g. double-processed) is returned
// unchanged.
export const injectForumThreadSocialPreviewMeta = (
  html: string,
  preview: ForumThreadSocialPreview,
): string => {
  if (html.includes(SOCIAL_PREVIEW_MARKER)) {
    return html
  }

  const meta = renderForumThreadSocialPreviewMeta(preview)
  const lower = html.toLowerCase()
  const index = lower.lastIndexOf('</head>')

  if (index < 0) {
    return `${html}\n    ${meta}`
  }

  return `${html.slice(0, index)}    ${meta}\n  ${html.slice(index)}`
}

// Break a title into wrapped lines for the SVG image. Word-wrap by an
// approximate character budget; cap the number of rendered lines so a very long
// title degrades gracefully with an ellipsis instead of overflowing the canvas.
const wrapTitleLines = (
  title: string,
  maxCharsPerLine: number,
  maxLines: number,
): ReadonlyArray<string> => {
  const words = title.split(/\s+/).filter(word => word.length > 0)
  const lines: Array<string> = []
  let current = ''

  for (const word of words) {
    const candidate = current.length === 0 ? word : `${current} ${word}`
    if (candidate.length <= maxCharsPerLine || current.length === 0) {
      current = candidate
    } else {
      lines.push(current)
      current = word
      if (lines.length === maxLines) {
        break
      }
    }
  }

  if (lines.length < maxLines && current.length > 0) {
    lines.push(current)
  }

  if (lines.length > maxLines) {
    lines.length = maxLines
  }

  if (lines.length === maxLines) {
    const renderedWordCount = lines.join(' ').split(/\s+/).length
    if (renderedWordCount < words.length) {
      const last = lines[maxLines - 1] ?? ''
      lines[maxLines - 1] = `${last.replace(/[\s.]+$/, '')}\u2026`
    }
  }

  return lines.length > 0 ? lines : [DEFAULT_TITLE]
}

// Build a 1200x630 branded SVG OG image for a thread. Pure string output, no
// wasm rasterizer, so it adds no Worker bundle weight. Uses the Khala
// StarCraft palette so the share card matches the click-through forum.
export const buildForumThreadOgImageSvg = (
  title: string | null | undefined,
): string => {
  const safeTitle =
    title === null || title === undefined || title.trim().length === 0
      ? DEFAULT_TITLE
      : title.trim()

  const lines = wrapTitleLines(safeTitle, 26, 4)
  const lineHeight = 86
  const blockHeight = lines.length * lineHeight
  const startY = 250 + (Math.max(0, 4 - lines.length) * lineHeight) / 2

  const titleTexts = lines
    .map(
      (line, index) =>
        `<text x="80" y="${startY + index * lineHeight}" font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="64" font-weight="700" fill="#e7f4ff">${escapeSvgText(line)}</text>`,
    )
    .join('')

  const footerY = Math.min(560, startY + blockHeight + 40)

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${OG_IMAGE_WIDTH}" height="${OG_IMAGE_HEIGHT}" viewBox="0 0 ${OG_IMAGE_WIDTH} ${OG_IMAGE_HEIGHT}" role="img" aria-label="${escapeSvgText(safeTitle)}">
  <rect width="${OG_IMAGE_WIDTH}" height="${OG_IMAGE_HEIGHT}" fill="#05080e"/>
  <rect x="40" y="40" width="${OG_IMAGE_WIDTH - 80}" height="${OG_IMAGE_HEIGHT - 80}" fill="none" stroke="#1d2a44" stroke-width="2"/>
  <rect x="80" y="96" width="56" height="6" fill="#3a7bff"/>
  <text x="152" y="112" font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="30" font-weight="700" letter-spacing="3" fill="#cdeeff">OPENAGENTS FORUM</text>
  ${titleTexts}
  <text x="80" y="${footerY}" font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="26" fill="#8fb6ff">openagents.com/forum</text>
</svg>`
}

const SVG_CONTENT_TYPE = 'image/svg+xml; charset=utf-8'

export const forumThreadOgImageResponse = (
  title: string | null | undefined,
): Response =>
  new Response(buildForumThreadOgImageSvg(title), {
    headers: {
      'cache-control': 'public, max-age=300',
      'content-type': SVG_CONTENT_TYPE,
    },
  })

// Wrap a freshly fetched SPA shell response with the per-thread social preview
// meta. Only mutates successful HTML responses; preserves status/headers
// otherwise so the SPA experience and any session-cookie handling upstream are
// untouched. Mirrors `injectPylonStatsBootPayloadIntoAssetResponse`. Plain
// async (no Effect.runPromise) so the index.ts route handler stays free of new
// temporary Effect bridges.
export const withForumThreadSocialPreview = async (
  shellResponse: Response,
  preview: ForumThreadSocialPreview,
): Promise<Response> => {
  const contentType =
    shellResponse.headers.get('content-type')?.toLowerCase() ?? ''

  if (!shellResponse.ok || !contentType.includes('text/html')) {
    return shellResponse
  }

  const html = await shellResponse.text()
  const headers = new Headers(shellResponse.headers)
  headers.delete('content-length')

  return new Response(injectForumThreadSocialPreviewMeta(html, preview), {
    headers,
    status: shellResponse.status,
    statusText: shellResponse.statusText,
  })
}

// Named Effect->Promise bridge for the forum thread document SSR path. Reading
// the topic detail is the only Effect step the document handler needs; running
// it here keeps the index.ts route handler free of new Effect.runPromise calls
// (matching the pylon-stats boot-payload bridge precedent). A missing topic or
// any read failure resolves to null so the caller serves the default card.
export const readForumThreadPreview = async (
  db: D1Database,
  // CFG-4 (#8519): credited tip totals in the detail read the Postgres ledger.
  ledgerDb: import('../payments-ledger-db').PaymentsLedgerDb,
  topicId: string,
): Promise<ForumThreadSocialPreview> => {
  const detail = await Effect.runPromise(
    readForumTopicDetail(db, ledgerDb, topicId).pipe(
      Effect.catch(() => Effect.succeed(null)),
    ),
  )

  return forumThreadSocialPreviewFromDetail(topicId, detail)
}

// Produce the forum thread document Response: fetch the SPA shell, derive the
// per-thread preview, and inject the OG/Twitter meta. The app-shell fetch is
// injected so this HTTP module stays free of the worker's auth/env wiring while
// still owning the Response-shaping step (keeping that surface out of index.ts).
export const handleForumThreadDocument = async (
  options: Readonly<{
    db: D1Database
    ledgerDb: import('../payments-ledger-db').PaymentsLedgerDb
    topicId: string
    fetchAppShell: () => Promise<Response>
  }>,
): Promise<Response> => {
  const [shellResponse, preview] = await Promise.all([
    options.fetchAppShell(),
    readForumThreadPreview(options.db, options.ledgerDb, options.topicId),
  ])

  return withForumThreadSocialPreview(shellResponse, preview)
}
