import type {
  WorkroomTimelineMessage,
  WorkroomTimelinePart,
} from '@openagentsinc/sync-schema'
import {
  dispatchWorkbenchItem,
  type WorkbenchDispatchItem,
  type WorkbenchDispatchStatus,
} from '@openagentsinc/ui/desktop-workbench'
import '@openagentsinc/ui/desktop-workbench.css'
import './-share-workbench.css'

import { userFacingCopy } from './-share-fetch'

// Shared workroom-timeline rendering for the `/share/{shareId}` route.
//
// T14 (#8871, epic #8857 Wave 3): this used to be a parallel, Foldkit-ported
// card renderer (`ToolTimelinePart`/`DiffTimelinePart`/`FileTimelinePart`)
// over the closed 4-kind `text|tool|diff|file` union, with its own hand-
// rolled Tailwind card CSS. `workers/api/src/share-projections.ts` now emits
// the widened `WorkroomTimelinePart` union (reasoning, command, fileChange,
// toolCall, plan, approval, agent, notice, compaction, meter — mirroring the
// desktop `WorkbenchItem` model) for every kind BUT plain inline text, so
// this module now projects each non-text part into the same
// `WorkbenchDispatchItem` shape `apps/openagents-desktop`'s timeline
// dispatches, and renders it through `@openagentsinc/ui/desktop-workbench`'s
// `dispatchWorkbenchItem` — the EXACT same typed components desktop uses.
// The legacy `tool`/`diff`/`file` kinds keep working (older persisted shares
// still decode and render), projected onto the nearest widened shape.
//
// Message-level chrome (author label/avatar, inline text prose, per-message
// wrapper) stays exactly as before — only the per-part CARD rendering moved
// to the shared components. `href`/`actionHref`/`actionLabel` on the legacy
// `tool` part (used by team-thread "Open run" messages) have no equivalent
// on `WorkbenchDispatchItem`, since the shared workbench cards never open
// another surface; those affordances are preserved as a thin external link
// wrapper around the dispatched card rather than dropped.

type WorkroomTextPart = Extract<WorkroomTimelinePart, { kind: 'text' }>
type WorkroomToolPart = Extract<WorkroomTimelinePart, { kind: 'tool' }>
type WorkroomStatus = WorkroomToolPart['status']

const toDispatchStatus = (status: WorkroomStatus): WorkbenchDispatchStatus =>
  status === 'completed' ? 'completed' : status === 'failed' ? 'failed' : 'in_progress'

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

