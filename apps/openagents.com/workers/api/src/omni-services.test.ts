import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentRunBundle,
  type DeploymentBundle,
  type DispatchResult,
  type OmniRunStore,
  createQueuedAgentRun,
  createQueuedDeployment,
  parseGithubRepository,
} from './omni-runs'
import { makeOmniAssignmentService } from './omni/assignments'
import { makeOmniDeploymentRepository } from './omni/deployment-repository'
import { makeOmniDispatchService } from './omni/dispatch-service'
import {
  OmniBillingError,
  OmniDispatchError,
  OmniDispatchMalformedResponse,
  OmniDispatchMissingCredentials,
  OmniDispatchRejectedRequest,
  OmniDispatchTimeout,
  OmniDispatchTransportFailure,
  OmniDispatchUnavailableEndpoint,
  OmniRepositoryError,
  OmniRunnerCallbackDecodeError,
} from './omni/errors'
import { makeOmniOperatorService } from './omni/operator-service'
import { makeOmniPublicProjectionService } from './omni/public-service'
import { makeOmniRunRepository } from './omni/run-repository'
import { makeOmniRunnerEventService } from './omni/runner-events'

const repository = parseGithubRepository('OpenAgentsInc/autopilot-omega')

const unusedDatabase: D1Database = {
  batch: () => {
    throw new Error('unused database')
  },
  dump: () => {
    throw new Error('unused database')
  },
  exec: () => {
    throw new Error('unused database')
  },
  prepare: () => {
    throw new Error('unused database')
  },
  withSession: () => {
    throw new Error('unused database')
  },
}

const makeQueuedAgentRun = () =>
  createQueuedAgentRun({
    appOrigin: 'https://openagents.com',
    goal: 'Run the smoke test.',
    repository,
    runId: 'agent_run_test',
    userId: 'github:1',
  })

const makeQueuedDeployment = () =>
  createQueuedDeployment({
    appOrigin: 'https://openagents.com',
    repository,
    userId: 'github:1',
  })

const makeMemoryOmniRunStore = (): OmniRunStore => {
  const queuedRun = makeQueuedAgentRun()
  const queuedDeployment = makeQueuedDeployment()
  const state: {
    deploymentBundle: DeploymentBundle | undefined
    runBundle: AgentRunBundle | undefined
  } = {
    deploymentBundle: {
      deployment: queuedDeployment.deployment,
      events: queuedDeployment.events,
    },
    runBundle: {
      run: queuedRun.run,
      events: queuedRun.events,
    },
  }

  return {
    appendAgentRunEvents: async (runId, events, status, externalRunId) => {
      if (state.runBundle === undefined || state.runBundle.run.id !== runId) {
        return
      }

      state.runBundle = {
        events: [...state.runBundle.events, ...events],
        run: {
          ...state.runBundle.run,
          externalRunId: externalRunId ?? state.runBundle.run.externalRunId,
          status: status ?? state.runBundle.run.status,
        },
      }
    },
    appendDeploymentEvents: async (
      deployId,
      events,
      status,
      externalDeployId,
    ) => {
      if (
        state.deploymentBundle === undefined ||
        state.deploymentBundle.deployment.id !== deployId
      ) {
        return
      }

      state.deploymentBundle = {
        deployment: {
          ...state.deploymentBundle.deployment,
          externalDeployId:
            externalDeployId ??
            state.deploymentBundle.deployment.externalDeployId,
          status: status ?? state.deploymentBundle.deployment.status,
        },
        events: [...state.deploymentBundle.events, ...events],
      }
    },
    findAgentRunForUser: async (userId, runId) =>
      state.runBundle?.run.userId === userId && state.runBundle.run.id === runId
        ? state.runBundle
        : undefined,
    findDeploymentForUser: async (userId, deployId) =>
      state.deploymentBundle?.deployment.userId === userId &&
      state.deploymentBundle.deployment.id === deployId
        ? state.deploymentBundle
        : undefined,
    listAgentRunsForUser: async userId =>
      state.runBundle?.run.userId === userId ? [state.runBundle] : [],
    listDeploymentsForUser: async userId =>
      state.deploymentBundle?.deployment.userId === userId
        ? [state.deploymentBundle]
        : [],
    saveAgentRun: async (run, events) => {
      state.runBundle = { events, run }
    },
    saveDeployment: async (deployment, events) => {
      state.deploymentBundle = { deployment, events }
    },
  }
}

