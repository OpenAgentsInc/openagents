import { describe, expect, test, vi } from 'vitest'

const recorder = await import('./customer-one-cohort-recorder.mjs')

const outputBuffer = () => {
  const writes: Array<string> = []

  return {
    sink: {
      write: (chunk: string) => {
        writes.push(chunk)
      },
    },
    text: () => writes.join(''),
  }
}

const completedRow = {
  artifactRef: 'artifact.customer-one.team-1.delivery.v1',
  blockerRefs: [],
  caveatRefs: [],
  completionBundleRef: 'completion.customer-one.team-1.bundle.v1',
  privacyReviewRef: 'privacy.customer-one.team-1.review.v1',
  reviewRef: 'review.customer-one.team-1.human.v1',
  routingRef: 'routing.customer-one.team-1.owned-node.v1',
  runRef: 'run.customer-one.team-1.primary.v1',
  state: 'loop_completed',
  teamCohortRef: 'cohort.team.alpha.v1',
  templateRef: 'forge.template.ecommerce.inventory_campaign.v1',
  updatedAt: '2026-06-17T20:00:00.000Z',
  verificationRef: 'verification.customer-one.team-1.smoke.v1',
  verticalRef: 'vertical.ecommerce.v1',
  workspaceRef: 'workspace.customer-one.team-1.v1',
}

const blockedProjection = {
  blockerRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
  counts: { loop_completed: 0 },
  gate: {
    reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
    state: 'blocked',
  },
  rows: [],
  target: { minimumCompletedTeams: 3 },
}

const readyProjection = {
  blockerRefs: [],
  counts: { loop_completed: 3 },
  gate: {
    reasonRefs: [],
    state: 'ready',
  },
  rows: [
    { countsTowardD3Completion: true },
    { countsTowardD3Completion: true },
    { countsTowardD3Completion: true },
  ],
  target: { minimumCompletedTeams: 3 },
}

