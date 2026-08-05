// APP-FORUM (#8635) — the retained /forum* presentation inside the one
// OpenAgents web app.
//
// Converted from an Effect Native view tree to plain React (#9325).
//
// Route surface (deep-link stable, identical URL contract to the legacy
// Foldkit page in apps/web/src/page/forum.ts):
//   /forum                      board index
//   /forum/f/$forumRef          forum topic list
//   /forum/t/$topicId           topic posts (?sortDir=asc|desc, #post-<id>)
//   /forum/receipts/$receiptRef payment receipt
//
// Authority boundary: this file is presentation only. All Forum reads hit the
// existing public Worker projections. Content writes (topics, replies, edits,
// tombstones), moderation, locks, identity, and work-request authority remain
// on the Worker's /api/forum* contracts untouched.
//
// Styling note: the same components are served two ways — through TanStack
// Start (Tailwind + the khala palette present) and through the Cloud Run
// monolith, which packs `src/forum-entry.ts` into one standalone IIFE bundle
// with no stylesheet of its own (workers/api/scripts/deploy-cloudrun.sh). A
// colocated `.css` file cannot reach that second host, so the surface carries
// its own small stylesheet below. Every color reads a `--khala-*` custom
// property with the khalaTheme hex as the fallback, so both hosts agree.

import { khalaTheme } from '@openagentsinc/design-tokens'
import { Effect } from 'effect'
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { createRoot } from 'react-dom/client'

import { useScopedEffect } from '@/lib/use-scoped-effect'

import {
  actorDisplayName,
  actorInitial,
  actorProfilePath,
  actorRole,
  fetchForumAuthMode,
  fetchForumIndex,
  fetchForumReceipt,
  fetchForumSummary,
  fetchForumTopicDetail,
  fetchForumTopics,
  forumBoardPath,
  forumPath,
  forumRefPath,
  forumStatusLabel,
  friendlyTime,
  lastPostProjection,
  parseTopicPostSortDirection,
  postAnchor,
  postCountText,
  postNumberAnchor,
  postPath,
  receiptActionText,
  receiptAmountText,
  receiptPath,
  replyCountText,
  topicCountText,
  topicPath,
  topicSortPath,
  topicStatusLabel,
  viewCountText,
  type ForumAuthMode,
  type ForumPostProjection,
  type ForumReceiptProjection,
  type ForumSummaryProjection,
  type ForumTopicPostSortDirection,
  type ForumTopicProjection,
} from './-forum-data'
import {
  mountForumBoardAssembly,
  type ForumKhalaAssemblyDependencies,
} from './-forum-khala-motion'
import {
  parseForumMarkdown,
  type MarkdownBlock,
  type MarkdownInline,
} from './-forum-markdown'

// ---------------------------------------------------------------------------
// Route params + state
// ---------------------------------------------------------------------------

export type ForumRouteParams =
  | Readonly<{ kind: 'index' }>
  | Readonly<{ kind: 'forum'; forumRef: string }>
  | Readonly<{
      kind: 'topic'
      topicId: string
      sortDirection: ForumTopicPostSortDirection
    }>
  | Readonly<{ kind: 'receipt'; receiptRef: string }>

export type ForumPageState = Readonly<{
  params: ForumRouteParams
  phase: 'loading' | 'ready' | 'unavailable'
  errorMessage: string
  authMode: ForumAuthMode
  forums: ReadonlyArray<ForumSummaryProjection>
  forum: ForumSummaryProjection | null
  topics: ReadonlyArray<ForumTopicProjection>
  topic: ForumTopicProjection | null
  posts: ReadonlyArray<ForumPostProjection>
  receipt: ForumReceiptProjection | null
  copiedPermalinkPostId: string | null
}>

export const initialForumPageState = (
  params: ForumRouteParams,
): ForumPageState => ({
  params,
  phase: 'loading',
  errorMessage: '',
  authMode: 'LoggedOut',
  forums: [],
  forum: null,
  topics: [],
  topic: null,
  posts: [],
  receipt: null,
  copiedPermalinkPostId: null,
})

/** The route's return path — also the GitHub login `returnTo` target. */
export const forumReturnPath = (params: ForumRouteParams): string =>
  params.kind === 'forum'
    ? forumRefPath(params.forumRef)
    : params.kind === 'topic'
      ? topicPath(params.topicId)
      : params.kind === 'receipt'
        ? receiptPath(params.receiptRef)
        : forumBoardPath

// ---------------------------------------------------------------------------
// Stylesheet
// ---------------------------------------------------------------------------

