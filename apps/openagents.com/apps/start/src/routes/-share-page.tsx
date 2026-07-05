import type {
  ShareProjectionV1,
  WorkroomFileItem,
} from '@openagentsinc/sync-schema'
import { Copy, ExternalLink, Terminal } from 'lucide-react'
import type * as React from 'react'
import { useEffect, useState } from 'react'

import { fetchShareProjection, userFacingCopy } from './-share-fetch'
import { ShareTimelineMessage, shareMessagePreview } from './-share-timeline'

// `openagents.com/share/{shareId}` — the shared workroom-timeline viewer.
// Ported from `apps/web/src/page/loggedOut/page/share.ts`. This was the last
// fully-unmigrated standalone `loggedOut` public page in the TS-6 sweep
// (see `docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md`);
// every prior slice flagged it as needing the workroom timeline/file-panel
// component set ported to React first — that set (`-share-timeline.tsx`,
// plus the local components below) is the actual new work in this batch.
//
// Live-fetch note, same exception as `/pylons`: a share link's entire
// purpose is showing one specific shared conversation, so honestly
// rendering only the pre-fetch idle placeholder (the posture used for
// `/mirrorcode`, `/promises`, `/stats`, `/training/runs`) would not be a
// faithful port of what this page is for. This route polls the real,
// existing `GET /api/share/{shareId}/v1/data` endpoint once on mount — no
// auth beyond the browser's own session cookie (`credentials: 'include'`,
// same as the legacy page, so team/user-audience shares still resolve for a
// signed-in visitor), no spend, no mutation. Fail-soft is preserved exactly
// like the legacy page: any fetch/parse error renders the same honest
// "Share not found" fallback the legacy `failedBody` shows for an
// unrecognized status, never fabricated transcript content.

const shareLoginHref = (shareId: string): string =>
  `/login/github?returnTo=${encodeURIComponent(`/share/${shareId}`)}`

function shareStatusLabel(projection: ShareProjectionV1): string {
  return projection.status === 'active'
    ? 'Active'
    : projection.status === 'expired'
      ? 'Expired'
      : 'Revoked'
}

function sourceKindLabel(projection: ShareProjectionV1): string {
  return projection.source.kind === 'agent-run'
    ? 'Agent run'
    : projection.source.kind === 'team-thread'
      ? 'Team thread'
      : 'Project thread'
}

function sourceHref(projection: ShareProjectionV1): string | undefined {
  const source = projection.source
  return source.kind === 'agent-run'
    ? `/t/${source.id}`
    : source.kind === 'team-thread'
      ? `/teams/${source.teamId ?? source.id}/chat`
      : `/teams/${source.teamId}/projects/${source.projectId ?? source.id}/chat`
}

function sourceLabel(projection: ShareProjectionV1): string {
  return projection.source.kind === 'agent-run'
    ? 'Open source run'
    : 'Open source thread'
}

function formatShareTimestamp(value: string): string {
  const [date, timeWithZone] = value.trim().split('T')
  const time = timeWithZone?.slice(0, 5)

  if (date === undefined || date === '' || time === undefined || time === '') {
    return value
  }

  return `${date} ${time} UTC`
}

function reviewItemCountLabel(count: number): string {
  return count === 1 ? '1 item' : `${count} items`
}

function fileRows(
  projection: ShareProjectionV1,
): ReadonlyArray<WorkroomFileItem> {
  return [
    ...projection.files,
    ...projection.artifacts.map(artifact => ({
      label: artifact,
      meta: 'artifact',
      depth: 1 as const,
    })),
    ...projection.approvals.map(approval => ({
      label: approval,
      meta: 'approval',
      depth: 1 as const,
    })),
    ...projection.receipts.map(receipt => ({
      label: receipt,
      meta: 'receipt',
      depth: 1 as const,
    })),
  ]
}

function metadataRows(
  projection: ShareProjectionV1,
): ReadonlyArray<Readonly<{ label: string; value: string }>> {
  return [
    { label: 'Status', value: shareStatusLabel(projection) },
    { label: 'Source', value: sourceKindLabel(projection) },
    { label: 'Created', value: formatShareTimestamp(projection.createdAt) },
    { label: 'Events', value: String(projection.metrics.eventCount) },
    { label: 'Tools', value: String(projection.metrics.toolCallCount) },
    { label: 'Tokens', value: String(projection.metrics.tokenTotal) },
  ]
}