describe('customer one cohort recorder', () => {
  test('reads the public cohort projection without an admin token', async () => {
    const stdout = outputBuffer()
    const stderr = outputBuffer()
    const fetchImpl = vi.fn(async (url: URL, init: RequestInit) => {
      expect(url.href).toBe(
        'https://example.test/api/public/customer-one-cohort',
      )
      expect(init.method).toBe('GET')
      expect(init.headers).toEqual({ accept: 'application/json' })

      return new Response(
        JSON.stringify({
          counts: { loop_completed: 0 },
          gate: { state: 'blocked' },
          rows: [],
        }),
        {
          headers: { 'content-type': 'application/json' },
          status: 200,
        },
      )
    })

    const exitCode = await recorder.runRecorder(
      ['public'],
      { OPENAGENTS_BASE_URL: 'https://example.test' },
      { fetchImpl, stderr: stderr.sink, stdout: stdout.sink },
    )

    expect(exitCode).toBe(0)
    expect(fetchImpl).toHaveBeenCalledTimes(1)
    expect(stdout.text()).toContain('Customer #1 cohort gate: blocked')
    expect(stdout.text()).toContain('Completed teams: 0')
    expect(stderr.text()).toBe('')
  })

  test('refuses private row commands without an admin token', () => {
    const parsed = recorder.parseRecorderArgs(['list'])

    expect(() => recorder.buildRecorderRequest(parsed, {})).toThrow(
      /Missing OPENAGENTS_ADMIN_API_TOKEN/,
    )
  })

  test('builds an admin upsert request from inline row JSON', () => {
    const parsed = recorder.parseRecorderArgs([
      'upsert',
      '--row-json',
      JSON.stringify(completedRow),
    ])
    const request = recorder.buildRecorderRequest(parsed, {
      OPENAGENTS_ADMIN_API_TOKEN: 'oa_admin_secret_123',
    })

    expect(request).toMatchObject({
      body: completedRow,
      headers: {
        Authorization: 'Bearer oa_admin_secret_123',
        accept: 'application/json',
        'content-type': 'application/json',
      },
      method: 'POST',
      path: '/api/operator/customer-one-cohort/rows',
    })
  })

  test('checks a valid privacy-reviewed completion row without a network request', async () => {
    const stdout = outputBuffer()
    const stderr = outputBuffer()
    const fetchImpl = vi.fn()

    const exitCode = await recorder.runRecorder(
      ['check', '--row-json', JSON.stringify(completedRow)],
      {},
      { fetchImpl, stderr: stderr.sink, stdout: stdout.sink },
    )

    expect(exitCode).toBe(0)
    expect(fetchImpl).not.toHaveBeenCalled()
    expect(stdout.text()).toContain(
      'Customer #1 cohort row packet valid: cohort.team.alpha.v1',
    )
    expect(stdout.text()).toContain('Counts toward D3: yes')
    expect(stderr.text()).toBe('')
  })

  test('passes the public cohort audit when the projection gate is ready', async () => {
    const stdout = outputBuffer()
    const stderr = outputBuffer()
    const fetchImpl = vi.fn(async (url: URL, init: RequestInit) => {
      expect(url.href).toBe(
        'https://example.test/api/public/customer-one-cohort',
      )
      expect(init.method).toBe('GET')
      expect(init.headers).toEqual({ accept: 'application/json' })

      return new Response(JSON.stringify(readyProjection), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    })

    const exitCode = await recorder.runRecorder(
      ['audit'],
      { OPENAGENTS_BASE_URL: 'https://example.test' },
      { fetchImpl, stderr: stderr.sink, stdout: stdout.sink },
    )

    expect(exitCode).toBe(0)
    expect(fetchImpl).toHaveBeenCalledTimes(1)
    expect(stdout.text()).toContain('Customer #1 cohort audit: ready')
    expect(stdout.text()).toContain('Completed teams: 3/3')
    expect(stdout.text()).toContain('Counted completion rows: 3/3')
    expect(stderr.text()).toBe('')
  })

  test('fails the public cohort audit for the current zero-row blocked state', async () => {
    const stdout = outputBuffer()
    const stderr = outputBuffer()
    const fetchImpl = vi.fn(async () =>
      new Response(JSON.stringify(blockedProjection), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      }),
    )

    const exitCode = await recorder.runRecorder(
      ['audit'],
      { OPENAGENTS_BASE_URL: 'https://example.test' },
      { fetchImpl, stderr: stderr.sink, stdout: stdout.sink },
    )

    expect(exitCode).toBe(1)
    expect(stdout.text()).toContain('Customer #1 cohort audit: blocked')
    expect(stdout.text()).toContain('Completed teams: 0/3')
    expect(stdout.text()).toContain('Counted completion rows: 0/3')
    expect(stderr.text()).toContain(
      'customer-one-cohort-audit:gate-blocked',
    )
    expect(stderr.text()).toContain(
      'customer-one-cohort-audit:insufficient-completed-count',
    )
  })

  test('refuses obvious private material before upsert', () => {
    const parsed = recorder.parseRecorderArgs([
      'upsert',
      '--row-json',
      JSON.stringify({
        ...completedRow,
        workspaceRef: '/Users/operator/private-workspace',
      }),
    ])

    expect(() =>
      recorder.buildRecorderRequest(parsed, {
        OPENAGENTS_ADMIN_API_TOKEN: 'oa_admin_secret_123',
      }),
    ).toThrow(/unsafe private material/)
  })

  test('refuses completed rows missing privacy-review evidence', () => {
    const parsed = recorder.parseRecorderArgs([
      'check',
      '--row-json',
      JSON.stringify({
        ...completedRow,
        privacyReviewRef: '',
      }),
    ])

    expect(() => recorder.readRowInput(parsed)).toThrow(
      /loop_completed rows require privacyReviewRef/,
    )
  })

  test('refuses unresolved template placeholders', () => {
    const parsed = recorder.parseRecorderArgs([
      'check',
      '--row-json',
      JSON.stringify({
        ...completedRow,
        teamCohortRef: 'cohort.team.replace-me.v1',
      }),
    ])

    expect(() => recorder.readRowInput(parsed)).toThrow(
      /unresolved template placeholders/,
    )
  })

  test('redacts admin and bearer tokens from printable errors', () => {
    const secret = 'oa_admin_secret_123'
    const text = recorder.redactSecrets(
      `Authorization: Bearer ${secret} OPENAGENTS_ADMIN_API_TOKEN=${secret}`,
      [secret],
    )

    expect(text).not.toContain(secret)
    expect(text).toContain('Bearer <redacted>')
    expect(text).toContain('OPENAGENTS_ADMIN_API_TOKEN=<redacted>')
  })
})