export const forumSurfaceStyles = `
.forum-page {
  background: var(--khala-void, #05070d);
  color: var(--khala-text, #eef3ff);
  font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  min-height: 100%;
}
.forum-root {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin: 0 auto;
  max-width: 1180px;
  padding: 16px;
  width: 100%;
}
.forum-root *,
.forum-root *::before,
.forum-root *::after { box-sizing: border-box; }
.forum-text { display: block; margin: 0; width: 100%; }
.forum-heading { font-size: 24px; font-weight: 600; line-height: 30px; }
.forum-title { font-size: 18px; font-weight: 600; line-height: 24px; }
.forum-label { font-size: 13px; font-weight: 600; line-height: 18px; }
.forum-body { font-size: 14px; font-weight: 400; line-height: 21px; }
.forum-caption { font-size: 12px; font-weight: 500; line-height: 16px; }
.forum-muted { color: var(--khala-text-muted, #93a4c3); }
.forum-accent { color: var(--khala-energy, #3b82f6); }
.forum-link { color: var(--khala-energy, #3b82f6); text-decoration: none; }
.forum-link:hover { text-decoration: underline; }
.forum-link:focus-visible {
  outline: 2px solid var(--khala-energy-cyan, #38bdf8);
  outline-offset: 2px;
}
.forum-frame {
  border: none;
  isolation: isolate;
  overflow: visible;
  position: relative;
  width: 100%;
}
.forum-frame-decoration {
  height: 100%;
  inset: 0;
  overflow: visible;
  pointer-events: none;
  position: absolute;
  width: 100%;
  z-index: 0;
}
.forum-frame-content { position: relative; z-index: 1; }
.forum-crumbs {
  background: var(--khala-surface-raised, #141f36);
  border: 1px solid var(--khala-border, #1f2b45);
}
.forum-crumbs-row {
  align-items: center;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: 8px;
  padding: 12px;
}
.forum-board {
  background: var(--khala-surface, #0b1220);
  border: 1px solid var(--khala-border, #1f2b45);
}
.forum-board-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}
.forum-panel {
  background: var(--khala-surface, #0b1220);
  border: 1px solid var(--khala-border, #1f2b45);
  border-radius: 4px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  width: 100%;
}
.forum-card {
  border: 1px solid var(--khala-border, #1f2b45);
  border-radius: 4px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  width: 100%;
}
.forum-card-even { background: var(--khala-surface, #0b1220); }
.forum-card-odd { background: var(--khala-surface-raised, #141f36); }
.forum-row {
  align-items: center;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: 8px;
}
.forum-row-loose { gap: 12px; }
.forum-col { display: flex; flex-direction: column; gap: 4px; }
.forum-col-tight { display: flex; flex-direction: column; gap: 0; }
.forum-badge {
  border-radius: 9999px;
  display: inline-flex;
  font-size: 12px;
  font-weight: 500;
  line-height: 16px;
  padding: 2px 8px;
}
.forum-badge-neutral {
  background: var(--khala-surface-raised, #141f36);
  color: var(--khala-text-muted, #93a4c3);
}
.forum-badge-warn {
  background: var(--khala-surface-raised, #141f36);
  color: var(--khala-warning, #f59e0b);
}
.forum-badge-info {
  background: var(--khala-surface-raised, #141f36);
  color: var(--khala-energy-cyan, #38bdf8);
}
.forum-banner {
  align-items: center;
  background: var(--khala-surface-raised, #141f36);
  border: 1px solid var(--khala-danger, #f87171);
  border-radius: 4px;
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  width: 100%;
}
.forum-post-grid {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: 16px;
  width: 100%;
}
.forum-post-aside {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 160px;
}
.forum-post-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}
.forum-permalink {
  background: var(--khala-surface, #0b1220);
  border: 1px solid var(--khala-border, #1f2b45);
  border-radius: 4px;
  color: var(--khala-energy, #3b82f6);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  line-height: 16px;
  padding: 4px 8px;
}
.forum-markdown { width: 100%; }
.forum-markdown p,
.forum-markdown ul,
.forum-markdown ol,
.forum-markdown blockquote,
.forum-markdown h4,
.forum-markdown h5,
.forum-markdown h6 { margin: 0 0 8px; }
.forum-markdown h4 { font-size: 15px; line-height: 20px; }
.forum-markdown h5 { font-size: 14px; line-height: 19px; }
.forum-markdown h6 { font-size: 13px; line-height: 18px; }
.forum-markdown code {
  background: var(--khala-surface-muted, #0a0f1c);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  padding: 0 4px;
}
.forum-markdown blockquote {
  border-left: 2px solid var(--khala-border-strong, #2c3d63);
  padding-left: 12px;
}
.forum-markdown a { color: var(--khala-energy, #3b82f6); }
.forum-code {
  background: var(--khala-surface-muted, #0a0f1c);
  border-radius: 4px;
  margin: 0;
  overflow-x: auto;
  padding: 12px;
}
.forum-code pre {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  margin: 0;
  white-space: pre;
}
.forum-rule {
  border: 0;
  border-top: 1px solid var(--khala-border, #1f2b45);
  margin: 0;
  width: 100%;
}
`

function ForumStyleSheet() {
  return <style data-forum-styles="">{forumSurfaceStyles}</style>
}

// ---------------------------------------------------------------------------
// Khala frame decoration
//
// The board panel and the breadcrumb band keep their inert Khala outlines.
// The geometry is the closed two-motif subset this surface uses, resolved from
// the shared `khalaTheme.khalaUi` token block so the drawing stays theme data,
// not renderer constants. The decoration is `aria-hidden`, sits behind the
// content layer, and is the element `-forum-khala-motion.ts` animates.
// ---------------------------------------------------------------------------

type ForumKhalaMotif = 'cut-corner-surface' | 'signal-separator'
type KhalaLuminance = 'structural' | 'signal'

type KhalaSegment = Readonly<{
  from: readonly [number, number]
  to: readonly [number, number]
  role: KhalaLuminance
  width: number
}>

const khalaStrokeColor: Readonly<Record<KhalaLuminance, string>> = {
  structural: 'var(--khala-border-strong, #2c3d63)',
  signal: 'var(--khala-energy, #3b82f6)',
}

const khalaCoordinate = (value: number): string =>
  Number(value.toFixed(4)).toString()

const khalaMove = (x: number, y: number): string =>
  `M${khalaCoordinate(x)} ${khalaCoordinate(y)}`

const khalaLineTo = (x: number, y: number): string =>
  `L${khalaCoordinate(x)} ${khalaCoordinate(y)}`

export const khalaFrameDomId = (id: string): string =>
  `en-khala-${id.replace(/[^a-zA-Z0-9_-]/g, '-')}`

type KhalaPathGroup = Readonly<{
  id: string
  role: KhalaLuminance
  width: number
  data: string
}>