function ShareEmptyState({
  title,
  body,
  action,
}: Readonly<{ title: string; body?: string; action?: React.ReactNode }>) {
  return (
    <div className="grid max-w-[min(100%,32rem)] justify-items-start gap-3 border border-khala-border bg-khala-surface p-6">
      <p className="m-0 font-mono text-[0.6875rem] uppercase text-khala-text-faint">
        Empty
      </p>
      <h3 className="m-0 text-lg font-medium text-khala-text">{title}</h3>
      {body === undefined ? null : (
        <p className="m-0 text-sm/6 text-khala-text-muted">{body}</p>
      )}
      {action}
    </div>
  )
}

function ShareLinkButton({
  href,
  label,
}: Readonly<{ href: string; label: string }>) {
  return (
    <a
      className="khala-focus inline-flex min-h-10 w-fit items-center border border-khala-text bg-khala-text px-4 font-mono text-[0.8125rem] text-black hover:bg-white"
      href={href}
    >
      {label}
    </a>
  )
}

function ShareTopBarLink({
  href,
  icon,
  label,
}: Readonly<{
  href: string
  icon: 'Copy' | 'ExternalLink'
  label: string
}>) {
  const Icon = icon === 'Copy' ? Copy : ExternalLink

  return (
    <a
      aria-label={label}
      className="khala-focus inline-flex min-h-8 items-center gap-2 border border-khala-border bg-khala-surface px-2.5 text-xs text-khala-text-muted no-underline hover:border-khala-border-strong hover:text-khala-text"
      href={href}
    >
      <Icon aria-hidden="true" className="size-4 text-khala-text-faint" />
      <span className="max-[640px]:hidden">{label}</span>
    </a>
  )
}

function ShareCopyLinkButton({ url }: Readonly<{ url: string }>) {
  const handleClick = () => {
    void navigator.clipboard?.writeText(url).catch(() => {})
  }

  return (
    <button
      aria-label="Copy share link"
      className="khala-focus inline-flex min-h-8 items-center gap-2 border border-khala-border bg-khala-surface px-2.5 text-xs text-khala-text-muted hover:border-khala-border-strong hover:text-khala-text"
      onClick={handleClick}
      type="button"
    >
      <Copy aria-hidden="true" className="size-4 text-khala-text-faint" />
      <span className="max-[640px]:hidden">Copy link</span>
    </button>
  )
}

function ShareHeader({
  projection,
  reviewItems,
}: Readonly<{
  projection: ShareProjectionV1
  reviewItems: ReadonlyArray<WorkroomFileItem>
}>) {
  const href = sourceHref(projection)

  return (
    <header
      className="flex h-12 flex-none items-center justify-between gap-3 border-b border-khala-border bg-khala-surface px-4 max-[760px]:px-3"
      data-component="share-header"
    >
      <div className="flex min-w-0 items-center gap-3">
        <a
          aria-label="OpenAgents"
          className="khala-focus inline-flex size-6 shrink-0 items-center justify-center border border-khala-border bg-khala-surface-raised text-khala-text no-underline hover:border-khala-border-strong"
          href="/"
        >
          <Terminal aria-hidden="true" className="size-4 text-khala-text" />
        </a>
        <div
          className="min-w-0 truncate text-xs font-semibold text-khala-warning"
          data-share-audience-label=""
        >
          {userFacingCopy(projection.audienceLabel)}
        </div>
        <div className="hidden min-w-0 truncate text-xs text-khala-text-faint md:block">
          {userFacingCopy(projection.title)}
        </div>
      </div>
      <div className="flex min-w-0 shrink-0 items-center gap-2 max-[760px]:gap-1.5">
        <span className="hidden min-h-8 items-center border border-khala-border px-2.5 text-xs text-khala-text-muted sm:inline-flex">
          {shareStatusLabel(projection)}
        </span>
        {reviewItems.length === 0 ? null : (
          <span className="hidden min-h-8 items-center border border-khala-border px-2.5 text-xs text-khala-text-muted lg:inline-flex">
            {reviewItemCountLabel(reviewItems.length)}
          </span>
        )}
        <ShareCopyLinkButton url={projection.url} />
        {href === undefined ? null : (
          <ShareTopBarLink
            href={href}
            icon="ExternalLink"
            label={sourceLabel(projection)}
          />
        )}
      </div>
    </header>
  )
}

