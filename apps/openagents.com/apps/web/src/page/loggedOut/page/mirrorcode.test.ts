import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import {
  IdleMirrorCodeRuns,
  LoadedMirrorCodeRuns,
  LoadingMirrorCodeRuns,
} from '../model'
import type { MirrorCodeRunsResponse } from '../mirrorcode/runs'
import * as MirrorCode from './mirrorcode'

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
    style?: Record<string, string>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

// A test-local loaded response mirroring the live `/api/gym/mirrorcode/runs`
// shape (#6378). NOT a shipped fixture: it only drives render assertions.
const response: MirrorCodeRunsResponse = {
  generatedAt: '2026-06-27T05:00:00.000Z',
  model: 'openagents/khala',
  benchmark: {
    name: 'Epoch Research MirrorCode',
    scope: 'public tasks only (private set excluded)',
  },
  runs: [
    {
      runId: 'run.mirrorcode.khala.smoke-001',
      model: 'openagents/khala',
      taskId: 'mirrorcode/leftpad',
      bucket: 'S',
      language: 'python',
      status: 'passed',
      passRate: 1,
      tokensTotal: 12_345_678,
      startedAt: '2026-06-27T04:00:00.000Z',
      finishedAt: '2026-06-27T04:05:00.000Z',
      summary: 'Reimplemented left-pad from scratch; held-out suite passed.',
      grade: 'smoke',
      decisionGrade: false,
    },
    {
      runId: 'run.mirrorcode.khala.smoke-002',
      model: 'openagents/khala',
      taskId: 'mirrorcode/jsonparse',
      bucket: 'M',
      language: null,
      status: 'running',
      passRate: null,
      tokensTotal: 4_200,
      startedAt: '2026-06-27T04:30:00.000Z',
      finishedAt: null,
      summary: 'In flight against the JSON parser task.',
      grade: 'smoke',
      decisionGrade: false,
    },
  ],
  comparators: [
    {
      label: 'Paper A',
      model: 'frontier-x',
      source: 'paper_reference_illustrative',
      note: 'Quoted under a different harness; context only.',
    },
  ],
}

describe('public MirrorCode page', () => {
  test('renders the honest empty state when there are no runs yet', () => {
    const rendered = renderHtml(MirrorCode.view(IdleMirrorCodeRuns()))

    expect(rendered).toContain('data-mirrorcode-page')
    expect(rendered).toContain('MirrorCode, powered by Khala')
    expect(rendered).toContain('public tasks only')
    expect(rendered).toContain('data-mirrorcode-live-empty')
    expect(rendered).toContain(
      'No runs yet — machinery shipped, awaiting first Phase-0 run',
    )
    // Comparators section is always labeled as illustrative, never head-to-head.
    expect(rendered).toContain(
      'Paper-reference comparators (illustrative — not a head-to-head)',
    )
  })

  test('renders a loading state while the projection is read', () => {
    const rendered = renderHtml(MirrorCode.view(LoadingMirrorCodeRuns()))

    expect(rendered).toContain('data-mirrorcode-live-loading')
  })

  test('renders the latest run, leaderboard, and comparators when loaded', () => {
    const rendered = renderHtml(MirrorCode.view(LoadedMirrorCodeRuns({ response })))

    // Empty state is gone.
    expect(rendered).not.toContain('data-mirrorcode-live-empty')

    // Latest run is the newest by startedAt (the running JSON parse run).
    expect(rendered).toContain('data-mirrorcode-latest-run')
    expect(rendered).toContain('mirrorcode/jsonparse')

    // Compact token formatting (12_345_678 -> 12.3M).
    expect(rendered).toContain('12.3M')

    // Pass-rate as a percentage and the null "not measured".
    expect(rendered).toContain('100.0%')
    expect(rendered).toContain('not measured')

    // Status markers with semantic accents.
    expect(rendered).toContain('data-mirrorcode-status="passed"')
    expect(rendered).toContain('data-mirrorcode-status="running"')

    // Smoke runs are labeled Phase-0 smoke, never a frontier number.
    expect(rendered).toContain('Phase-0 smoke')

    // Leaderboard + comparators tables present.
    expect(rendered).toContain('data-mirrorcode-leaderboard')
    expect(rendered).toContain('data-mirrorcode-run-row')
    expect(rendered).toContain('data-mirrorcode-comparators')
    expect(rendered).toContain('Paper A')
  })

  test('renders an honest error state without inventing numbers', () => {
    const rendered = renderHtml(
      MirrorCode.view({ _tag: 'MirrorCodeRunsFailed', error: 'HTTP 503' }),
    )

    expect(rendered).toContain('data-mirrorcode-live-error')
    expect(rendered).toContain('Run feed unavailable')
    expect(rendered).toContain('HTTP 503')
  })
})