const khalaFramePaths = (
  id: string,
  motif: ForumKhalaMotif,
  width: number,
  height: number,
): ReadonlyArray<KhalaPathGroup> => {
  const ui = khalaTheme.khalaUi
  const density = ui.density.compact
  const collapse =
    width < ui.responsiveCollapse.borderOnlyBelow
      ? 'border-only'
      : width < ui.responsiveCollapse.simplifiedBelow
        ? 'simplified'
        : 'full'
  const domId = khalaFrameDomId(id)

  if (motif === 'cut-corner-surface') {
    const rawCut =
      collapse === 'full'
        ? ui.cutSize[density.cut]
        : collapse === 'simplified'
          ? ui.cutSize.small
          : 0
    const cut = Math.min(rawCut, width / 4, height / 4)
    const points: ReadonlyArray<readonly [number, number]> =
      cut === 0
        ? [
            [0, 0],
            [width, 0],
            [width, height],
            [0, height],
          ]
        : [
            [cut, 0],
            [width - cut, 0],
            [width, cut],
            [width, height - cut],
            [width - cut, height],
            [cut, height],
            [0, height - cut],
            [0, cut],
          ]
    const [first, ...rest] = points
    return first === undefined
      ? []
      : [
          {
            id: `${domId}-path-0`,
            role: 'structural',
            width: 1,
            data: `${khalaMove(first[0], first[1])} ${rest
              .map((point) => khalaLineTo(point[0], point[1]))
              .join(' ')} Z`,
          },
        ]
  }

  const accentLength = Math.min(ui.accentLength[density.accent], width / 2)
  const center = width / 2
  const halfAccent = accentLength / 2
  const gap = density.gap
  const segment = (
    fromX: number,
    toX: number,
    role: KhalaLuminance,
    strokeWidth: number,
  ): KhalaSegment => ({
    from: [fromX, 0],
    to: [toX, 0],
    role,
    width: strokeWidth,
  })
  const segments: ReadonlyArray<KhalaSegment> =
    collapse === 'full'
      ? [
          segment(0, center - halfAccent - gap, 'structural', ui.edgeWidth.structural),
          segment(center - halfAccent, center + halfAccent, 'signal', ui.edgeWidth.emphasis),
          segment(center + halfAccent + gap, width, 'structural', ui.edgeWidth.structural),
        ]
      : collapse === 'simplified'
        ? [segment(center - halfAccent, center + halfAccent, 'signal', ui.edgeWidth.emphasis)]
        : [segment(0, width, 'structural', ui.edgeWidth.structural)]

  // Same grouping the static resolver used: one path per role+width pair, in
  // first-appearance order.
  const grouped = new Map<string, { role: KhalaLuminance; width: number; data: string }>()
  for (const value of segments) {
    const key = `${value.role}:${value.width}`
    const data = `${khalaMove(value.from[0], value.from[1])} ${khalaLineTo(value.to[0], value.to[1])}`
    const current = grouped.get(key)
    if (current === undefined) {
      grouped.set(key, { role: value.role, width: value.width, data })
    } else {
      current.data = `${current.data} ${data}`
    }
  }
  return [...grouped.values()].map((value, index) => ({
    id: `${domId}-path-${index}`,
    role: value.role,
    width: value.width,
    data: value.data,
  }))
}

