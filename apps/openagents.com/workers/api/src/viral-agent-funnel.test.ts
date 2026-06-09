import { describe, expect, test } from 'vitest'

import {
  classifyViralFunnelActor,
  classifyViralFunnelUserAgent,
  makeViralAgentFunnelEventRecord,
  recordViralAgentFunnelEvent,
} from './viral-agent-funnel'

class RecordingD1Statement {
  readonly bound: Array<unknown> = []

  constructor(
    private readonly db: RecordingD1Database,
    readonly query: string,
  ) {}

  bind(...values: Array<unknown>): RecordingD1Statement {
    this.bound.push(...values)

    return this
  }

  run(): Promise<void> {
    this.db.runs.push({
      query: this.query,
      values: this.bound,
    })

    return Promise.resolve()
  }
}

class RecordingD1Database {
  readonly runs: Array<Readonly<{ query: string; values: Array<unknown> }>> = []

  prepare(query: string): RecordingD1Statement {
    return new RecordingD1Statement(this, query)
  }
}

describe('viral agent funnel metrics', () => {
  test('classifies public agent, browser, crawler, and auth surfaces coarsely', () => {
    expect(classifyViralFunnelUserAgent('codex-cli/1.0')).toBe(
      'agent_or_cli',
    )
    expect(classifyViralFunnelUserAgent('Mozilla/5.0 Safari/605')).toBe(
      'browser',
    )
    expect(classifyViralFunnelUserAgent('ExampleBot/1.0')).toBe('crawler')
    expect(classifyViralFunnelUserAgent(null)).toBe('unknown')
    expect(
      classifyViralFunnelActor(
        new Request('https://openagents.com/.well-known/openagents.json'),
      ),
    ).toBe('public_anonymous')
    expect(
      classifyViralFunnelActor(
        new Request('https://openagents.com/api/openapi.json', {
          headers: { authorization: 'Bearer scoped_test' },
        }),
      ),
    ).toBe('scoped_agent_possible')
    expect(
      classifyViralFunnelActor(
        new Request('https://openagents.com/api/openapi.json', {
          headers: { cookie: 'openagents_session=present' },
        }),
      ),
    ).toBe('signed_in_browser_possible')
  })

  test('builds bounded public-safe event records without raw prompts or tokens', () => {
    const record = makeViralAgentFunnelEventRecord(
      new Request('https://openagents.com/api/public/proof/otec', {
        headers: { 'user-agent': 'codex-cli/1.0' },
      }),
      {
        eventKind: 'public_proof_read',
        metadata: {
          badKey: 'x'.repeat(400),
          prompt: 'do not store raw user prompts here',
        },
        proofRef: 'proof:otec',
        route: '/api/public/proof/otec',
        siteSlug: 'otec',
      },
      {
        makeEventId: () => 'viral_funnel_event_1',
        nowIso: () => '2026-06-05T12:00:00.000Z',
      },
    )

    expect(record).toMatchObject({
      actorClass: 'public_anonymous',
      eventKind: 'public_proof_read',
      id: 'viral_funnel_event_1',
      proofRef: 'proof:otec',
      route: '/api/public/proof/otec',
      siteSlug: 'otec',
      userAgentClass: 'agent_or_cli',
    })
    expect(record.metadataJson.length).toBeLessThan(260)
    expect(record.metadataJson).not.toContain('x'.repeat(200))
    expect(record.metadataJson).not.toContain('gho_')
  })

  test('records bounded event rows without private request material', async () => {
    const db = new RecordingD1Database()
    await recordViralAgentFunnelEvent(
      db as unknown as D1Database,
      new Request('https://openagents.com/.well-known/openagents.json', {
        headers: {
          authorization: 'Bearer should_not_be_stored',
          'user-agent': 'curl/8.0',
        },
      }),
      {
        eventKind: 'capability_manifest_read',
        metadata: { source: 'public_route' },
        route: '/.well-known/openagents.json',
      },
      {
        makeEventId: () => 'viral_funnel_event_1',
        nowIso: () => '2026-06-05T12:00:00.000Z',
      },
    )

    expect(db.runs).toHaveLength(1)
    expect(db.runs[0]?.query).toContain(
      'INSERT INTO viral_agent_funnel_events',
    )
    expect(db.runs[0]?.values).toContain('capability_manifest_read')
    expect(db.runs[0]?.values).toContain('scoped_agent_possible')
    expect(db.runs[0]?.values).toContain('agent_or_cli')
    expect(JSON.stringify(db.runs)).not.toContain('should_not_be_stored')
  })
})