function ShareTitleBadge({
  label,
  tone,
}: Readonly<{ label: string; tone: 'accent' | 'neutral' }>) {
  return (
    <span
      className={
        tone === 'accent'
          ? 'inline-flex min-h-6 items-center border border-khala-warning/70 bg-khala-warning/10 px-2 text-xs text-khala-warning'
          : 'inline-flex min-h-6 items-center border border-khala-border bg-khala-surface px-2 text-xs text-khala-text-muted'
      }
    >
      {label}
    </span>
  )
}

function ShareSessionTitleBlock({
  projection,
  reviewItems,
}: Readonly<{
  projection: ShareProjectionV1
  reviewItems: ReadonlyArray<WorkroomFileItem>
}>) {
  return (
    <div
      className="grid w-full max-w-[980px] gap-4 px-6 py-6 max-[760px]:px-3 max-[760px]:py-5"
      data-component="share-session-title"
    >
      <div className="flex min-w-0 flex-wrap items-center gap-2 text-xs text-khala-text-faint">
        <ShareTitleBadge
          label={userFacingCopy(projection.audienceLabel)}
          tone="accent"
        />
        <ShareTitleBadge label={sourceKindLabel(projection)} tone="neutral" />
        <ShareTitleBadge
          label={shareStatusLabel(projection)}
          tone="neutral"
        />
        <span className="min-w-0 break-words max-[760px]:basis-full">
          {formatShareTimestamp(projection.createdAt)}
        </span>
      </div>
      <div className="grid gap-2">
        <h1 className="m-0 min-w-0 text-base font-medium text-khala-text">
          {userFacingCopy(projection.title)}
        </h1>
        <p className="m-0 max-w-[74ch] text-base text-khala-text-faint sm:text-[0.8125rem]">
          {userFacingCopy(projection.subtitle)}
        </p>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1 text-xs text-khala-text-faint">
        <span>{projection.metrics.eventCount} events</span>
        <span>{projection.metrics.toolCallCount} tools</span>
        <span>{projection.metrics.tokenTotal} tokens</span>
        {reviewItems.length === 0 ? null : (
          <span>{reviewItemCountLabel(reviewItems.length)} in review</span>
        )}
      </div>
    </div>
  )
}

function ShareMessageNav({
  messages,
}: Readonly<{ messages: ShareProjectionV1['messages'] }>) {
  const userMessages = messages.filter(message => message.author === 'user')

  if (userMessages.length <= 1) return null

  return (
    <nav
      aria-label="Message navigation"
      className="sticky top-4 hidden w-44 shrink-0 self-start px-2 py-1 lg:block"
    >
      <ul className="m-0 grid list-none gap-1 p-0" role="list">
        {userMessages.map((message, index) => (
          <li className="min-w-0" key={message.id}>
            <a
              className="grid min-h-8 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)] items-center gap-2 border border-transparent px-2 text-xs text-khala-text-faint no-underline hover:border-khala-border hover:bg-khala-surface hover:text-khala-text"
              href={`#message-${message.id}`}
            >
              <span className="tabular-nums">
                {String(index + 1).padStart(2, '0')}
              </span>
              <span className="truncate">
                {shareMessagePreview(message)}
              </span>
            </a>
          </li>
        ))}
      </ul>
    </nav>
  )
}

