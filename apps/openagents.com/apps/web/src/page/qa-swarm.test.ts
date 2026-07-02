import { Option, Schema as S } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from '../main'
import { QaSwarmRoute, urlToAppRoute } from '../route'
import * as QaSwarm from './qa-swarm'
import {
  QA_SWARM_SAMPLE_RUN_REF,
  QaSwarmRunProjection,
  assertQaSwarmPublicProjection,
  lookupQaSwarmRunProjection,
  qaSwarmProjectionHasPrivateMaterial,
  sampleQaSwarmRunProjection,
} from './qa-swarm/projection'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

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
  const style = Object.entries(node.data?.style ?? {})
    .map(([key, value]) => `${key}:${value}`)
    .join(';')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
    ...(style.length === 0 ? [] : [['style', style] as const]),
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
  if (html === null || !isVNodeLike(html)) return ''
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
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

describe('QA Swarm route', () => {
  test('parses /qa/{runRef} and stays public', () => {
    const route = urlToAppRoute(appUrl(`/qa/${QA_SWARM_SAMPLE_RUN_REF}`))
    expect(route).toEqual(QaSwarmRoute({ runRef: QA_SWARM_SAMPLE_RUN_REF }))

    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl(`/qa/${QA_SWARM_SAMPLE_RUN_REF}`),
    )
    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'QaSwarm', runRef: QA_SWARM_SAMPLE_RUN_REF },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders the committed public-safe run projection end to end', () => {
    const rendered = renderHtml(
      QaSwarm.view(QaSwarmRoute({ runRef: QA_SWARM_SAMPLE_RUN_REF }), {
        _tag: 'LoggedOut',
      }),
    )

    expect(rendered).toContain('data-component="qa-swarm-run-page"')
    expect(rendered).toContain('Khala Code nightly QA swarm')
    expect(rendered).toContain('QA Swarm run')
    expect(rendered).toContain('Verdict wall')
    expect(rendered).toContain('Coverage + frontier')
    expect(rendered).toContain('Perf budgets')
    expect(rendered).toContain('Videos and traces')
    expect(rendered).toContain('/trace/24c6fea6-b271-46c6-a9a9-bc614440e9ef')
    expect(rendered).toContain('/docs/qa/khala-code-mechanical-corpus')
    expect(rendered).toContain('artifact.qa_swarm.target.opaque.customer_one')
    expect(rendered).not.toMatch(/\/Users\/|bearer|token|secret/i)
  })

  test('does not disclose unknown or owner-only run refs', () => {
    const rendered = renderHtml(
      QaSwarm.view(QaSwarmRoute({ runRef: 'qa-run.private.customer-one' }), {
        _tag: 'LoggedOut',
      }),
    )

    expect(rendered).toContain('data-component="qa-swarm-not-found"')
    expect(rendered).toContain('Run unavailable')
    expect(rendered).toContain('Private or owner-only targets are not disclosed')
  })
})

describe('QA Swarm projection schema and redaction', () => {
  test('decodes the sample projection and declares staleness', () => {
    expect(S.decodeUnknownSync(QaSwarmRunProjection)(sampleQaSwarmRunProjection))
      .toEqual(sampleQaSwarmRunProjection)
    expect(sampleQaSwarmRunProjection.schemaVersion).toBe(
      'openagents.qa_swarm.run_projection.v1',
    )
    expect(sampleQaSwarmRunProjection.staleness).toEqual({
      contractVersion: 'projection_staleness.v1',
      maxAgeHours: 24,
      mode: 'artifact_snapshot',
    })
    expect(lookupQaSwarmRunProjection(QA_SWARM_SAMPLE_RUN_REF)).toEqual(
      sampleQaSwarmRunProjection,
    )
  })

  test('keeps non-owner targets opaque and public-safe', () => {
    expect(sampleQaSwarmRunProjection.target.visibility).toBe('opaque')
    expect(sampleQaSwarmRunProjection.opaqueTargetRefs).toContain(
      sampleQaSwarmRunProjection.target.ref,
    )
    expect(qaSwarmProjectionHasPrivateMaterial(sampleQaSwarmRunProjection))
      .toBe(false)
  })

  test('redaction tripwire rejects private refs', () => {
    expect(() =>
      assertQaSwarmPublicProjection({
        ...sampleQaSwarmRunProjection,
        traceRefs: ['/Users/operator/private/raw-trace.json'],
      }),
    ).toThrow(/private material|Unsafe QA Swarm projection ref/)
  })
})
