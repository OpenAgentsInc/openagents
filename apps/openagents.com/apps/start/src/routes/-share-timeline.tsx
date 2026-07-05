import type {
  WorkroomTimelineMessage,
  WorkroomTimelinePart,
} from '@openagentsinc/sync-schema'

import { userFacingCopy } from './-share-fetch'

// Shared workroom-timeline rendering for the `/share/{shareId}` route.
// Ported from `@openagentsinc/ui`'s Foldkit-`Html` `workroomTimelinePart` /
// `workroomTimelineMessage` (`packages/ui/src/workroom.ts`) and from
// `apps/web/src/page/loggedOut/page/share.ts`'s own inline
// `shareUserTimelineMessage`. Neither of those exists as a React component
// yet — this is the first port of that component set. Visual styling is
// re-derived in Tailwind against the `khala-*` design tokens (same
// hex -> token mapping already established for Terms/Privacy: `#ffb400` ->
// `khala-warning`, `#00c853` -> `khala-success`, `#d32f2f` -> `khala-danger`,
// `#2979ff`-ish info blue -> `khala-energy`), rather than reusing the old
// `oa-ui-workroom-*` CSS-in-JS classes tied to the retired Foldkit stylesheet.
//
// Simplification (decorative, not a content change, same posture as the
// digit-roll-animation skip on `/pylons`): the legacy tool-call "collapsible"
// never actually toggles within this render tree (no client JS wires a
// click handler in the ported source), so its detail is always shown when
// present rather than reproducing an inert expand/collapse affordance.

function toolStatusDotClass(status: WorkroomToolPart['status']): string {
  if (status === 'running') return 'border-khala-warning bg-khala-warning'
  if (status === 'completed') return 'border-khala-success bg-khala-success'
  if (status === 'failed') return 'border-khala-danger bg-khala-danger'
  return 'border-khala-energy bg-khala-energy'
}

type WorkroomToolPart = Extract<WorkroomTimelinePart, { kind: 'tool' }>
type WorkroomDiffPart = Extract<WorkroomTimelinePart, { kind: 'diff' }>
type WorkroomFilePart = Extract<WorkroomTimelinePart, { kind: 'file' }>
type WorkroomTextPart = Extract<WorkroomTimelinePart, { kind: 'text' }>

function TextTimelinePart({ part }: Readonly<{ part: WorkroomTextPart }>) {
  return (
    <div
      className="whitespace-pre-wrap break-words text-sm/6 text-khala-text"
      data-component="text-part"
    >
      {part.body.map((line, index) => (
        <p className="m-0" key={index}>
          {userFacingCopy(line)}
        </p>
      ))}
    </div>
  )
}

function ToolTimelinePart({ part }: Readonly<{ part: WorkroomToolPart }>) {
  const isShell = part.subtitle.toLowerCase().includes('shell')
  const wrapperClass =
    'block border border-khala-border bg-khala-surface p-3'
  const content = (
    <>
      <div className="flex items-center gap-2.5">
        <span
          aria-hidden="true"
          className={`size-2 flex-none border ${toolStatusDotClass(part.status)}`}
        />
        <div className="grid min-w-0 gap-0.5">
          <span className="min-w-0 truncate text-sm font-medium text-khala-text">
            {userFacingCopy(part.title)}
          </span>
          <span className="min-w-0 truncate text-xs text-khala-text-faint">
            {userFacingCopy(part.subtitle)}
          </span>
        </div>
      </div>
      {part.detail.length === 0 ? null : (
        <div
          className="mt-2.5 max-h-56 overflow-auto border border-khala-border bg-khala-void p-2.5"
          data-component={isShell ? 'bash-output' : 'tool-output'}
        >
          <pre className="m-0 whitespace-pre-wrap break-words font-mono text-xs leading-5 text-khala-text-muted">
            <code>
              {part.detail.map(userFacingCopy).join('\n')}
            </code>
          </pre>
        </div>
      )}
      {part.actionHref === undefined ? null : (
        <div className="mt-3 flex justify-start">
          <a
            className="khala-focus inline-flex min-h-8 items-center border border-khala-border bg-khala-surface px-2.5 text-xs text-khala-text-muted hover:border-khala-border-strong hover:text-khala-text"
            href={part.actionHref}
          >
            {part.actionLabel === undefined
              ? 'Open'
              : userFacingCopy(part.actionLabel)}
          </a>
        </div>
      )}
    </>
  )

  return part.href === undefined ? (
    <div className={wrapperClass} data-component="tool-part-wrapper">
      {content}
    </div>
  ) : (
    <a
      aria-label={`${part.title}: open full thread`}
      className={`${wrapperClass} no-underline hover:border-khala-border-strong`}
      data-component="tool-part-wrapper"
      href={part.href}
    >
      {content}
    </a>
  )
}

