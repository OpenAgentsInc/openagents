import type { AtifStep, AtifTrajectory } from '@openagentsinc/atif/trace'
import {
  Badge,
  Button,
  Card,
  CodeBlock,
  Divider,
  Image,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  Transcript,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentHandlers,
  type IntentReporter,
  type Tone,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { khalaTheme } from '@effect-native/tokens'
import { Effect, Exit, Schema, Scope, SubscriptionRef } from '@effect-native/core/effect'
import { useEffect, useRef, useState } from 'react'

import {
  fetchTraceProjection,
  traceBlobUrl,
  traceReadToken,
  type TraceProjection,
} from './-trace-fetch'
import './-trace.css'

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
  for (const step of [...trajectory.steps].reverse()) {
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

const verdictTone = (verdict: TraceVerdict): Tone =>
  verdict === 'PASS'
    ? 'success'
    : verdict === 'REFUTED'
      ? 'danger'
      : verdict === 'INCONCLUSIVE'
        ? 'warn'
        : 'neutral'

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
    .map(step => step.timestamp === undefined ? Number.NaN : Date.parse(step.timestamp))
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
  for (const step of [...trajectory.steps].reverse()) {
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
      lines.push('```json', JSON.stringify({
        tool: call.function_name,
        arguments: call.arguments,
      }, null, 2), '```', '')
    }
    for (const result of step.observation?.results ?? []) {
      lines.push('Observation:', '', ...result.content.split('\n').map(line => `> ${line}`), '')
    }
  }
  return `${lines.join('\n').trim()}\n`
}

const TraceCopyRequested = defineIntent('TraceCopyRequested', Schema.Struct({}))
const TraceHomeRequested = defineIntent('TraceHomeRequested', Schema.Struct({}))
const traceIntents = [TraceCopyRequested, TraceHomeRequested] as const

export type TraceSurfaceState =
  | Readonly<{ tag: 'loading' }>
  | Readonly<{ tag: 'failed'; status: number }>
  | Readonly<{
      tag: 'loaded'
      projection: TraceProjection
      token?: string
      origin: string
      copied: boolean
      scrollToKey?: string
    }>

const text = (
  key: string,
  content: string,
  variant: 'caption' | 'label' | 'body' | 'title' | 'heading' = 'body',
  color: 'textPrimary' | 'textMuted' | 'accent' = 'textPrimary',
): View => Text({ key, content, variant, color, style: { width: 'full' } })

const codeBlock = (key: string, content: string): View =>
  CodeBlock({
    key,
    language: 'text',
    lines: content.split('\n').map(line => ({
      tokens: [{ kind: 'plain' as const, text: line }],
    })),
    style: {
      backgroundColor: 'surface',
      borderColor: 'border',
      borderRadius: 'none',
      borderWidth: 1,
      width: 'full',
    },
  })

