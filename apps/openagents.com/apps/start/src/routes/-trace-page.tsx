import type { AtifStep, AtifToolCall, AtifTrajectory } from '@openagentsinc/atif/trace'
import { khalaTheme } from '@effect-native/tokens'
import {
  desktopThemeCssVariables,
  dispatchWorkbenchItem,
  type WorkbenchDispatchItem,
} from '@openagentsinc/ui/desktop-workbench'
import '@openagentsinc/ui/desktop-workbench.css'
import { Copy, Terminal } from 'lucide-react'
import { useEffect, useState } from 'react'

import {
  fetchTraceProjection,
  traceBlobUrl,
  traceReadToken,
  type TraceProjection,
} from './-trace-fetch'
import './-trace.css'

// The public `/trace/{uuid}` ATIF evidence viewer.
//
// Parity work (issue #9061): the timeline used to render every tool call as a
// generic Effect Native Card with `JSON.stringify(arguments)` in a code block —
// no per-tool components, no sub-agent UI. It now projects each ATIF step into
// the SAME `WorkbenchDispatchItem` shape the desktop transcript uses and renders
// it through `@openagentsinc/ui/desktop-workbench`'s `dispatchWorkbenchItem` —
// the exact typed cards (`DesktopCommandCard` for Bash, `DesktopFileChangeCard`
// for Edit/Write, `DesktopAgentGroup` for Task/Agent sub-agents,
// `DesktopReasoningDisclosure`, `DesktopToolCallCard`) OpenAgents Desktop ships.
// This mirrors the `/share/{shareId}` viewer (`-share-timeline.tsx`), which
// already feeds ATIF-adjacent data into the same dispatch.

type TraceVerdict = 'PASS' | 'REFUTED' | 'INCONCLUSIVE' | 'IN_PROGRESS'

const normalizeVerdict = (value: unknown): TraceVerdict | undefined => {
  if (typeof value !== 'string') return undefined
  switch (value.toUpperCase()) {
    case 'PASS':
    case 'PASSED':
    case 'TEST_PASSED':
      return 'PASS'
    case 'REFUTED':
    case 'FAILED':
      return 'REFUTED'
    case 'INCONCLUSIVE':
      return 'INCONCLUSIVE'
    default:
      return undefined
  }
}

export const traceVerdict = (trajectory: AtifTrajectory): TraceVerdict => {
  for (const step of trajectory.steps.toReversed()) {
    for (const call of step.tool_calls ?? []) {
      if (call.function_name.toLowerCase() !== 'done') continue
      const verdict = normalizeVerdict(call.arguments.verdict)
      if (verdict !== undefined) return verdict
    }
    for (const result of step.observation?.results ?? []) {
      if (result.content.includes('test_passed')) return 'PASS'
      if (result.content.includes('verification_class=failed')) return 'REFUTED'
    }
  }
  return 'IN_PROGRESS'
}

const verdictLabel = (verdict: TraceVerdict): string =>
  verdict === 'PASS'
    ? 'Verified'
    : verdict === 'REFUTED'
      ? 'Refuted'
      : verdict === 'INCONCLUSIVE'
        ? 'Inconclusive'
        : 'In progress'

const verdictBadgeClass = (verdict: TraceVerdict): string => {
  const base =
    'inline-flex min-h-6 items-center border px-2 font-mono text-xs uppercase tracking-[0.08em]'
  switch (verdict) {
    case 'PASS':
      return `${base} border-khala-success/60 bg-khala-success/10 text-khala-success`
    case 'REFUTED':
      return `${base} border-khala-danger/60 bg-khala-danger/10 text-khala-danger`
    case 'INCONCLUSIVE':
      return `${base} border-khala-warning/60 bg-khala-warning/10 text-khala-warning`
    default:
      return `${base} border-khala-border bg-khala-surface text-khala-text-muted`
  }
}

const firstUserStep = (trajectory: AtifTrajectory): AtifStep | undefined =>
  trajectory.steps.find(step => step.source === 'user')

const timelineSteps = (trajectory: AtifTrajectory): ReadonlyArray<AtifStep> => {
  const goal = firstUserStep(trajectory)
  return trajectory.steps.filter(step => step !== goal)
}

const formatNumber = (value: number): string =>
  new Intl.NumberFormat('en-US').format(Math.round(value))

const formatCost = (value: number | undefined): string =>
  value === undefined
    ? 'Not measured'
    : value === 0
      ? '$0.00'
      : value < 0.01
        ? `$${value.toFixed(4)}`
        : `$${value.toFixed(2)}`

