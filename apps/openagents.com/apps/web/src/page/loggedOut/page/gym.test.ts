import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { initGymModel } from '../gym/flow'
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

describe('public Gym page', () => {
  test('renders the typed config controls and locked economics', () => {
    const rendered = renderHtml(Gym.view(initGymModel()))

    expect(rendered).toContain('data-gym-page')
    expect(rendered).toContain('data-gym-no-spend-banner')
    expect(rendered).toContain('data-gym-terminal-bench-panel')
    expect(rendered).toContain('Terminal-Bench 2.0')
    expect(rendered).toContain('Provider fan-out')
    expect(rendered).toContain('Program signature modules')
    expect(rendered).toContain('no spend')
  })

  test('renders honest empty states with NO fixture numbers', () => {
    const rendered = renderHtml(Gym.view(initGymModel()))

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
})