const toolCallView = (step: AtifStep, index: number): View => {
  const call = step.tool_calls?.[index]
  if (call === undefined) return Stack({ key: `step-${step.step_id}-empty-tool`, direction: 'column' })
  const observation = step.observation?.results.find(result => result.source_call_id === call.tool_call_id)
  return Card(
    {
      key: `step-${step.step_id}-tool-${call.tool_call_id}`,
      padding: '3',
      radius: 'none',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    [
      Stack(
        { key: `step-${step.step_id}-tool-head-${index}`, direction: 'row', gap: '3', justify: 'between' },
        [
          text(`step-${step.step_id}-tool-name-${index}`, call.function_name, 'label'),
          text(`step-${step.step_id}-tool-ref-${index}`, call.tool_call_id, 'caption', 'textMuted'),
        ],
      ),
      codeBlock(
        `step-${step.step_id}-tool-args-${index}`,
        JSON.stringify(call.arguments, null, 2),
      ),
      ...(observation === undefined
        ? []
        : [
            text(`step-${step.step_id}-observation-label-${index}`, 'Observation', 'caption', 'accent'),
            codeBlock(`step-${step.step_id}-observation-${index}`, observation.content),
          ]),
    ],
  )
}

const traceTranscript = (
  trajectory: AtifTrajectory,
  scrollToKey: string | undefined,
): View =>
  Transcript({
    key: 'trace-timeline',
    messages: timelineSteps(trajectory).map(step => {
      const timestamp = step.timestamp === undefined
        ? undefined
        : new Date(step.timestamp).toLocaleString(undefined, {
            dateStyle: 'medium',
            timeStyle: 'short',
          })
      const toolCallIds = new Set((step.tool_calls ?? []).map(call => call.tool_call_id))
      return {
        key: `step-${step.step_id}`,
        role: step.source === 'agent' ? 'assistant' as const : step.source,
        status: 'done' as const,
        senderLabel: `STEP ${step.step_id} · ${step.source.toUpperCase()}`,
        ...(timestamp === undefined ? {} : { timestamp }),
        body: [
          Stack(
            {
              key: `step-${step.step_id}-body`,
              direction: 'column',
              gap: '3',
              style: { width: 'full' },
            },
            [
              ...(step.message.trim() === ''
                ? []
                : [text(`step-${step.step_id}-message`, step.message, 'body')]),
              ...(step.reasoning_content === undefined || step.reasoning_content.trim() === ''
                ? []
                : [
                    text(`step-${step.step_id}-reasoning-label`, 'Reasoning', 'caption', 'textMuted'),
                    text(`step-${step.step_id}-reasoning`, step.reasoning_content, 'caption', 'textMuted'),
                  ]),
              ...(step.tool_calls ?? []).map((_, index) => toolCallView(step, index)),
              ...(step.observation?.results ?? [])
                .filter(result => result.source_call_id === undefined || !toolCallIds.has(result.source_call_id))
                .map((result, index) => codeBlock(
                  `step-${step.step_id}-standalone-observation-${index}`,
                  result.content,
                )),
            ],
          ),
        ],
      }
    }),
    pinToEnd: false,
    preserveScrollAnchor: true,
    ...(scrollToKey === undefined ? {} : { scrollToKey }),
    virtualize: false,
    style: { width: 'full' },
  })

const metadataRow = (label: string, value: string, index: number) => ({
  id: `trace-meta-${index}`,
  cells: [
    text(`trace-meta-${index}-label`, label, 'caption', 'textMuted'),
    text(`trace-meta-${index}-value`, value, 'label'),
  ],
})

const loadedTraceView = (state: Extract<TraceSurfaceState, { tag: 'loaded' }>): View => {
  const { projection } = state
  const trajectory = projection.trajectory
  const verdict = traceVerdict(trajectory)
  const goal = firstUserStep(trajectory)
  const images = projection.blobRefs.filter(ref => ref.kind === 'screenshot' || ref.kind === 'image')

  return Stack(
    {
      key: 'trace-root',
      direction: 'column',
      gap: '0',
      style: { backgroundColor: 'background', minHeight: 'full', width: 'full' },
    },
    [
      Stack(
        {
          key: 'trace-topbar',
          direction: 'row',
          align: 'center',
          justify: 'between',
          gap: '3',
          padding: '3',
          style: { backgroundColor: 'surface', borderColor: 'border', borderWidth: 1, width: 'full' },
        },
        [
          text('trace-brand', 'OA / TRACE', 'label'),
          Stack(
            { key: 'trace-topbar-actions', direction: 'row', align: 'center', gap: '2' },
            [
              Badge({
                key: 'trace-visibility',
                label: projection.visibility.replace('_', ' '),
                tone: 'neutral',
                variant: 'outline',
                size: 'sm',
              }),
              Button({
                key: 'trace-copy',
                label: state.copied ? 'Copied' : 'Copy trace',
                variant: 'outline',
                size: 'sm',
                onPress: IntentRef('TraceCopyRequested', StaticPayload({})),
              }),
            ],
          ),
        ],
      ),
      Stack(
        {
          key: 'trace-article',
          direction: 'column',
          gap: '6',
          padding: '6',
          style: { alignSelf: 'center', maxWidth: 896, width: 'full' },
        },
        [
          Stack({ key: 'trace-title-block', direction: 'column', gap: '3', style: { width: 'full' } }, [
            text('trace-eyebrow', 'Agent session trace', 'caption', 'textMuted'),
            text(
              'trace-title',
              terminalSummary(trajectory) ?? trajectory.trajectory_id,
              'heading',
            ),
            text('trace-uuid', projection.uuid, 'caption', 'textMuted'),
            Badge({
              key: 'trace-verdict',
              label: verdictLabel(verdict),
              tone: verdictTone(verdict),
              variant: 'soft',
              size: 'md',
            }),
          ]),
          Divider({ key: 'trace-header-divider' }),
          Stack({ key: 'trace-metadata', direction: 'column', gap: '2', style: { width: 'full' } }, [
            ...[
              ['Agent', trajectory.agent.name],
              ['Model', trajectory.agent.model_name ?? 'Unknown'],
              ['Duration', formatDuration(traceDurationMs(trajectory))],
              ['Cost', formatCost(trajectory.final_metrics?.total_cost_usd)],
              ['Steps', formatNumber(projection.stepCount)],
            ].map(([label, value], index) => Card(
              {
                key: `trace-meta-card-${index}`,
                padding: '3',
                radius: 'none',
                style: { borderColor: 'border', borderWidth: 1, width: 'full' },
              },
              [
                Stack(
                  { key: `trace-meta-row-${index}`, direction: 'row', gap: '4', justify: 'between' },
                  metadataRow(label ?? '', value ?? '', index).cells,
                ),
              ],
            )),
          ]),
          text(
            'trace-authority',
            'Evidence only. This trace grants no accepted-work, payout, or public-claim authority.',
            'caption',
            'textMuted',
          ),
          ...(goal === undefined
            ? []
            : [
                Divider({ key: 'trace-goal-divider' }),
                text('trace-goal-label', 'Goal', 'label', 'textMuted'),
                Card(
                  {
                    key: 'trace-goal',
                    padding: '4',
                    radius: 'none',
                    style: {
                      backgroundColor: 'surface',
                      borderColor: 'border',
                      borderWidth: 1,
                      width: 'full',
                    },
                  },
                  [text('trace-goal-copy', goal.message, 'body')],
                ),
              ]),
          Divider({ key: 'trace-timeline-divider' }),
          text('trace-timeline-label', 'Timeline', 'label', 'textMuted'),
          traceTranscript(trajectory, state.scrollToKey),
          ...(images.length === 0
            ? []
            : [
                Divider({ key: 'trace-images-divider' }),
                text('trace-images-label', 'Screenshots', 'label', 'textMuted'),
                ...images.map((image, index) => Stack(
                  { key: `trace-image-${index}`, direction: 'column', gap: '2', style: { width: 'full' } },
                  [
                    Image({
                      key: `trace-image-content-${index}`,
                      alt: image.caption ?? 'Trace screenshot',
                      source: new URL(
                        traceBlobUrl(projection.uuid, image.r2Key, state.token),
                        state.origin,
                      ).href,
                      width: 'full',
                      height: 480,
                      fit: 'contain',
                      style: { borderRadius: 'none', width: 'full' },
                    }),
                    text(
                      `trace-image-caption-${index}`,
                      image.caption ?? image.r2Key,
                      'caption',
                      'textMuted',
                    ),
                  ],
                )),
              ]),
        ],
      ),
    ],
  )
}

const statusTraceView = (state: Exclude<TraceSurfaceState, { tag: 'loaded' }>): View => {
  const loading = state.tag === 'loading'
  const notFound = state.tag === 'failed' && (state.status === 404 || state.status === 403)
  return Stack(
    {
      key: 'trace-status-root',
      direction: 'column',
      align: 'center',
      justify: 'center',
      padding: '6',
      style: { backgroundColor: 'background', minHeight: 'full', width: 'full' },
    },
    [
      Card(
        {
          key: 'trace-status-card',
          padding: '6',
          radius: 'none',
          style: {
            backgroundColor: 'surface',
            borderColor: 'border',
            borderWidth: 1,
            maxWidth: 480,
            width: 'full',
          },
        },
        [
          text('trace-status-label', 'Trace', 'caption', 'textMuted'),
          text(
            'trace-status-title',
            loading ? 'Loading trace' : notFound ? 'No trace at this link' : 'Trace unavailable',
            'title',
          ),
          text(
            'trace-status-body',
            loading
              ? 'Reading the stored ATIF evidence.'
              : notFound
                ? 'This trace does not exist, is private, or is no longer available.'
                : 'The trace could not be loaded. Try again shortly.',
            'body',
            'textMuted',
          ),
          ...(loading
            ? []
            : [Button({
                key: 'trace-home',
                label: 'Go home',
                variant: 'outline',
                onPress: IntentRef('TraceHomeRequested', StaticPayload({})),
              })]),
        ],
      ),
    ],
  )
}

export const traceSurfaceView = (state: TraceSurfaceState): View =>
  state.tag === 'loaded' ? loadedTraceView(state) : statusTraceView(state)

export const mountTraceEffectNativeSurface = (
  container: HTMLElement,
  initialState: TraceSurfaceState,
) => Effect.gen(function* () {
  const state = yield* SubscriptionRef.make(initialState)
  const program = makeViewProgramFromState(state, traceSurfaceView)
  const handlers: IntentHandlers<typeof traceIntents> = {
    TraceCopyRequested: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (current.tag !== 'loaded') return
      yield* Effect.tryPromise({
        try: () => navigator.clipboard.writeText(trajectoryToMarkdown(current.projection.trajectory)),
        catch: () => undefined,
      }).pipe(Effect.catch(() => Effect.void))
      yield* SubscriptionRef.set(state, { ...current, copied: true })
    }),
    TraceHomeRequested: () => Effect.sync(() => { window.location.assign('/') }),
  }
  const registry = yield* makeIntentRegistry(traceIntents, handlers)
  const report: IntentReporter = (ref, runtimeValue) =>
    registry.dispatch(resolveIntentRef(ref, runtimeValue))
  const surface = yield* makeDomRenderer({ theme: khalaTheme })
    .mount(container, program.viewStream, report)
  return { state, unmount: surface.unmount }
})