function workroomPartToDispatchItem(
  part: Exclude<WorkroomTimelinePart, WorkroomTextPart>,
): WorkbenchDispatchItem {
  switch (part.kind) {
    case 'tool':
      return {
        kind: 'toolCall',
        source: 'local',
        callKind: 'dynamic',
        tool: userFacingCopy(part.title),
        args: [],
        status: toDispatchStatus(part.status),
        ...(part.detail.length === 0
          ? {}
          : { resultSnippet: part.detail.map(userFacingCopy).join('\n') }),
      }

    case 'diff':
      return {
        kind: 'fileChange',
        source: 'local',
        status: 'completed',
        changes: part.files.map(file => ({
          path: userFacingCopy(file.path),
          kind: file.status === 'added' ? ('add' as const) : ('update' as const),
          adds: file.added,
          dels: file.removed,
        })),
      }

    case 'file':
      return {
        kind: 'fileChange',
        source: 'local',
        status: 'completed',
        changes: [
          {
            path: userFacingCopy(part.path),
            kind: 'update',
            diff: part.excerpt.map(userFacingCopy).join('\n'),
          },
        ],
      }

    case 'reasoning':
      return { kind: 'reasoning', source: 'local', summary: userFacingCopy(part.summary) }

    case 'command':
      return {
        kind: 'command',
        source: 'local',
        command: userFacingCopy(part.command),
        status: toDispatchStatus(part.status),
        ...(part.cwd === undefined ? {} : { cwd: userFacingCopy(part.cwd) }),
        ...(part.exitCode === undefined ? {} : { exitCode: part.exitCode }),
        ...(part.durationMs === undefined ? {} : { durationMs: part.durationMs }),
        ...(part.outputTail === undefined ? {} : { outputTail: userFacingCopy(part.outputTail) }),
        ...(part.outputCapReached === undefined ? {} : { outputCapReached: part.outputCapReached }),
      }

    case 'fileChange':
      return {
        kind: 'fileChange',
        source: 'local',
        status: toDispatchStatus(part.status),
        changes: part.changes.map(change => ({
          path: userFacingCopy(change.path),
          kind: change.kind,
          ...(change.adds === undefined ? {} : { adds: change.adds }),
          ...(change.dels === undefined ? {} : { dels: change.dels }),
          ...(change.diff === undefined ? {} : { diff: userFacingCopy(change.diff) }),
          ...(change.diffCapReached === undefined ? {} : { diffCapReached: change.diffCapReached }),
        })),
      }

    case 'toolCall':
      return {
        kind: 'toolCall',
        source: 'local',
        callKind: part.callKind,
        tool: userFacingCopy(part.tool),
        status: toDispatchStatus(part.status),
        args: part.args.map(arg => ({ key: arg.key, value: userFacingCopy(arg.value) })),
        ...(part.server === undefined ? {} : { server: userFacingCopy(part.server) }),
        ...(part.namespace === undefined ? {} : { namespace: userFacingCopy(part.namespace) }),
        ...(part.resultSnippet === undefined
          ? {}
          : { resultSnippet: userFacingCopy(part.resultSnippet) }),
        ...(part.errorMessage === undefined
          ? {}
          : { errorMessage: userFacingCopy(part.errorMessage) }),
        ...(part.durationMs === undefined ? {} : { durationMs: part.durationMs }),
        ...(part.query === undefined ? {} : { query: userFacingCopy(part.query) }),
        ...(part.resultCount === undefined ? {} : { resultCount: part.resultCount }),
        ...(part.path === undefined ? {} : { path: userFacingCopy(part.path) }),
      }

    case 'plan':
      return {
        kind: 'plan',
        source: 'local',
        entries: part.entries.map(entry => ({
          step: userFacingCopy(entry.step),
          status: entry.status,
        })),
        ...(part.prose === undefined ? {} : { prose: userFacingCopy(part.prose) }),
      }

    case 'approval':
      return {
        kind: 'approval',
        source: 'local',
        status: part.decision === 'denied' ? 'declined' : 'completed',
        ...(part.decision === undefined ? {} : { decision: part.decision }),
        ...(part.detail === undefined ? {} : { detail: userFacingCopy(part.detail) }),
      }

    case 'agent':
      return {
        kind: 'agent',
        source: 'local',
        status: toDispatchStatus(part.status),
        ...(part.tool === undefined ? {} : { tool: part.tool }),
        ...(part.prompt === undefined ? {} : { prompt: userFacingCopy(part.prompt) }),
        ...(part.children === undefined
          ? {}
          : {
              children: part.children.map(child => ({
                threadRef: child.threadRef,
                status: child.status,
                ...(child.nickname === undefined ? {} : { nickname: userFacingCopy(child.nickname) }),
              })),
            }),
      }

    case 'notice':
      return {
        kind: 'notice',
        source: 'local',
        ...(part.severity === undefined ? {} : { severity: part.severity }),
        text: userFacingCopy(part.text),
      }

    case 'compaction':
      return { kind: 'compaction', source: 'local' }

    case 'meter':
      return {
        kind: 'meter',
        source: 'local',
        ...(part.inputTokens === undefined ? {} : { inputTokens: part.inputTokens }),
        ...(part.cachedInputTokens === undefined
          ? {}
          : { cachedInputTokens: part.cachedInputTokens }),
        ...(part.outputTokens === undefined ? {} : { outputTokens: part.outputTokens }),
        ...(part.reasoningTokens === undefined ? {} : { reasoningTokens: part.reasoningTokens }),
        ...(part.totalTokens === undefined ? {} : { totalTokens: part.totalTokens }),
      }
  }
}

export function ShareTimelinePart({
  part,
  itemKey,
}: Readonly<{ part: WorkroomTimelinePart; itemKey: string }>) {
  if (part.kind === 'text') {
    return <TextTimelinePart part={part} />
  }

  const rendered = dispatchWorkbenchItem(workroomPartToDispatchItem(part), { itemKey })

  if (part.kind === 'tool' && part.href !== undefined) {
    return (
      <a
        aria-label={`${userFacingCopy(part.title)}: open full thread`}
        className="oa-share-tool-link"
        href={part.href}
      >
        {rendered}
      </a>
    )
  }

  if (part.kind === 'tool' && part.actionHref !== undefined) {
    return (
      <div className="oa-share-tool-wrapper">
        {rendered}
        <div className="oa-share-tool-action-row">
          <a className="oa-share-tool-action" href={part.actionHref}>
            {part.actionLabel === undefined ? 'Open' : userFacingCopy(part.actionLabel)}
          </a>
        </div>
      </div>
    )
  }

  return rendered
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
          <ShareTimelinePart itemKey={`${message.id}-part-${index}`} key={index} part={part} />
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
          <ShareTimelinePart itemKey={`${message.id}-part-${index}`} key={index} part={part} />
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