function KhalaFrame({
  children,
  className,
  frameKey,
  height,
  id,
  motif,
  width,
}: Readonly<{
  children: ReactNode
  className: string
  frameKey: string
  height: number
  id: string
  motif: ForumKhalaMotif
  width: number
}>) {
  const paths = khalaFramePaths(id, motif, width, height)
  return (
    <div
      className={`forum-frame ${className}`}
      data-en-key={frameKey}
      data-en-khala={motif}
    >
      <svg
        aria-hidden="true"
        className="forum-frame-decoration"
        data-en-khala-decoration="true"
        data-en-khala-decorative-nodes={String(paths.length + 1)}
        focusable="false"
        id={khalaFrameDomId(id)}
        preserveAspectRatio="none"
        viewBox={`0 0 ${width} ${height}`}
      >
        {paths.map((path) => (
          <path
            d={path.data}
            data-en-khala-role={path.role}
            fill="none"
            id={path.id}
            key={path.id}
            stroke={khalaStrokeColor[path.role]}
            strokeWidth={String(path.width)}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      <div className="forum-frame-content" data-en-khala-content="true">
        {children}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Small presentation pieces
// ---------------------------------------------------------------------------

type TextTone = 'primary' | 'muted' | 'accent'

const toneClass = (tone: TextTone): string =>
  tone === 'muted' ? ' forum-muted' : tone === 'accent' ? ' forum-accent' : ''

function BodyText({
  children,
  tone = 'primary',
  variant = 'body',
}: Readonly<{
  children: ReactNode
  tone?: TextTone
  variant?: 'body' | 'caption' | 'label' | 'title'
}>) {
  return (
    <p className={`forum-text forum-${variant}${toneClass(tone)}`}>{children}</p>
  )
}

function PathLink({
  children,
  className,
  href,
}: Readonly<{ children: ReactNode; className: string; href: string }>) {
  return (
    <a className={`forum-link ${className}`} href={href}>
      {children}
    </a>
  )
}

function Badge({
  children,
  tone,
}: Readonly<{ children: ReactNode; tone: 'neutral' | 'warn' | 'info' }>) {
  return <span className={`forum-badge forum-badge-${tone}`}>{children}</span>
}

const forumBoardFrameSize = [1180, 640] as const
const forumBreadcrumbFrameSize = [1180, 56] as const

type BreadcrumbItem = Readonly<{ key: string; label: string; path?: string }>

const boardBreadcrumbItem: BreadcrumbItem = {
  key: 'crumb-board',
  label: 'Board index',
  path: forumBoardPath,
}

function Breadcrumb({
  frameKey,
  trail,
}: Readonly<{ frameKey: string; trail: ReadonlyArray<BreadcrumbItem> }>) {
  return (
    <KhalaFrame
      className="forum-crumbs"
      frameKey={frameKey}
      height={forumBreadcrumbFrameSize[1]}
      id={`${frameKey}-status-band`}
      motif="signal-separator"
      width={forumBreadcrumbFrameSize[0]}
    >
      <nav aria-label="Forum breadcrumb" className="forum-crumbs-row">
        {trail.map((item, index) => (
          <span className="forum-row" key={item.key}>
            {index === 0 ? null : (
              <span className="forum-label forum-muted">»</span>
            )}
            {item.path === undefined ? (
              <span className="forum-label forum-muted">{item.label}</span>
            ) : (
              <PathLink className="forum-label" href={item.path}>
                {item.label}
              </PathLink>
            )}
          </span>
        ))}
      </nav>
    </KhalaFrame>
  )
}

const lastPostSummary = (
  item: Readonly<{
    lastPost?: ForumPostProjection | null
    lastPostSummary?: ForumPostProjection | null
    latestPost?: ForumPostProjection | null
  }>,
  nowMs: number,
): string => {
  const lastPost = lastPostProjection(item)
  if (lastPost === null) {
    return 'No posts'
  }
  const subject =
    lastPost.subject ?? lastPost.title ?? lastPost.topicTitle ?? 'Last post'
  const author =
    lastPost.author?.displayName ??
    lastPost.author?.actorRef ??
    lastPost.authorDisplayName ??
    lastPost.actorRef ??
    'Unknown'
  const time = friendlyTime(
    lastPost.createdAt ?? lastPost.updatedAt ?? lastPost.timestamp,
    nowMs,
  )
  return `${subject} — by ${author} » ${time}`
}

function UnavailableBanner({ message }: Readonly<{ message: string }>) {
  return (
    <div aria-live="assertive" className="forum-banner" role="alert">
      <span className="forum-body">
        {message === '' ? 'Forum unavailable' : `Forum unavailable · ${message}`}
      </span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Markdown rendering
//
// The parsed model is a closed union (see `-forum-markdown.ts`). Every node is
// mapped to a specific element here; there is no HTML pass-through and no
// raw-innerHTML escape hatch anywhere on this surface. `-forum.test.tsx`
// asserts that as a source-level boundary.
// ---------------------------------------------------------------------------

function MarkdownInlines({
  nodes,
}: Readonly<{ nodes: ReadonlyArray<MarkdownInline> }>) {
  return (
    <>
      {nodes.map((node, index) => {
        const key = `inline-${index}`
        switch (node.kind) {
          case 'text':
            return <span key={key}>{node.text}</span>
          case 'code':
            return <code key={key}>{node.text}</code>
          case 'strong':
            return (
              <strong key={key}>
                <MarkdownInlines nodes={node.children} />
              </strong>
            )
          case 'emphasis':
            return (
              <em key={key}>
                <MarkdownInlines nodes={node.children} />
              </em>
            )
          case 'link':
            return (
              <a href={node.href} key={key} rel="noopener noreferrer">
                <MarkdownInlines nodes={node.children} />
              </a>
            )
        }
      })}
    </>
  )
}

function MarkdownBlocks({
  blocks,
}: Readonly<{ blocks: ReadonlyArray<MarkdownBlock> }>) {
  return (
    <>
      {blocks.map((block, index) => {
        const key = `block-${index}`
        switch (block.kind) {
          case 'heading': {
            const Heading = `h${block.level}` as 'h1'
            return (
              <Heading key={key}>
                <MarkdownInlines nodes={block.children} />
              </Heading>
            )
          }
          case 'paragraph':
            return (
              <p key={key}>
                <MarkdownInlines nodes={block.children} />
              </p>
            )
          case 'list': {
            const List = block.ordered ? 'ol' : 'ul'
            return (
              <List key={key}>
                {block.items.map((item, itemIndex) => (
                  <li key={`${key}-item-${itemIndex}`}>
                    <MarkdownBlocks blocks={item} />
                  </li>
                ))}
              </List>
            )
          }
          case 'blockquote':
            return (
              <blockquote key={key}>
                <MarkdownBlocks blocks={block.children} />
              </blockquote>
            )
        }
      })}
    </>
  )
}

/**
 * Plain, unhighlighted code — the same posture the previous code-block
 * presentation had (it painted pre-tokenized lines and this page only ever
 * produced plain tokens). `shiki` is intentionally not pulled in here.
 */
function ForumCodeBlock({
  code,
  language,
}: Readonly<{ code: string; language: string | undefined }>) {
  return (
    <figure
      className="forum-code"
      {...(language === undefined ? {} : { 'data-forum-language': language })}
    >
      <pre>
        <code>{code}</code>
      </pre>
    </figure>
  )
}

function MarkdownBody({ body }: Readonly<{ body: string }>) {
  return (
    <>
      {parseForumMarkdown(body).map((segment, index) => {
        const key = `segment-${index}`
        if (segment.kind === 'markdown') {
          return (
            <div className="forum-markdown" key={key}>
              <MarkdownBlocks blocks={segment.blocks} />
            </div>
          )
        }
        if (segment.kind === 'code') {
          return (
            <ForumCodeBlock code={segment.code} key={key} language={segment.language} />
          )
        }
        return <hr className="forum-rule" key={key} />
      })}
    </>
  )
}

// ---------------------------------------------------------------------------
// Index view
// ---------------------------------------------------------------------------

function ForumRow({
  forum,
  index,
  nowMs,
}: Readonly<{
  forum: ForumSummaryProjection
  index: number
  nowMs: number
}>) {
  return (
    <section
      className={`forum-card ${index % 2 === 0 ? 'forum-card-even' : 'forum-card-odd'}`}
      data-en-key={`forum-row-${forum.slug ?? forum.forumId ?? index}`}
    >
      <div className="forum-col">
        <h2 className="forum-text forum-title">
          <PathLink className="forum-title" href={forumPath(forum)}>
            {forum.title ?? 'Forum'}
          </PathLink>
        </h2>
        <span className="forum-caption forum-muted">
          {forum.slug ?? forum.forumId ?? ''}
        </span>
      </div>
      <BodyText tone="muted" variant="caption">
        {forum.description ?? forum.summary ?? forumStatusLabel(forum)}
      </BodyText>
      <div className="forum-row">
        <Badge tone="neutral">{topicCountText(forum.topicCount)}</Badge>
        <Badge tone="neutral">{postCountText(forum.postCount)}</Badge>
        {forum.discoverability === 'unlisted' ? (
          <Badge tone="warn">Unlisted</Badge>
        ) : null}
        {forum.locked === true ? <Badge tone="warn">Locked</Badge> : null}
      </div>
      <BodyText tone="muted" variant="caption">
        {lastPostSummary(forum, nowMs)}
      </BodyText>
    </section>
  )
}

function IndexView({
  nowMs,
  state,
}: Readonly<{ nowMs: number; state: ForumPageState }>) {
  return (
    <>
      <Breadcrumb frameKey="forum-index-crumbs" trail={[boardBreadcrumbItem]} />
      <KhalaFrame
        className="forum-board"
        frameKey="forum-index-panel"
        height={forumBoardFrameSize[1]}
        id="forum-board-index"
        motif="cut-corner-surface"
        width={forumBoardFrameSize[0]}
      >
        <div className="forum-board-content">
          <h1 className="forum-text forum-heading">OpenAgents Forum</h1>
          {state.forums.length === 0 ? (
            <BodyText tone="muted">No listed forums yet.</BodyText>
          ) : (
            state.forums.map((forum, index) => (
              <ForumRow
                forum={forum}
                index={index}
                key={`forum-row-${forum.slug ?? forum.forumId ?? index}`}
                nowMs={nowMs}
              />
            ))
          )}
        </div>
      </KhalaFrame>
    </>
  )
}

// ---------------------------------------------------------------------------
// Forum (topic list) view
// ---------------------------------------------------------------------------

function TopicRow({
  index,
  nowMs,
  topic,
}: Readonly<{ index: number; nowMs: number; topic: ForumTopicProjection }>) {
  const postCount = Number(topic.postCount ?? 0)
  const replies = Number(topic.replyCount ?? Math.max(postCount - 1, 0))
  const views = Number(topic.viewCount ?? topic.views ?? 0)
  const statusLabel = topicStatusLabel(topic)
  return (
    <section
      className={`forum-card ${index % 2 === 0 ? 'forum-card-even' : 'forum-card-odd'}`}
      data-en-key={`topic-row-${topic.topicId ?? index}`}
    >
      <h2 className="forum-text forum-title">
        <PathLink className="forum-title" href={topicPath(topic.topicId ?? '')}>
          {topic.title ?? 'Topic'}
        </PathLink>
      </h2>
      <BodyText tone="muted" variant="caption">
        {`by ${topic.author?.displayName ?? 'Unknown'} » ${friendlyTime(topic.createdAt ?? topic.updatedAt, nowMs)}`}
      </BodyText>
      <div className="forum-row">
        <Badge tone="neutral">{replyCountText(replies)}</Badge>
        <Badge tone="neutral">{viewCountText(views)}</Badge>
        {statusLabel === 'Topic' ? null : <Badge tone="warn">{statusLabel}</Badge>}
      </div>
      <BodyText tone="muted" variant="caption">
        {lastPostSummary(topic, nowMs)}
      </BodyText>
    </section>
  )
}

function ForumView({
  nowMs,
  state,
}: Readonly<{ nowMs: number; state: ForumPageState }>) {
  const forum = state.forum
  if (forum === null) {
    return <UnavailableBanner message={state.errorMessage} />
  }
  return (
    <>
      <Breadcrumb
        frameKey="forum-crumbs"
        trail={[
          boardBreadcrumbItem,
          { key: 'crumb-forum', label: forum.title ?? 'Forum' },
        ]}
      />
      <section className="forum-panel" data-en-key="forum-panel">
        <p className="forum-text forum-label forum-accent">Forum</p>
        <h1 className="forum-text forum-heading">{forum.title ?? 'Forum'}</h1>
        <BodyText tone="muted" variant="caption">
          {`${topicCountText(forum.topicCount)} · ${postCountText(forum.postCount)}${forum.locked === true ? ' · Locked' : ''}`}
        </BodyText>
        {state.topics.length === 0 ? (
          <BodyText tone="muted">No topics yet.</BodyText>
        ) : (
          state.topics.map((topic, index) => (
            <TopicRow
              index={index}
              key={`topic-row-${topic.topicId ?? index}`}
              nowMs={nowMs}
              topic={topic}
            />
          ))
        )}
      </section>
    </>
  )
}

// ---------------------------------------------------------------------------
// Topic (posts) view
// ---------------------------------------------------------------------------

function AuthorAside({
  nowMs,
  post,
}: Readonly<{ nowMs: number; post: ForumPostProjection }>) {
  const actor = post.author ?? null
  const displayName = actorDisplayName(actor)
  const profilePath = actorProfilePath(actor)
  const postCount = actor?.postCount ?? actor?.forumPostCount ?? post.authorPostCount
  const joinedAt = actor?.joinedAt ?? actor?.firstSeenAt ?? post.authorFirstSeenAt
  return (
    <div className="forum-post-aside">
      <div className="forum-row">
        <Badge tone="info">{actorInitial(actor)}</Badge>
        <div className="forum-col-tight">
          {profilePath === null ? (
            <span className="forum-label">{displayName}</span>
          ) : (
            <PathLink className="forum-label" href={profilePath}>
              {displayName}
            </PathLink>
          )}
          <span className="forum-caption forum-muted">{actorRole(actor)}</span>
        </div>
      </div>
      {postCount == null ? null : (
        <BodyText tone="muted" variant="caption">{`Posts: ${postCount}`}</BodyText>
      )}
      {joinedAt == null ? null : (
        <BodyText tone="muted" variant="caption">
          {`Joined: ${friendlyTime(joinedAt, nowMs)}`}
        </BodyText>
      )}
    </div>
  )
}

function PostArticle({
  index,
  nowMs,
  onCopyPermalink,
  post,
  state,
}: Readonly<{
  index: number
  nowMs: number
  onCopyPermalink: (postId: string, href: string) => void
  post: ForumPostProjection
  state: ForumPageState
}>) {
  const topicId = state.topic?.topicId ?? ''
  const anchor = postAnchor(post)
  const postNumber = Number(post.postNumber ?? 0)
  const subject =
    post.subject ?? post.title ?? state.topic?.title ?? `Post #${postNumber}`
  const href = postPath(topicId, post)
  const copied = state.copiedPermalinkPostId === (post.postId ?? '')
  return (
    <section
      className={`forum-card ${index % 2 === 0 ? 'forum-card-even' : 'forum-card-odd'}`}
      data-en-key={anchor}
      id={anchor}
    >
      <div className="forum-post-grid">
        <AuthorAside nowMs={nowMs} post={post} />
        <div className="forum-post-body">
          {/* Post-number marker keeps `#post-<n>` deep links resolvable. */}
          <div className="forum-col" data-en-key={postNumberAnchor(post)} id={postNumberAnchor(post)}>
            <h2 className="forum-text forum-title">
              <PathLink className="forum-title" href={href}>
                {subject}
              </PathLink>
            </h2>
            <BodyText tone="muted" variant="caption">
              {`Post #${postNumber} » ${friendlyTime(post.createdAt, nowMs)}`}
            </BodyText>
          </div>
          <MarkdownBody body={post.bodyText ?? post.contentRef ?? ''} />
          <div className="forum-row">
            <button
              className="forum-permalink"
              onClick={() => onCopyPermalink(post.postId ?? '', href)}
              type="button"
            >
              {copied ? 'Copied' : 'Permalink'}
            </button>
          </div>
        </div>
      </div>
    </section>
  )
}

function SortToggle({ state }: Readonly<{ state: ForumPageState }>) {
  const params = state.params
  const topicId = params.kind === 'topic' ? params.topicId : ''
  const active = params.kind === 'topic' ? params.sortDirection : 'asc'
  const item = (direction: ForumTopicPostSortDirection, label: string) =>
    active === direction ? (
      <Badge tone="info">{label}</Badge>
    ) : (
      <PathLink className="forum-caption" href={topicSortPath(topicId, direction)}>
        {label}
      </PathLink>
    )
  return (
    <div className="forum-row">
      <span className="forum-caption forum-muted">Post order</span>
      {item('asc', 'Oldest first')}
      {item('desc', 'Newest first')}
    </div>
  )
}

function TopicView({
  nowMs,
  onCopyPermalink,
  state,
}: Readonly<{
  nowMs: number
  onCopyPermalink: (postId: string, href: string) => void
  state: ForumPageState
}>) {
  const topic = state.topic
  if (topic === null) {
    return <UnavailableBanner message={state.errorMessage} />
  }
  const statusLabel = topicStatusLabel(topic)
  return (
    <>
      <Breadcrumb
        frameKey="topic-crumbs"
        trail={[
          boardBreadcrumbItem,
          {
            key: 'crumb-forum',
            label: 'Forum',
            path: forumRefPath(topic.forumId ?? ''),
          },
          { key: 'crumb-topic', label: topic.title ?? 'Topic' },
        ]}
      />
      <section className="forum-panel" data-en-key="topic-panel">
        <p className="forum-text forum-label forum-accent">Thread</p>
        <h1 className="forum-text forum-heading">{topic.title ?? 'Topic'}</h1>
        <div className="forum-row forum-row-loose">
          <span className="forum-caption forum-muted">
            {postCountText(topic.postCount)}
          </span>
          {statusLabel === 'Topic' ? null : <Badge tone="warn">{statusLabel}</Badge>}
          <SortToggle state={state} />
        </div>
        {state.posts.length === 0 ? (
          <BodyText tone="muted">No visible posts yet.</BodyText>
        ) : (
          state.posts.map((post, index) => (
            <PostArticle
              index={index}
              key={postAnchor(post)}
              nowMs={nowMs}
              onCopyPermalink={onCopyPermalink}
              post={post}
              state={state}
            />
          ))
        )}
      </section>
    </>
  )
}

// ---------------------------------------------------------------------------
// Receipt view
// ---------------------------------------------------------------------------

function ReceiptTargetLinks({
  receipt,
}: Readonly<{ receipt: ForumReceiptProjection }>) {
  const links: ReactNode[] = []
  const target = receipt.target ?? null
  if (receipt.targetPostPermalink != null && receipt.targetPostPermalink !== '') {
    links.push(
      <PathLink
        className="forum-body"
        href={receipt.targetPostPermalink}
        key="receipt-target-post"
      >
        Post
      </PathLink>,
    )
  }
  if (target?.topicId != null && target.topicId !== '') {
    links.push(
      <PathLink
        className="forum-body"
        href={topicPath(target.topicId)}
        key="receipt-target-topic"
      >
        Topic
      </PathLink>,
    )
    if (
      (receipt.targetPostPermalink == null || receipt.targetPostPermalink === '') &&
      target.postId != null &&
      target.postId !== ''
    ) {
      links.push(
        <PathLink
          className="forum-body"
          href={`${topicPath(target.topicId)}#post-${encodeURIComponent(target.postId)}`}
          key="receipt-target-topic-post"
        >
          Post
        </PathLink>,
      )
    }
  }
  return (
    <>
      {links.length === 0 ? (
        <BodyText tone="muted">Forum payment</BodyText>
      ) : (
        links
      )}
    </>
  )
}

function ReceiptRow({
  children,
  label,
}: Readonly<{ children: ReactNode; label: string }>) {
  return (
    <div className="forum-col">
      <span className="forum-label forum-accent">{label}</span>
      {children}
    </div>
  )
}

function ReceiptView({
  nowMs,
  state,
}: Readonly<{ nowMs: number; state: ForumPageState }>) {
  const receipt = state.receipt
  if (receipt === null) {
    return <UnavailableBanner message={state.errorMessage} />
  }
  return (
    <>
      <Breadcrumb
        frameKey="receipt-crumbs"
        trail={[boardBreadcrumbItem, { key: 'crumb-receipt', label: 'Receipt' }]}
      />
      <section className="forum-panel" data-en-key="receipt-panel">
        <p className="forum-text forum-label forum-accent">Forum receipt</p>
        <h1 className="forum-text forum-heading">
          {receiptActionText(receipt.actionKind)}
        </h1>
        <BodyText tone="muted" variant="caption">
          {`${receiptAmountText(receipt.amount)} · ${friendlyTime(receipt.createdAt, nowMs)}`}
        </BodyText>
        <ReceiptRow label="Receipt">
          <BodyText>{receipt.receiptRef ?? ''}</BodyText>
        </ReceiptRow>
        <ReceiptRow label="Target">
          <ReceiptTargetLinks receipt={receipt} />
        </ReceiptRow>
        <ReceiptRow label="Recipient">
          <BodyText>
            {receipt.recipientActorRef ?? 'OpenAgents moderation pool'}
          </BodyText>
        </ReceiptRow>
      </section>
    </>
  )
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

const noopCopyPermalink = (): void => undefined

export function ForumPageView({
  nowMs = Date.now(),
  onCopyPermalink = noopCopyPermalink,
  state,
}: Readonly<{
  nowMs?: number
  onCopyPermalink?: (postId: string, href: string) => void
  state: ForumPageState
}>) {
  return (
    <div className="forum-root" data-en-key="forum-root">
      {state.phase === 'loading' ? (
        <>
          <Breadcrumb frameKey="loading-crumbs" trail={[boardBreadcrumbItem]} />
          <section className="forum-panel" data-en-key="forum-loading">
            <BodyText tone="muted">Loading…</BodyText>
          </section>
        </>
      ) : state.phase === 'unavailable' ? (
        <>
          <Breadcrumb
            frameKey="unavailable-crumbs"
            trail={[boardBreadcrumbItem]}
          />
          <UnavailableBanner message={state.errorMessage} />
        </>
      ) : state.params.kind === 'index' ? (
        <IndexView nowMs={nowMs} state={state} />
      ) : state.params.kind === 'forum' ? (
        <ForumView nowMs={nowMs} state={state} />
      ) : state.params.kind === 'topic' ? (
        <TopicView nowMs={nowMs} onCopyPermalink={onCopyPermalink} state={state} />
      ) : (
        <ReceiptView nowMs={nowMs} state={state} />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

export type ForumSurfaceDependencies = Readonly<{
  fetchFn?: typeof fetch
  now?: () => number
  copyToClipboard?: (value: string) => Promise<void>
  scrollToAnchor?: (anchor: string) => void
  khalaAssembly?: ForumKhalaAssemblyDependencies | false
}>

/**
 * Load the live projections for one route. Fail-soft: every fetcher in
 * `-forum-data.ts` resolves to `null` on failure, so an unreachable Worker
 * yields the honest unavailable state and nothing is fabricated client-side.
 */
export const loadForumState = async (
  params: ForumRouteParams,
  fetchFn: typeof fetch,
): Promise<Partial<ForumPageState>> => {
  if (params.kind === 'index') {
    const forums = await fetchForumIndex(fetchFn)
    if (forums === null) {
      return { phase: 'unavailable', errorMessage: 'Board index unavailable' }
    }
    return { phase: 'ready', forums }
  }
  if (params.kind === 'forum') {
    const [forum, topics] = await Promise.all([
      fetchForumSummary(params.forumRef, fetchFn),
      fetchForumTopics(params.forumRef, fetchFn),
    ])
    if (forum === null) {
      return { phase: 'unavailable', errorMessage: 'Forum unavailable' }
    }
    return { phase: 'ready', forum, topics: topics ?? [] }
  }
  if (params.kind === 'topic') {
    const [detail, authMode] = await Promise.all([
      fetchForumTopicDetail(params.topicId, params.sortDirection, fetchFn),
      fetchForumAuthMode(fetchFn),
    ])
    if (detail === null) {
      return { phase: 'unavailable', errorMessage: 'Topic unavailable' }
    }
    return {
      phase: 'ready',
      topic: detail.topic,
      posts: detail.posts,
      authMode,
    }
  }
  const receipt = await fetchForumReceipt(params.receiptRef, fetchFn)
  if (receipt === null) {
    return { phase: 'unavailable', errorMessage: 'Receipt unavailable' }
  }
  return { phase: 'ready', receipt }
}

// ---------------------------------------------------------------------------
// Surface component
// ---------------------------------------------------------------------------

type ForumSurfaceProps = Readonly<{
  route: string
  params: ForumRouteParams
  deps?: ForumSurfaceDependencies
}>

export function ForumSurface({ deps = {}, params, route }: ForumSurfaceProps) {
  const rootRef = useRef<HTMLElement | null>(null)
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const scrolledRef = useRef(false)
  const [state, setState] = useState<ForumPageState>(() =>
    initialForumPageState(params),
  )

  const now = deps.now ?? (() => Date.now())
  const fetchFn = deps.fetchFn ?? fetch
  const copyToClipboard =
    deps.copyToClipboard ??
    (async (value: string) => {
      await navigator.clipboard?.writeText(value)
    })
  const scrollToAnchor =
    deps.scrollToAnchor ??
    ((anchor: string) => {
      const root = rootRef.current
      if (root === null) return
      const target = root.querySelector(
        `[id="${anchor.replace(/["\\]/gu, '\\$&')}"]`,
      )
      if (target instanceof HTMLElement) {
        target.scrollIntoView({ block: 'start' })
      }
    })

  // Load the live projections once per mounted route.
  useEffect(() => {
    let disposed = false
    void loadForumState(params, fetchFn)
      .then((loaded) => {
        if (!disposed) {
          setState((previous) => ({ ...previous, ...loaded }))
        }
      })
      .catch(() => {
        if (!disposed) {
          setState((previous) => ({
            ...previous,
            phase: 'unavailable' as const,
            errorMessage: 'Forum unavailable',
          }))
        }
      })
    return () => {
      disposed = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) {
        clearTimeout(copyTimerRef.current)
      }
    },
    [],
  )

  // Deep-link parity: `#post-<id>` / `#post-<n>` anchors scroll to the
  // rendered post once the data boundary has settled.
  useEffect(() => {
    if (
      state.phase === 'loading' ||
      params.kind !== 'topic' ||
      scrolledRef.current ||
      typeof window === 'undefined'
    ) {
      return
    }
    const rawHash = window.location.hash
    if (!rawHash.startsWith('#post-')) {
      return
    }
    scrolledRef.current = true
    const raw = rawHash.slice(1)
    let anchor = raw
    try {
      anchor = decodeURIComponent(raw)
    } catch {
      anchor = raw
    }
    scrollToAnchor(anchor)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.phase])

  // Motion is attached only after the board content has committed. Semantic
  // content never waits for it.
  useScopedEffect(() => {
    const root = rootRef.current
    const khalaAssembly = deps.khalaAssembly
    if (
      root === null ||
      params.kind !== 'index' ||
      state.phase !== 'ready' ||
      khalaAssembly === false
    ) {
      return Effect.void
    }
    return mountForumBoardAssembly(root, khalaAssembly ?? {})
  }, [params.kind, state.phase])

  const onCopyPermalink = useCallback(
    (postId: string, href: string) => {
      const origin = typeof window === 'undefined' ? '' : window.location.origin
      void copyToClipboard(`${origin}${href}`)
        .catch(() => undefined)
        .then(() => {
          setState((previous) => ({
            ...previous,
            copiedPermalinkPostId: postId,
          }))
          if (copyTimerRef.current !== null) {
            clearTimeout(copyTimerRef.current)
          }
          copyTimerRef.current = setTimeout(() => {
            copyTimerRef.current = null
            setState((previous) =>
              previous.copiedPermalinkPostId === postId
                ? { ...previous, copiedPermalinkPostId: null }
                : previous,
            )
          }, 1500)
        })
    },
    [copyToClipboard],
  )

  return (
    <main
      aria-label="OpenAgents Forum"
      className="forum-page"
      data-route={route}
      ref={rootRef}
    >
      <ForumStyleSheet />
      <ForumPageView
        nowMs={now()}
        onCopyPermalink={onCopyPermalink}
        state={state}
      />
    </main>
  )
}

// ---------------------------------------------------------------------------
// Standalone mount
//
// The Cloud Run monolith serves /forum* from a static shell plus one packed
// browser bundle (src/forum-entry.ts). That host has no router, so it mounts
// this same component tree into `#forum-root` directly.
// ---------------------------------------------------------------------------

const routeNameForParams = (params: ForumRouteParams): string =>
  params.kind === 'index'
    ? 'forum'
    : params.kind === 'forum'
      ? 'forum-forum'
      : params.kind === 'topic'
        ? 'forum-topic'
        : 'forum-receipt'

export const mountForumSurface = (
  container: HTMLElement,
  params: ForumRouteParams,
  deps: ForumSurfaceDependencies = {},
): Readonly<{ unmount: () => void }> => {
  const root = createRoot(container)
  root.render(
    <ForumSurface deps={deps} params={params} route={routeNameForParams(params)} />,
  )
  return {
    unmount: () => {
      root.unmount()
    },
  }
}

// ---------------------------------------------------------------------------
// TanStack Start route components
// ---------------------------------------------------------------------------

export function ForumIndexPage() {
  return <ForumSurface params={{ kind: 'index' }} route="forum" />
}

export function ForumForumPage({ forumRef }: Readonly<{ forumRef: string }>) {
  return <ForumSurface params={{ kind: 'forum', forumRef }} route="forum-forum" />
}

export function ForumTopicPage({ topicId }: Readonly<{ topicId: string }>) {
  return (
    <ForumSurface
      params={{
        kind: 'topic',
        topicId,
        sortDirection: parseTopicPostSortDirection(
          typeof window === 'undefined' ? '' : window.location.search,
        ),
      }}
      route="forum-topic"
    />
  )
}

export function ForumReceiptPage({
  receiptRef,
}: Readonly<{ receiptRef: string }>) {
  return (
    <ForumSurface params={{ kind: 'receipt', receiptRef }} route="forum-receipt" />
  )
}