describe('Omni Effect services', () => {
  test('assignment service creates queued run and deployment Effects', async () => {
    const service = makeOmniAssignmentService()
    const queuedRun = await Effect.runPromise(
      service.createQueuedAgentRun({
        appOrigin: 'https://openagents.com',
        goal: 'Run tests.',
        repository,
        runId: 'agent_run_service_test',
        userId: 'github:1',
      }),
    )
    const queuedDeployment = await Effect.runPromise(
      service.createQueuedDeployment({
        appOrigin: 'https://openagents.com',
        repository,
        userId: 'github:1',
      }),
    )

    expect(queuedRun.run.id).toBe('agent_run_service_test')
    expect(queuedRun.events[0]?.parentId).toBe('agent_run_service_test')
    expect(queuedDeployment.deployment.status).toBe('queued')
  })

  test('run and deployment repositories expose store operations as Effects', async () => {
    const store = makeMemoryOmniRunStore()
    const runRepository = makeOmniRunRepository(store)
    const deploymentRepository = makeOmniDeploymentRepository(store)
    const runs = await Effect.runPromise(
      runRepository.listAgentRunsForUser('github:1', 10),
    )
    const deployments = await Effect.runPromise(
      deploymentRepository.listDeploymentsForUser('github:1', 10),
    )
    const run = runs[0]
    const deployment = deployments[0]

    expect(run?.run.status).toBe('queued')
    expect(deployment?.deployment.status).toBe('queued')

    if (run !== undefined) {
      await Effect.runPromise(
        runRepository.appendAgentRunEvents(
          run.run.id,
          run.events,
          'running',
          'shc:run',
        ),
      )
      const found = await Effect.runPromise(
        runRepository.findAgentRunForUser('github:1', run.run.id),
      )

      expect(found?.run.status).toBe('running')
      expect(found?.run.externalRunId).toBe('shc:run')
    }

    if (deployment !== undefined) {
      await Effect.runPromise(
        deploymentRepository.appendDeploymentEvents(
          deployment.deployment.id,
          deployment.events,
          'running',
          'shc:deploy',
        ),
      )
      const found = await Effect.runPromise(
        deploymentRepository.findDeploymentForUser(
          'github:1',
          deployment.deployment.id,
        ),
      )

      expect(found?.deployment.status).toBe('running')
      expect(found?.deployment.externalDeployId).toBe('shc:deploy')
    }
  })

  test('runner event service decodes callbacks with Schema and maps failures', async () => {
    const service = makeOmniRunnerEventService()
    const decoded = await Effect.runPromise(
      service.decodeCallbackEvent({
        createdAt: '2026-06-04T00:00:00.000Z',
        sequence: 2,
        source: 'runner',
        summary: 'Runner started.',
        type: 'runner.started',
      }),
    )
    const event = await Effect.runPromise(
      service.eventFromCallbackPayload('agent_run_test', 2, {
        createdAt: '2026-06-04T00:00:00.000Z',
        sequence: 2,
        source: 'runner',
        summary: 'Runner started.',
        type: 'runner.started',
      }),
    )

    expect(decoded.type).toBe('runner.started')
    expect(event.parentId).toBe('agent_run_test')
    expect(event.sequence).toBe(2)
    const shcDecoded = await Effect.runPromise(
      service.decodeCallbackEvent({
        artifact_refs: ['artifact_1'],
        emitted_at_ms: Date.parse('2026-06-04T00:00:01.000Z'),
        external_event_id: 'runner.event.1',
        schema_version: 'openagents.runner_event.v1',
        sequence: 3,
        source: 'codex',
        summary: 'stdout JSON event captured.',
        type: 'tool_use',
      }),
    )
    const shcEvent = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_test', 3, [
        {
          artifact_refs: ['artifact_1'],
          emitted_at_ms: Date.parse('2026-06-04T00:00:01.000Z'),
          external_event_id: 'runner.event.1',
          schema_version: 'openagents.runner_event.v1',
          sequence: 3,
          source: 'codex',
          summary: 'stdout JSON event captured.',
          type: 'tool_use',
        },
      ]),
    )

    expect(shcDecoded.createdAt).toBe('2026-06-04T00:00:01.000Z')
    expect(shcDecoded.externalEventId).toBe('runner.event.1')
    expect(shcEvent[0]?.artifactRefs).toEqual(['artifact_1'])
    expect(shcEvent[0]?.externalEventId).toBe('runner.event.1')
    const shcJobEvent = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_test', 4, [
        {
          dataJson: JSON.stringify({
            artifactRefs: [],
            createdAtMs: Date.parse('2026-06-04T00:00:02.000Z'),
            detail: 'Build completed',
            kind: 'tool_use',
            sequence: 4,
          }),
          digest: null,
          source: 'runner',
          summary: 'stdout JSON event captured.',
          type: 'runner.tool_use',
        },
      ]),
    )

    expect(shcJobEvent[0]?.sequence).toBe(4)
    expect(shcJobEvent[0]?.summary).toBe('stdout JSON event captured.')
    const shcQueuedEvent = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_test', 5, [
        {
          dataJson: JSON.stringify({
            externalRunId: 'shc-codex:oa-shc-katy-01:run_1',
            runnerId: 'oa-shc-katy-01',
          }),
          digest: null,
          source: 'control',
          summary: 'Codex run queued on SHC control daemon.',
          type: 'cloud.run.queued',
        },
      ]),
    )

    expect(shcQueuedEvent[0]?.sequence).toBe(5)
    expect(shcQueuedEvent[0]?.type).toBe('cloud.run.queued')
    const redactedEvent = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_test', 6, [
        {
          dataJson: JSON.stringify({
            output: '{"refresh_token":"secret"}',
            sequence: 6,
          }),
          source: 'runner',
          summary: 'Redacted runner output captured.',
          type: 'runner.redacted',
        },
      ]),
    )

    expect(redactedEvent[0]?.sequence).toBe(6)
    expect(redactedEvent[0]?.payloadJson).not.toContain('refresh_token')
    await expect(
      Effect.runPromise(service.decodeCallbackEvent('runner.started')),
    ).rejects.toThrow(OmniRunnerCallbackDecodeError)
  })

  test('runner event service identifies provider reconnect-required failures', async () => {
    const service = makeOmniRunnerEventService()
    const records = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_test', 2, [
        {
          createdAt: '2026-06-04T00:00:00.000Z',
          payload: {
            error: {
              data: {
                statusCode: 401,
              },
            },
          },
          sequence: 2,
          source: 'runner',
          status: 'failed',
          summary:
            'ChatGPT/Codex account token invalidated by OpenAI: token_invalidated.',
          type: 'runner.failed',
        },
      ]),
    )
    const event = records[0]

    expect(event).toBeDefined()
    if (event !== undefined) {
      const reason = await Effect.runPromise(
        service.providerReauthReason(event),
      )

      expect(reason).toBe(
        'ChatGPT/Codex account token was invalidated by OpenAI (token_invalidated, HTTP 401).',
      )
    }
  })

  test('runner event service accepts redacted Artanis bootstrap callback batches', async () => {
    const service = makeOmniRunnerEventService()
    const records = await Effect.runPromise(
      service.eventsFromCallbackPayloads('agent_run_artanis_bootstrap', 72, [
        {
          createdAt: '2026-06-07T21:37:00.000Z',
          payload: {
            settlementIntentRef:
              'settlement_intent.public.artanis.bootstrap.pylon_launch',
          },
          sequence: 72,
          source: 'artanis',
          status: 'running',
          summary: 'Artanis attached a redacted settlement intent.',
          type: 'artanis.settlement_intent.attached',
        },
        {
          createdAt: '2026-06-07T21:37:01.000Z',
          payload: {
            authGrantRef: 'provider_auth_grant.public.redacted',
          },
          sequence: 73,
          source: 'runner',
          status: 'running',
          summary: 'Runner resolved the provider auth grant.',
          type: 'runner.auth_grant_resolved',
        },
        {
          createdAt: '2026-06-07T21:37:02.000Z',
          payload: {
            failureClass: 'token_invalidated',
          },
          sequence: 74,
          source: 'runner',
          status: 'failed',
          summary: 'Runner failed with a redacted token_invalidated class.',
          type: 'runner.failed',
        },
        {
          createdAt: '2026-06-07T21:37:03.000Z',
          payload: {
            cleanupRef: 'cleanup.public.artanis.bootstrap',
          },
          sequence: 75,
          source: 'runner',
          status: 'failed',
          summary: 'Runner cleanup completed.',
          type: 'runner.cleanup',
        },
      ]),
    )

    expect(records.map(record => record.type)).toEqual([
      'artanis.settlement_intent.attached',
      'runner.auth_grant_resolved',
      'runner.failed',
      'runner.cleanup',
    ])
    expect(records.map(record => record.sequence)).toEqual([72, 73, 74, 75])
    expect(JSON.stringify(records)).not.toContain('refresh_token')
    expect(JSON.stringify(records)).not.toContain('OPENCODE_AUTH_CONTENT')
  })

  test('dispatch service exposes SHC dispatch with typed failures', async () => {
    const queued = makeQueuedAgentRun()
    const service = makeOmniDispatchService({
      dispatchAgentRunToShc: async (): Promise<DispatchResult> => ({
        externalId: 'shc:agent_run_test',
        mode: 'live',
        status: 'queued',
      }),
      dispatchDeploymentToShc: async () => {
        throw new Error('control plane unavailable')
      },
    })
    const result = await Effect.runPromise(
      service.dispatchAgentRun(queued.run.assignment, {
        controlApiBearerToken: 'secret',
        controlApiUrl: 'https://shc.example.test/v1/codex-runs',
        dispatchMode: 'live',
      }),
    )
    const queuedDeployment = makeQueuedDeployment()

    expect(result.externalId).toBe('shc:agent_run_test')
    await expect(
      Effect.runPromise(
        service.dispatchDeployment(queuedDeployment.deployment.assignment, {
          controlApiBearerToken: 'secret',
          controlApiUrl: 'https://shc.example.test/v1/codex-runs',
          dispatchMode: 'live',
        }),
      ),
    ).rejects.toThrow(OmniDispatchError)
  })

  test('dispatch service classifies SHC control failures by tag', async () => {
    const queued = makeQueuedAgentRun()
    const service = makeOmniDispatchService()
    const config = (fetcher: typeof fetch) => ({
      controlApiBearerToken: 'secret',
      controlApiUrl: 'https://shc.example.test/v1/codex-runs',
      dispatchMode: 'live',
      fetcher,
    })
    const unavailableFetcher: typeof fetch = async () =>
      new Response('not found', { status: 404 })
    const rejectedFetcher: typeof fetch = async () =>
      new Response('denied', { status: 503 })
    const malformedFetcher: typeof fetch = async () =>
      new Response('not-json', { status: 200 })
    const timeoutFetcher: typeof fetch = async () => {
      throw new DOMException('operation timed out', 'TimeoutError')
    }
    const transportFetcher: typeof fetch = async () => {
      throw new Error('network unreachable')
    }

    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(queued.run.assignment, {
          dispatchMode: 'fake',
        }),
      ),
    ).rejects.toThrow(OmniDispatchMissingCredentials)
    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(
          queued.run.assignment,
          config(unavailableFetcher),
        ),
      ),
    ).rejects.toThrow(OmniDispatchUnavailableEndpoint)
    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(
          queued.run.assignment,
          config(rejectedFetcher),
        ),
      ),
    ).rejects.toThrow(OmniDispatchRejectedRequest)
    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(
          queued.run.assignment,
          config(malformedFetcher),
        ),
      ),
    ).rejects.toThrow(OmniDispatchMalformedResponse)
    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(queued.run.assignment, config(timeoutFetcher)),
      ),
    ).rejects.toThrow(OmniDispatchTimeout)
    await expect(
      Effect.runPromise(
        service.dispatchAgentRun(
          queued.run.assignment,
          config(transportFetcher),
        ),
      ),
    ).rejects.toThrow(OmniDispatchTransportFailure)
  })

  test('run repository maps event persistence failures to typed storage errors', async () => {
    const queued = makeQueuedAgentRun()
    const failingStore: OmniRunStore = {
      ...makeMemoryOmniRunStore(),
      appendAgentRunEvents: async () => {
        throw new Error('d1 unavailable')
      },
    }
    const repository = makeOmniRunRepository(failingStore)

    await expect(
      Effect.runPromise(
        repository.appendAgentRunEvents(
          queued.run.id,
          queued.events,
          'running',
          'shc:run',
        ),
      ),
    ).rejects.toThrow(OmniRepositoryError)
  })

  test('operator service represents credit checks and debits as typed billing Effects', async () => {
    const queued = makeQueuedAgentRun()
    const service = makeOmniOperatorService({
      requireMinimumRunCredits: async (_db, _userId, _runtime) => ({
        billing: {
          activeRuns: [],
          autoTopUp: {
            events: [],
            policy: {
              amountCents: 2500,
              amountFormatted: '$25.00',
              enabled: false,
              monthlyCapCents: 10000,
              monthlyCapFormatted: '$100.00',
              pauseReason: null,
              spentThisMonthCents: 0,
              spentThisMonthFormatted: '$0.00',
              status: 'disabled',
              thresholdCents: 500,
              thresholdFormatted: '$5.00',
              updatedAt: '2026-06-11T00:00:00.000Z',
            },
            savedPaymentMethod: null,
          },
          balanceCents: 0,
          balanceFormatted: '$0.00',
          currency: 'USD',
          minimumRunCreditCents: 5,
          minimumRunCreditFormatted: '$0.05',
          packages: [],
          rates: {
            codexCentsPerThousandTokens: 2,
            containerCentsPerMinute: 5,
          },
          recentEntries: [],
          status: 'active',
        },
        message: 'Add credits before launching Autopilot.',
        ok: false,
      }),
      recordContainerUsageDebitForRun: async () => {
        throw new Error('ledger write failed')
      },
    })

    await expect(
      Effect.runPromise(service.requireRunCredits(unusedDatabase, 'github:1')),
    ).rejects.toThrow(OmniBillingError)
    await expect(
      Effect.runPromise(
        service.recordContainerUsageDebit(unusedDatabase, queued.run),
      ),
    ).rejects.toThrow(OmniBillingError)
  })

  test('public projection service redacts runner callback token refs', async () => {
    const queued = makeQueuedAgentRun()
    const service = makeOmniPublicProjectionService()
    const publicBundle = await Effect.runPromise(
      service.agentRunBundle({
        events: queued.events,
        run: queued.run,
      }),
    )

    expect(publicBundle.run.assignment.callback.tokenRef).toBe(
      'runner_callback_token',
    )
  })
})
