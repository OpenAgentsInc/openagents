import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { initGymModel } from '../gym/flow'
import { GYM_RUN_PROGRESS_SCHEMA, type GymRunProgress } from '../gym/runProgress'
import {
  IdlePublicGymRunProgress,
  LoadedPublicGymRunProgress,
} from '../model'
import * as Gym from './gym'

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
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

// A test-local web_authorized run mirroring the live `/api/public/gym/run-
// progress` shape (#6261). This is NOT a shipped fixture: it only drives the
// render assertions. The page renders the honest empty state until real runs
// arrive over the live poll.
const liveRun: GymRunProgress = {
  schemaVersion: GYM_RUN_PROGRESS_SCHEMA,
  runRef: 'run.gym.terminal_bench.glm52-reap-baseline',
  jobRef: 'job.gym.harbor_terminal_bench.glm52-reap-baseline',
  configId: 'gym.terminal_bench.glm52-reap-baseline',
  environmentRef: 'terminal-bench',
  datasetRef: 'terminal-bench@2.0',
  runner: 'harbor',
  agent: 'terminus-2',
  profile: {
    profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
    publicLabel: 'glm-reap baseline profile',
    model: 'openagents/glm-5.2-reap-504b',
    attribution: 'Z.ai GLM-5.2 REAP',
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    contextWindowTokens: 250_000,
  },
  phase: 'running',
  decisionGrade: false,
  inProgress: true,
  publication: 'web_authorized',
  counts: {
    officialDenominator: 89,
    completed: 18,
    completedPassed: 10,
    completedFailed: 8,
    running: 1,
    pending: 70,
    error: 5,
    cancelled: 0,
  },
  passRateOverCompleted: 10 / 18,
  completionFraction: 18 / 89,
  tokens: {
    promptTokens: 61_966_392,
    completionTokens: 296_537,
    totalTokens: 62_262_929,
  },
  elapsedMs: null,
  lastUpdatedAt: '2026-06-25T18:13:35.081Z',
  caveatRefs: [],
  blockerRefs: [],
}

const secondRun: GymRunProgress = {
  ...liveRun,
  runRef: 'run.gym.terminal_bench.khala-live',
  jobRef: 'job.gym.harbor_terminal_bench.khala-live',
  configId: 'gym.terminal_bench.khala-live',
  profile: {
    ...liveRun.profile,
    profileRef: 'khala-public-heuristic',
    publicLabel: 'khala heuristic public route',
    model: 'openagents/khala',
    attribution: 'OpenAgents Khala orchestrator',
    hardwareProfile: 'khala-router',
  },
  counts: {
    officialDenominator: 89,
    completed: 1,
    completedPassed: 0,
    completedFailed: 1,
    running: 1,
    pending: 87,
    error: 0,
    cancelled: 0,
  },
  passRateOverCompleted: 0,
  completionFraction: 1 / 89,
}

describe('public Gym page', () => {
  test('renders the typed config controls and locked economics', () => {
    const rendered = renderHtml(
      Gym.view(initGymModel(), IdlePublicGymRunProgress()),
    )

    expect(rendered).toContain('data-gym-page')
    expect(rendered).toContain('data-gym-no-spend-banner')
    expect(rendered).toContain('data-gym-terminal-bench-panel')
    expect(rendered).toContain('Terminal-Bench 2.0')
    expect(rendered).toContain('Provider fan-out')
    expect(rendered).toContain('Program signature modules')
    expect(rendered).toContain('no spend')
  })

  test('renders honest empty states with NO fixture numbers', () => {
    const rendered = renderHtml(
      Gym.view(initGymModel(), IdlePublicGymRunProgress()),
    )

    // Benchmark comparison empty state (no fixture pass rates).
    expect(rendered).toContain('data-gym-terminal-bench-empty')
    expect(rendered).toContain(
      'No decision-grade benchmark reports published yet',
    )

    // Live run follow-along empty state + accessible mirror marker.
    expect(rendered).toContain('data-gym-run-progress-panel')
    expect(rendered).toContain('data-gym-run-progress-accessible-mirror')
    expect(rendered).toContain('data-gym-run-progress-empty')
    expect(rendered).toContain('No active Gym run')
    expect(rendered).toContain(
      'Live runs appear here when a real Harbor/Khala benchmark is ingested',
    )

    // No run-and-show-fake-report button or fixture result anywhere. (The
    // run-progress / ingest-note data attributes legitimately share the
    // `data-gym-run` prefix, so assert the exact removed markers.)
    expect(rendered).not.toContain('data-gym-run=""')
    expect(rendered).not.toContain('data-gym-result')
    expect(rendered).not.toContain('Run fixture')
    expect(rendered).not.toContain('openagents.gym.fixture_report.v1')

    // None of the removed fabricated numbers / labels may appear.
    expect(rendered).not.toContain('69.7')
    expect(rendered).not.toContain('67.4')
    expect(rendered).not.toContain('70.0')
    expect(rendered).not.toContain('69.1')
    expect(rendered).not.toContain('41 of 89')
    expect(rendered).not.toContain('GLM-5.2 REAP 504B TP4 MTP-2')

    // No raw benchmark content leaks into the rendered surface.
    expect(rendered).not.toContain('private_openai_compat')
    expect(rendered).not.toContain('Bearer')
  })

  test('renders live run-progress runs (counts + in-progress, not the empty state)', () => {
    const rendered = renderHtml(
      Gym.view(
        initGymModel(),
        LoadedPublicGymRunProgress({ runs: [liveRun, secondRun] }),
      ),
    )

    // The empty state is gone while runs exist.
    expect(rendered).not.toContain('data-gym-run-progress-empty')
    expect(rendered).not.toContain('No active Gym run')

    // A mirror per run (two runs right now).
    const mirrorCount = rendered.split(
      'data-gym-run-progress-accessible-mirror',
    ).length - 1
    expect(mirrorCount).toBe(2)

    // Live counts: completed / official denominator, and the in-progress +
    // not-decision-grade honesty markers.
    expect(rendered).toContain('18 / 89')
    expect(rendered).toContain('glm-reap baseline profile')
    expect(rendered).toContain('khala heuristic public route')
    expect(rendered).toContain('in progress')
    expect(rendered).toContain('not decision-grade')
    expect(rendered).toContain('gym-run-in-progress="true"')
    expect(rendered).toContain('gym-run-decision-grade="false"')

    // Pass rate over completed is rendered (10/18 ≈ 55.6%), kept distinct from
    // the official denominator progress.
    expect(rendered).toContain('55.6%')
  })

  test('renders the empty state ONLY for runs:[]', () => {
    const rendered = renderHtml(
      Gym.view(initGymModel(), LoadedPublicGymRunProgress({ runs: [] })),
    )

    expect(rendered).toContain('data-gym-run-progress-empty')
    expect(rendered).toContain('No active Gym run')
    expect(rendered).not.toContain('gym-run-in-progress="true"')
  })
})