function TraceEffectNativeSurface({ state }: Readonly<{ state: TraceSurfaceState }>) {
  const rootRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const root = rootRef.current
    if (root === null) return undefined
    let disposed = false
    let closeScope: (() => void) | undefined
    void Effect.runPromise(Scope.make())
      .then(scope => {
        const close = () => { void Effect.runPromise(Scope.close(scope, Exit.void)) }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(Scope.provide(scope)(mountTraceEffectNativeSurface(root, state)))
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      closeScope?.()
    }
  }, [state])

  return <div ref={rootRef} data-trace-effect-native-root="" />
}

export function TraceLoadedView({
  projection,
  token,
}: Readonly<{ projection: TraceProjection; token?: string }>) {
  const hashStep = typeof window === 'undefined'
    ? undefined
    : /^#step-\d+$/.test(window.location.hash)
      ? window.location.hash.slice(1)
      : undefined
  const video = projection.blobRefs.find(ref => ref.kind === 'video')
  const state: TraceSurfaceState = {
    tag: 'loaded',
    projection,
    origin: typeof window === 'undefined' ? 'https://openagents.com' : window.location.origin,
    copied: false,
    ...(token === undefined ? {} : { token }),
    ...(hashStep === undefined ? {} : { scrollToKey: hashStep }),
  }

  return (
    <main className="h-dvh overflow-auto bg-khala-void text-khala-text" data-component="trace-page" data-route="trace" data-trace-effect-native="">
      <TraceEffectNativeSurface state={state} />
      {video === undefined ? null : (
        // The Effect Native media-video Host is intentionally stream-only. This
        // bounded URL-playback element is renderer machinery until the catalog
        // gains a typed recorded-media source contract.
        <section className="mx-auto grid w-full max-w-[896px] gap-3 border-t border-khala-border px-6 py-8" data-component="trace-recording">
          <p className="m-0 font-mono text-xs uppercase tracking-[0.16em] text-khala-text-faint">Session recording</p>
          <video className="aspect-video w-full border border-khala-border bg-black object-contain" controls playsInline preload="metadata" src={traceBlobUrl(projection.uuid, video.r2Key, token)} />
          <p className="m-0 font-mono text-xs text-khala-text-faint">{video.caption ?? video.r2Key}</p>
        </section>
      )}
    </main>
  )
}

export function TraceFailedView({ status }: Readonly<{ status: number }>) {
  return (
    <main className="h-dvh overflow-auto bg-khala-void" data-component="trace-not-found" data-route="trace" data-trace-effect-native="">
      <TraceEffectNativeSurface state={{ tag: 'failed', status }} />
    </main>
  )
}

function TraceLoadingView() {
  return (
    <main aria-busy="true" className="h-dvh overflow-auto bg-khala-void" data-component="trace-skeleton" data-route="trace" data-trace-effect-native="">
      <TraceEffectNativeSurface state={{ tag: 'loading' }} />
    </main>
  )
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
      setState(result.tag === 'loaded'
        ? { tag: 'loaded', projection: result.projection }
        : { tag: 'failed', status: result.status })
    })
    return () => { cancelled = true }
  }, [token, traceUuid])

  if (state.tag === 'loaded') return (
    <TraceLoadedView
      projection={state.projection}
      {...(token === undefined ? {} : { token })}
    />
  )
  if (state.tag === 'failed') return <TraceFailedView status={state.status} />
  return <TraceLoadingView />
}