const traceDurationMs = (trajectory: AtifTrajectory): number | undefined => {
  const timestamps = trajectory.steps
    .map(step => (step.timestamp === undefined ? Number.NaN : Date.parse(step.timestamp)))
    .filter(Number.isFinite)
  if (timestamps.length < 2) return undefined
  return Math.max(...timestamps) - Math.min(...timestamps)
}

const formatDuration = (durationMs: number | undefined): string => {
  if (durationMs === undefined) return 'Not measured'
  if (durationMs < 1_000) return `${Math.round(durationMs)} ms`
  if (durationMs < 60_000) return `${(durationMs / 1_000).toFixed(1)} s`
  const minutes = Math.floor(durationMs / 60_000)
  const seconds = Math.round((durationMs % 60_000) / 1_000)
  return `${minutes}m ${seconds}s`
}

const terminalSummary = (trajectory: AtifTrajectory): string | undefined => {
  for (const step of trajectory.steps.toReversed()) {
    for (const call of step.tool_calls ?? []) {
      if (call.function_name.toLowerCase() !== 'done') continue
      const summary = call.arguments.summary
      if (typeof summary === 'string' && summary.trim() !== '') return summary
    }
  }
  return undefined
}

export const trajectoryToMarkdown = (trajectory: AtifTrajectory): string => {
  const lines = [
    '# Agent session trace',
    '',
    `Agent: ${trajectory.agent.name}`,
    `Model: ${trajectory.agent.model_name ?? 'unknown'}`,
    `Verdict: ${verdictLabel(traceVerdict(trajectory))}`,
    '',
  ]
  const goal = firstUserStep(trajectory)?.message
  if (goal !== undefined) lines.push('## Goal', '', goal, '')
  for (const step of timelineSteps(trajectory)) {
    lines.push(`## Step ${step.step_id}`, '', step.message, '')
    for (const call of step.tool_calls ?? []) {
      lines.push(
        '```json',
        JSON.stringify({ tool: call.function_name, arguments: call.arguments }, null, 2),
        '```',
        '',
      )
    }
    for (const result of step.observation?.results ?? []) {
      lines.push('Observation:', '', ...result.content.split('\n').map(line => `> ${line}`), '')
    }
  }
  return `${lines.join('\n').trim()}\n`
}

// ---------------------------------------------------------------------------
// ATIF step -> WorkbenchDispatchItem projection (the parity core).
// ---------------------------------------------------------------------------

const MAX_SNIPPET = 6_000

const truncate = (value: string, max = MAX_SNIPPET): string =>
  value.length <= max ? value : `${value.slice(0, max)}\n… [${value.length - max} more chars]`

