import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

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
    expect(rendered).toContain('Arbiter swarm board')
    expect(rendered).toContain('data-link-id="scenario-to-target"')
    expect(rendered).toContain('data-status="evidence_backed"')
    expect(rendered).toContain('QA Swarm board text mirror')
    expect(rendered).toContain('Coverage + frontier')
    expect(rendered).toContain('Perf budgets')
    expect(rendered).toContain('Findings ledger')
    expect(rendered).toContain('Caught')
    expect(rendered).toContain('Filed')
    expect(rendered).toContain('Fixed')
    expect(rendered).toContain('Distilled')
    expect(rendered).toContain('Case-study seed')
    expect(rendered).toContain('/qa/qa-run.khala-code-nightly.latest')
    expect(rendered).toContain('artifact.qa_swarm.weekly_report.khala_code.latest')
    expect(rendered).toContain('artifact.khala_code.qa_status_surface.latest')
    expect(rendered).toContain('Cockpit blanks when one startup RPC fails')
    expect(rendered).toContain('Videos and traces')
    expect(rendered).toContain('oa-qa-swarm-scene')
    expect(rendered).toContain('Static scene fallback')
    expect(rendered).toContain('scene.qa_swarm.three_effect.additive_hdr_bloom.20260702')
    expect(rendered).toContain('scene.qa_swarm.reduced_motion.static_fallback.20260702')
    expect(rendered).toContain('/trace/24c6fea6-b271-46c6-a9a9-bc614440e9ef')
    expect(rendered).toContain('/docs/qa/khala-code-mechanical-corpus')
    expect(rendered).toContain('/docs/qa/qa-swarm-khala-code-standing-engagement')
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

  test('uses Khala tokens instead of bespoke hex page colors', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/page/qa-swarm.ts'),
      'utf8',
    )

    expect(source).toContain('var(--oa-color-khala-')
    expect(source).not.toMatch(/#[0-9a-fA-F]{3,8}/)
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
    expect(sampleQaSwarmRunProjection.engagement).toMatchObject({
      cadence: 'weekly',
      reportHref: '/qa/qa-run.khala-code-nightly.latest',
      status: 'standing_customer_one',
    })
    expect(sampleQaSwarmRunProjection.findingsLedger).toMatchObject({
      caughtCount: 3,
      filedIssueCount: 3,
      fixedCount: 2,
      distilledRegressionCount: 1,
    })
    expect(sampleQaSwarmRunProjection.caseStudy.href).toBe(
      '/docs/qa/qa-swarm-khala-code-standing-engagement',
    )
    expect(lookupQaSwarmRunProjection(QA_SWARM_SAMPLE_RUN_REF)).toEqual(
      sampleQaSwarmRunProjection,
    )
    expect(sampleQaSwarmRunProjection.boardGraph.schemaVersion).toBe(
      'openagents.arbiter.graph_spec.v0',
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

  test('board projection refuses evidence-backed links without receipts', () => {
    const broken = {
      ...sampleQaSwarmRunProjection,
      boardGraph: {
        ...sampleQaSwarmRunProjection.boardGraph,
        links: sampleQaSwarmRunProjection.boardGraph.links.map(link =>
          link.id === 'scenario-to-target'
            ? { ...link, status: 'evidence_backed' as const, evidenceRefs: [] }
            : link,
        ),
      },
    }

    expect(() =>
      assertQaSwarmPublicProjection(broken),
    ).toThrow(/lit without receipt/)
  })

  test('redaction tripwire rejects unsafe scene refs', () => {
    expect(() =>
      assertQaSwarmPublicProjection({
        ...sampleQaSwarmRunProjection,
        scene: {
          ...sampleQaSwarmRunProjection.scene,
          qualityRefs: ['raw.qa_swarm.scene.private_payload'],
        },
      }),
    ).toThrow(/private material|Unsafe QA Swarm projection ref/)
  })
})