function DiffTimelinePart({ part }: Readonly<{ part: WorkroomDiffPart }>) {
  return (
    <div
      className="border border-khala-border bg-khala-surface"
      data-component="session-turn-diffs-group"
    >
      <div className="flex items-center justify-between border-b border-khala-border px-3 py-2">
        <span className="text-xs text-khala-text-faint">
          {part.files.length === 1
            ? '1 changed file'
            : `${part.files.length} changed files`}
        </span>
      </div>
      <div className="grid gap-0.5 p-2">
        {part.files.map(file => (
          <div
            className="flex items-center justify-between gap-3 border border-transparent px-2 py-1.5 text-sm"
            key={file.path}
          >
            <span className="min-w-0 truncate text-khala-text">
              {file.path}
            </span>
            <span
              className={
                file.status === 'added'
                  ? 'shrink-0 text-xs tabular-nums text-khala-success'
                  : 'shrink-0 text-xs tabular-nums text-khala-text-muted'
              }
            >
              +{file.added}
              {file.removed > 0 ? ` -${file.removed}` : ''}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function FileTimelinePart({ part }: Readonly<{ part: WorkroomFilePart }>) {
  return (
    <div
      className="border border-khala-border bg-khala-surface"
      data-component="write-tool"
    >
      <div className="flex items-center justify-between gap-3 border-b border-khala-border px-3 py-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="text-xs text-khala-text-faint">File</span>
          <span className="min-w-0 truncate font-mono text-sm text-khala-text">
            {part.path}
          </span>
        </div>
        <span className="shrink-0 text-xs text-khala-text-faint">
          {part.language}
        </span>
      </div>
      <div className="max-h-56 overflow-auto p-2.5">
        <pre className="m-0 whitespace-pre-wrap break-words font-mono text-xs leading-5 text-khala-text-muted">
          <code>{part.excerpt.map(userFacingCopy).join('\n')}</code>
        </pre>
      </div>
    </div>
  )
}

export function ShareTimelinePart({
  part,
}: Readonly<{ part: WorkroomTimelinePart }>) {
  if (part.kind === 'text') return <TextTimelinePart part={part} />
  if (part.kind === 'tool') return <ToolTimelinePart part={part} />
  if (part.kind === 'diff') return <DiffTimelinePart part={part} />
  return <FileTimelinePart part={part} />
}

export function AgentInitialsIcon({ label }: Readonly<{ label: string }>) {
  const initials = label.trim().slice(0, 2).toUpperCase()

  return (
    <span className="inline-flex size-8 shrink-0 items-center justify-center border border-khala-border bg-khala-surface-raised font-mono text-xs font-bold leading-none text-khala-text">
      {initials}
    </span>
  )
}

function messageTextBody(message: WorkroomTimelineMessage): string {
  return message.parts
    .flatMap(part => (part.kind === 'text' ? part.body : []))
    .join('\n')
}

export function ShareUserMessage({
  message,
}: Readonly<{ message: WorkroomTimelineMessage }>) {
  const body = messageTextBody(message)
  const nonTextParts = message.parts.filter(part => part.kind !== 'text')

  return (
    <article
      className="w-full min-w-0 max-w-[920px]"
      data-author={message.author}
      data-timeline-row="UserMessage"
      id={`message-${message.id}`}
    >
      <div
        className="grid min-w-0 gap-3 px-4 md:px-5"
        data-component="share-user-message"
      >
        <div className="flex min-w-0 max-w-full items-start gap-3 text-left">
          {message.avatarUrl === undefined || message.avatarUrl === '' ? (
            <AgentInitialsIcon label={userFacingCopy(message.label)} />
          ) : (
            <img
              alt=""
              className="size-8 shrink-0 border border-khala-border object-cover"
              src={message.avatarUrl}
            />
          )}
          <div className="grid min-w-0 flex-1 gap-1">
            <h3 className="m-0 font-mono text-sm font-bold text-khala-text">
              {userFacingCopy(message.label)}
            </h3>
            {body.trim() === '' ? null : (
              <p className="m-0 min-w-0 max-w-full whitespace-pre-wrap break-words text-sm leading-6 text-khala-text-muted">
                {userFacingCopy(body)}
              </p>
            )}
          </div>
        </div>
        {nonTextParts.map((part, index) => (
          <ShareTimelinePart key={index} part={part} />
        ))}
      </div>
    </article>
  )
}

export function ShareGenericMessage({
  message,
}: Readonly<{ message: WorkroomTimelineMessage }>) {
  const isSystem = message.author === 'system'

  return (
    <article
      className="w-full min-w-0 max-w-full md:mx-auto md:max-w-200 2xl:max-w-[1000px]"
      data-author={message.author}
      data-timeline-row={isSystem ? 'TurnDivider' : 'AssistantPart'}
      id={`message-${message.id}`}
    >
      <div
        className="grid min-w-0 gap-2.5 px-4 md:px-5"
        data-component="session-turn"
      >
        {message.parts.map((part, index) => (
          <ShareTimelinePart key={index} part={part} />
        ))}
        {message.status !== 'streaming' ? null : (
          <span
            aria-hidden="true"
            className="ml-1 inline-block h-[15px] w-[7px] translate-y-0.5 animate-pulse bg-khala-warning"
          />
        )}
      </div>
    </article>
  )
}

export function ShareTimelineMessage({
  message,
}: Readonly<{ message: WorkroomTimelineMessage }>) {
  return message.author === 'user' ? (
    <ShareUserMessage message={message} />
  ) : (
    <ShareGenericMessage message={message} />
  )
}

export function shareMessagePreview(
  message: WorkroomTimelineMessage,
): string {
  const body = message.parts
    .flatMap(part => (part.kind === 'text' ? part.body : []))
    .join('\n')
    .trim()
  const firstLine = body.split('\n')[0]?.trim()

  return userFacingCopy(
    firstLine === undefined || firstLine === '' ? message.label : firstLine,
  )
}