function ShareMobileReviewPanel({
  projection,
  reviewItems,
}: Readonly<{
  projection: ShareProjectionV1
  reviewItems: ReadonlyArray<WorkroomFileItem>
}>) {
  if (reviewItems.length === 0) return null

  return (
    <details
      className="mx-3 mb-6 border border-khala-border bg-khala-surface lg:hidden"
      data-component="share-mobile-review"
    >
      <summary className="flex min-h-10 cursor-pointer items-center justify-between gap-3 px-3 text-sm text-khala-text [&::-webkit-details-marker]:hidden">
        <span>Review</span>
        <span className="text-xs text-khala-text-faint">
          {reviewItemCountLabel(reviewItems.length)}
        </span>
      </summary>
      <div className="grid border-t border-khala-border">
        {metadataRows(projection).map(row => (
          <div
            className="grid min-h-9 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-khala-border/60 px-3 text-xs"
            key={row.label}
          >
            <span className="text-khala-text-faint">{row.label}</span>
            <div className="text-right text-khala-text">{row.value}</div>
          </div>
        ))}
        <ul className="m-0 grid list-none gap-0.5 p-2" role="list">
          {reviewItems.map((item, index) => (
            <li
              className={
                item.depth === 1
                  ? 'grid min-h-8 min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-transparent px-2 pl-6 text-sm'
                  : 'grid min-h-8 min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-transparent px-2 text-sm'
              }
              key={index}
            >
              <span className="truncate text-khala-text">
                {userFacingCopy(item.label)}
              </span>
              <span className="text-xs text-khala-text-faint">
                {item.meta}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </details>
  )
}

function ShareTimeline({
  projection,
  reviewItems,
}: Readonly<{
  projection: ShareProjectionV1
  reviewItems: ReadonlyArray<WorkroomFileItem>
}>) {
  const isActive = projection.messages.some(
    message => message.status === 'streaming',
  )

  return (
    <section
      aria-label="Shared conversation"
      className="relative h-full min-h-0 min-w-0 flex-1 overflow-hidden bg-khala-void"
      data-component="share-session"
    >
      {isActive ? (
        <div
          aria-hidden="true"
          className="absolute inset-x-0 top-0 z-[2] h-px overflow-hidden bg-khala-warning/70"
        />
      ) : null}
      <div className="absolute inset-0 overflow-auto overscroll-contain pb-10">
        <ShareSessionTitleBlock
          projection={projection}
          reviewItems={reviewItems}
        />
        <ShareMobileReviewPanel
          projection={projection}
          reviewItems={reviewItems}
        />
        <div className="flex min-w-0 items-start gap-2 px-[clamp(12px,4vw,56px)] max-[760px]:px-3">
          <ShareMessageNav messages={projection.messages} />
          <div className="flex min-w-0 flex-1 flex-col gap-[26px]">
            {projection.messages.length === 0 ? (
              <ShareEmptyState
                body="This share does not include transcript messages."
                title="No messages"
              />
            ) : (
              <>
                {projection.messages.map(message => (
                  <ShareTimelineMessage key={message.id} message={message} />
                ))}
                <div data-share-timeline-end="true" />
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  )
}

function ShareSidePanel({
  projection,
  reviewItems,
}: Readonly<{
  projection: ShareProjectionV1
  reviewItems: ReadonlyArray<WorkroomFileItem>
}>) {
  return (
    <aside
      aria-label="Files and review"
      className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l border-khala-border bg-khala-surface max-[1100px]:hidden"
    >
      <div className="grid gap-0 border-b border-khala-border px-3 py-2.5">
        {metadataRows(projection).map(row => (
          <div
            className="flex items-center justify-between gap-3 py-[7px] text-xs"
            key={row.label}
          >
            <span className="text-khala-text-faint">{row.label}</span>
            <span className="max-w-[55%] break-words text-right text-[0.8125rem] text-khala-text">
              {row.value}
            </span>
          </div>
        ))}
      </div>
      <div className="grid min-h-0 flex-1 content-start gap-0.5 overflow-auto overscroll-contain p-2.5">
        {reviewItems.map((file, index) => (
          <div
            className={
              file.depth === 1
                ? 'flex items-center justify-between gap-3 border border-transparent p-2 pl-6'
                : 'flex items-center justify-between gap-3 border border-transparent p-2'
            }
            key={index}
          >
            <span className="truncate text-[0.8125rem] text-khala-text">
              {userFacingCopy(file.label)}
            </span>
            <span className="shrink-0 truncate text-xs text-khala-text-faint">
              {file.meta}
            </span>
          </div>
        ))}
      </div>
      <div className="grid flex-none gap-2.5 border-t border-khala-border p-2.5">
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs text-khala-text-faint">Share</span>
          <span
            aria-label={userFacingCopy(projection.audienceLabel)}
            className="inline-flex min-h-8 items-center border border-khala-border bg-khala-surface-raised px-2.5 text-xs text-khala-text-muted"
          >
            {userFacingCopy(projection.audienceLabel)}
          </span>
        </div>
      </div>
    </aside>
  )
}

export function ShareLoadedView({
  projection,
}: Readonly<{ projection: ShareProjectionV1 }>) {
  const reviewItems = fileRows(projection)

  return (
    <div
      className="isolate flex h-dvh min-h-[720px] w-full min-w-0 flex-col overflow-hidden bg-khala-void font-mono text-khala-text antialiased"
      data-component="share-page"
      data-route="share"
    >
      <ShareHeader projection={projection} reviewItems={reviewItems} />
      <div
        className={
          reviewItems.length === 0
            ? 'grid h-full min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)] overflow-hidden select-text'
            : 'grid h-full min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_420px] grid-rows-[minmax(0,1fr)] overflow-hidden select-text max-[1200px]:grid-cols-[minmax(0,1fr)_380px] max-[1100px]:grid-cols-[minmax(0,1fr)]'
        }
      >
        <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col">
          <ShareTimeline projection={projection} reviewItems={reviewItems} />
        </div>
        {reviewItems.length === 0 ? null : (
          <ShareSidePanel projection={projection} reviewItems={reviewItems} />
        )}
      </div>
    </div>
  )
}

export function ShareFailedView({
  status,
  error,
  shareId,
}: Readonly<{ status: number; error: string; shareId: string }>) {
  const body =
    status === 401 ? (
      <ShareEmptyState
        action={
          <ShareLinkButton href={shareLoginHref(shareId)} label="Sign in" />
        }
        body="This share is restricted to specific OpenAgents members."
        title="Sign in to view this share"
      />
    ) : status === 403 ? (
      <ShareEmptyState
        action={<ShareLinkButton href="/" label="Go Home" />}
        body="This share is not available to your account."
        title="Share unavailable"
      />
    ) : status === 410 ? (
      <ShareEmptyState
        action={<ShareLinkButton href="/" label="Go Home" />}
        body="This shared projection is no longer available."
        title={error === 'share_expired' ? 'Share expired' : 'Share revoked'}
      />
    ) : (
      <ShareEmptyState
        action={<ShareLinkButton href="/" label="Go Home" />}
        body="This share does not exist or is no longer available."
        title="Share not found"
      />
    )

  return (
    <div
      className="grid min-h-[calc(100dvh-96px)] place-items-center bg-khala-void px-4 py-12 font-mono text-khala-text"
      data-component="share-page"
      data-route="share"
    >
      {body}
    </div>
  )
}

function ShareLoadingView() {
  return (
    <div
      className="grid min-h-[calc(100dvh-96px)] place-items-center bg-khala-void px-4 py-12 font-mono text-khala-text"
      data-component="share-page"
      data-route="share"
    >
      <ShareEmptyState
        body="Preparing the shared workroom."
        title="Loading share"
      />
    </div>
  )
}

type ShareLoadState =
  | Readonly<{ tag: 'loading' }>
  | Readonly<{ tag: 'loaded'; projection: ShareProjectionV1 }>
  | Readonly<{ tag: 'failed'; status: number; error: string }>

export function SharePage({ shareId }: Readonly<{ shareId: string }>) {
  const [state, setState] = useState<ShareLoadState>({ tag: 'loading' })

  useEffect(() => {
    let cancelled = false

    void fetchShareProjection(shareId).then(result => {
      if (cancelled) return
      setState(
        result.tag === 'loaded'
          ? { tag: 'loaded', projection: result.projection }
          : { tag: 'failed', status: result.status, error: result.error },
      )
    })

    return () => {
      cancelled = true
    }
  }, [shareId])

  if (state.tag === 'loaded') {
    return <ShareLoadedView projection={state.projection} />
  }

  if (state.tag === 'failed') {
    return (
      <ShareFailedView
        error={state.error}
        shareId={shareId}
        status={state.status}
      />
    )
  }

  return <ShareLoadingView />
}