const argValue = (value: unknown): string => {
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

const splitLines = (value: string): ReadonlyArray<string> =>
  value === '' ? [] : value.split('\n')

/** A `+old / -new` unified-diff string the desktop file-change card colorizes. */
const buildEditDiff = (oldStr: string, newStr: string): string =>
  [...splitLines(oldStr).map(line => `-${line}`), ...splitLines(newStr).map(line => `+${line}`)].join(
    '\n',
  )

const observationFor = (step: AtifStep, callId: string): string | undefined => {
  const result = step.observation?.results.find(r => r.source_call_id === callId)
  return result === undefined ? undefined : result.content
}

type KeyedDispatchItem = Readonly<{ key: string; item: WorkbenchDispatchItem }>

/** Project one tool call (+ its observation) onto the matching desktop card. */
const toolCallToDispatchItem = (
  step: AtifStep,
  call: AtifToolCall,
  index: number,
): ReadonlyArray<KeyedDispatchItem> => {
  const key = `step-${step.step_id}-tool-${index}`
  const name = call.function_name
  const lower = name.toLowerCase()
  const args = call.arguments
  const obs = observationFor(step, call.tool_call_id)

  // Bash -> DesktopCommandCard (terminal command + output).
  if (lower === 'bash') {
    return [
      {
        key,
        item: {
          kind: 'command',
          source: 'local',
          command: argValue(args.command ?? ''),
          status: 'completed',
          ...(obs === undefined ? {} : { outputTail: truncate(obs) }),
        },
      },
    ]
  }

  // Edit -> DesktopFileChangeCard (colorized diff).
  if (lower === 'edit') {
    const oldStr = typeof args.old_string === 'string' ? args.old_string : ''
    const newStr = typeof args.new_string === 'string' ? args.new_string : ''
    return [
      {
        key,
        item: {
          kind: 'fileChange',
          source: 'local',
          status: 'completed',
          changes: [
            {
              path: argValue(args.file_path ?? 'file'),
              kind: 'update',
              adds: splitLines(newStr).length,
              dels: splitLines(oldStr).length,
              diff: truncate(buildEditDiff(oldStr, newStr)),
            },
          ],
        },
      },
    ]
  }

  // Write -> DesktopFileChangeCard (new file content as additions).
  if (lower === 'write') {
    const content = typeof args.content === 'string' ? args.content : ''
    return [
      {
        key,
        item: {
          kind: 'fileChange',
          source: 'local',
          status: 'completed',
          changes: [
            {
              path: argValue(args.file_path ?? 'file'),
              kind: 'add',
              adds: splitLines(content).length,
              diff: truncate(splitLines(content).map(line => `+${line}`).join('\n')),
            },
          ],
        },
      },
    ]
  }

  // Task / Agent -> DesktopAgentGroup (sub-agent card) + the returned report.
  if (lower === 'agent' || lower === 'task' || lower === 'spawn_agent') {
    const subagentType =
      typeof args.subagent_type === 'string' && args.subagent_type.trim() !== ''
        ? args.subagent_type
        : 'agent'
    const description = typeof args.description === 'string' ? args.description : ''
    const prompt = typeof args.prompt === 'string' ? args.prompt : description
    const items: Array<KeyedDispatchItem> = [
      {
        key,
        item: {
          kind: 'agent',
          source: 'local',
          status: 'completed',
          ...(prompt === '' ? {} : { prompt: truncate(prompt) }),
          children: [
            { threadRef: call.tool_call_id, status: 'completed', nickname: subagentType },
          ],
          ...(description === '' ? {} : { agentPath: description }),
        },
      },
    ]
    if (obs !== undefined && obs.trim() !== '') {
      items.push({
        key: `${key}-result`,
        item: { kind: 'message', source: 'local', role: 'assistant', text: truncate(obs) },
      })
    }
    return items
  }

  // Everything else -> DesktopToolCallCard (icon + arg table + result snippet).
  const callKind: 'mcp' | 'web' | 'dynamic' = name.startsWith('mcp__')
    ? 'mcp'
    : lower === 'webfetch' || lower === 'websearch'
      ? 'web'
      : 'dynamic'
  const webQuery =
    callKind === 'web'
      ? argValue(args.url ?? args.query ?? '')
      : undefined
  return [
    {
      key,
      item: {
        kind: 'toolCall',
        source: 'local',
        callKind,
        tool: name,
        status: 'completed',
        args: Object.entries(args).map(([k, v]) => ({ key: k, value: truncate(argValue(v), 800) })),
        ...(webQuery === undefined || webQuery === '' ? {} : { query: webQuery }),
        ...(obs === undefined ? {} : { resultSnippet: truncate(obs) }),
      },
    },
  ]
}

/** Project one ATIF step into its ordered dispatch items. */
const stepToDispatchItems = (step: AtifStep): ReadonlyArray<KeyedDispatchItem> => {
  const items: Array<KeyedDispatchItem> = []

  if (step.message.trim() !== '') {
    const role = step.source === 'agent' ? 'assistant' : step.source
    items.push({
      key: `step-${step.step_id}-message`,
      item: { kind: 'message', source: 'local', role, text: step.message },
    })
  }

  if (step.reasoning_content !== undefined && step.reasoning_content.trim() !== '') {
    items.push({
      key: `step-${step.step_id}-reasoning`,
      item: { kind: 'reasoning', source: 'local', summary: step.reasoning_content },
    })
  }

  const boundCallIds = new Set<string>()
  ;(step.tool_calls ?? []).forEach((call, index) => {
    boundCallIds.add(call.tool_call_id)
    items.push(...toolCallToDispatchItem(step, call, index))
  })

  // Observations with no matching tool_call in this step -> a quiet notice.
  ;(step.observation?.results ?? [])
    .filter(result => !boundCallIds.has(result.source_call_id))
    .forEach((result, index) => {
      items.push({
        key: `step-${step.step_id}-obs-${index}`,
        item: { kind: 'notice', source: 'local', severity: 'info', text: truncate(result.content) },
      })
    })

  return items
}

// ---------------------------------------------------------------------------
// React views
// ---------------------------------------------------------------------------

const metadataRows = (
  projection: TraceProjection,
): ReadonlyArray<Readonly<{ label: string; value: string }>> => {
  const trajectory = projection.trajectory
  return [
    { label: 'Agent', value: trajectory.agent.name },
    { label: 'Model', value: trajectory.agent.model_name ?? 'Unknown' },
    { label: 'Duration', value: formatDuration(traceDurationMs(trajectory)) },
    { label: 'Cost', value: formatCost(trajectory.final_metrics?.total_cost_usd) },
    { label: 'Steps', value: formatNumber(projection.stepCount) },
  ]
}

function TraceCopyButton({ trajectory }: Readonly<{ trajectory: AtifTrajectory }>) {
  const [copied, setCopied] = useState(false)
  const handleClick = () => {
    void navigator.clipboard
      ?.writeText(trajectoryToMarkdown(trajectory))
      .then(() => setCopied(true))
      .catch(() => {})
  }
  return (
    <button
      aria-label="Copy trace as Markdown"
      className="khala-focus inline-flex min-h-8 items-center gap-2 border border-khala-border bg-khala-surface px-2.5 font-mono text-xs text-khala-text-muted hover:border-khala-border-strong hover:text-khala-text"
      onClick={handleClick}
      type="button"
    >
      <Copy aria-hidden="true" className="size-4 text-khala-text-faint" />
      <span className="max-[640px]:hidden">{copied ? 'Copied' : 'Copy trace'}</span>
    </button>
  )
}

export function TraceLoadedView({
  projection,
  token,
}: Readonly<{ projection: TraceProjection; token?: string }>) {
  const trajectory = projection.trajectory
  const verdict = traceVerdict(trajectory)
  const goal = firstUserStep(trajectory)
  const images = projection.blobRefs.filter(
    ref => ref.kind === 'screenshot' || ref.kind === 'image',
  )
  const video = projection.blobRefs.find(ref => ref.kind === 'video')
  const timelineItems = timelineSteps(trajectory).flatMap(stepToDispatchItems)

  return (
    <main
      className="oa-react-workbench min-h-dvh w-full overflow-auto bg-khala-void font-mono text-khala-text"
      data-component="trace-page"
      data-route="trace"
      style={desktopThemeCssVariables(khalaTheme)}
    >
      <header
        className="flex h-12 flex-none items-center justify-between gap-3 border-b border-khala-border bg-khala-surface px-4"
        data-component="trace-header"
      >
        <div className="flex min-w-0 items-center gap-3">
          <a
            aria-label="OpenAgents"
            className="khala-focus inline-flex size-6 shrink-0 items-center justify-center border border-khala-border bg-khala-surface-raised text-khala-text no-underline hover:border-khala-border-strong"
            href="/"
          >
            <Terminal aria-hidden="true" className="size-4 text-khala-text" />
          </a>
          <span className="text-xs font-semibold uppercase tracking-[0.12em] text-khala-text-muted">
            Trace
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className="inline-flex min-h-8 items-center border border-khala-border px-2.5 text-xs text-khala-text-muted">
            {projection.visibility.replace('_', ' ')}
          </span>
          <TraceCopyButton trajectory={trajectory} />
        </div>
      </header>

      <div className="mx-auto grid w-full max-w-[980px] gap-6 px-6 py-6 max-[760px]:px-3">
        <div className="grid gap-3">
          <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
            Agent session trace
          </p>
          <h1 className="m-0 min-w-0 break-words text-lg font-medium text-khala-text">
            {terminalSummary(trajectory) ?? trajectory.trajectory_id}
          </h1>
          <p className="m-0 font-mono text-xs text-khala-text-faint">{projection.uuid}</p>
          <span className={verdictBadgeClass(verdict)}>{verdictLabel(verdict)}</span>
        </div>

        <div className="grid grid-cols-2 gap-px border border-khala-border bg-khala-border sm:grid-cols-5">
          {metadataRows(projection).map(row => (
            <div className="grid gap-1 bg-khala-surface px-3 py-2.5" key={row.label}>
              <span className="font-mono text-[0.625rem] uppercase tracking-[0.1em] text-khala-text-faint">
                {row.label}
              </span>
              <span className="break-words text-sm text-khala-text">{row.value}</span>
            </div>
          ))}
        </div>

        <p className="m-0 font-mono text-xs text-khala-text-faint">
          Evidence only. This trace grants no accepted-work, payout, or public-claim authority.
        </p>

        {goal === undefined ? null : (
          <div className="grid gap-2">
            <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
              Goal
            </p>
            <div
              className="border border-khala-border bg-khala-surface p-4 text-sm/6 text-khala-text"
              data-component="trace-goal"
            >
              <p className="m-0 whitespace-pre-wrap break-words">{goal.message}</p>
            </div>
          </div>
        )}

        <div className="grid gap-2">
          <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
            Timeline
          </p>
          <div className="flex min-w-0 flex-col gap-2.5" data-component="trace-timeline">
            {timelineItems.map(({ key, item }) => (
              <div className="min-w-0" key={key}>
                {dispatchWorkbenchItem(item, { itemKey: key })}
              </div>
            ))}
          </div>
        </div>

        {images.length === 0 ? null : (
          <div className="grid gap-3" data-component="trace-screenshots">
            <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
              Screenshots
            </p>
            {images.map((image, index) => (
              <figure className="m-0 grid gap-2" key={index}>
                <img
                  alt={image.caption ?? 'Trace screenshot'}
                  className="w-full border border-khala-border bg-black object-contain"
                  src={traceBlobUrl(projection.uuid, image.r2Key, token)}
                />
                <figcaption className="m-0 font-mono text-xs text-khala-text-faint">
                  {image.caption ?? image.r2Key}
                </figcaption>
              </figure>
            ))}
          </div>
        )}

        {video === undefined ? null : (
          <section className="grid gap-3" data-component="trace-recording">
            <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
              Session recording
            </p>
            <video
              className="aspect-video w-full border border-khala-border bg-black object-contain"
              controls
              playsInline
              preload="metadata"
              src={traceBlobUrl(projection.uuid, video.r2Key, token)}
            />
            <p className="m-0 font-mono text-xs text-khala-text-faint">
              {video.caption ?? video.r2Key}
            </p>
          </section>
        )}
      </div>
    </main>
  )
}

function TraceStatusView({
  status,
  loading,
}: Readonly<{ status?: number; loading: boolean }>) {
  const notFound = status === 404 || status === 403
  return (
    <main
      aria-busy={loading ? 'true' : undefined}
      className="grid min-h-dvh place-items-center bg-khala-void px-4 py-12 font-mono text-khala-text"
      data-component={loading ? 'trace-skeleton' : notFound ? 'trace-not-found' : 'trace-error'}
      data-route="trace"
    >
      <div className="grid max-w-[min(100%,32rem)] justify-items-start gap-3 border border-khala-border bg-khala-surface p-6">
        <p className="m-0 font-mono text-[0.6875rem] uppercase tracking-[0.16em] text-khala-text-faint">
          Trace
        </p>
        <h1 className="m-0 text-lg font-medium text-khala-text">
          {loading ? 'Loading trace' : notFound ? 'No trace at this link' : 'Trace unavailable'}
        </h1>
        <p className="m-0 text-sm/6 text-khala-text-muted">
          {loading
            ? 'Reading the stored ATIF evidence.'
            : notFound
              ? 'This trace does not exist, is private, or is no longer available.'
              : 'The trace could not be loaded. Try again shortly.'}
        </p>
        {loading ? null : (
          <a
            className="khala-focus inline-flex min-h-10 w-fit items-center border border-khala-text bg-khala-text px-4 font-mono text-[0.8125rem] text-black hover:bg-white"
            href="/"
          >
            Go home
          </a>
        )}
      </div>
    </main>
  )
}

export function TraceFailedView({ status }: Readonly<{ status: number }>) {
  return <TraceStatusView loading={false} status={status} />
}

function TraceLoadingView() {
  return <TraceStatusView loading />
}

type TraceLoadState =
  | Readonly<{ tag: 'loading' }>
  | Readonly<{ tag: 'loaded'; projection: TraceProjection }>
  | Readonly<{ tag: 'failed'; status: number }>

export function TracePage({ traceUuid }: Readonly<{ traceUuid: string }>) {
  const token = typeof window === 'undefined' ? undefined : traceReadToken(window.location.search)
  const [state, setState] = useState<TraceLoadState>({ tag: 'loading' })

  useEffect(() => {
    let cancelled = false
    setState({ tag: 'loading' })
    void fetchTraceProjection(traceUuid, token).then(result => {
      if (cancelled) return
      setState(
        result.tag === 'loaded'
          ? { tag: 'loaded', projection: result.projection }
          : { tag: 'failed', status: result.status },
      )
    })
    return () => {
      cancelled = true
    }
  }, [token, traceUuid])

  if (state.tag === 'loaded')
    return (
      <TraceLoadedView projection={state.projection} {...(token === undefined ? {} : { token })} />
    )
  if (state.tag === 'failed') return <TraceFailedView status={state.status} />
  return <TraceLoadingView />
}
