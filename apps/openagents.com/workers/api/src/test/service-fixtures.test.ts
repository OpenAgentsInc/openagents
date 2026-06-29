import { IsoTimestamp, OmniRunnerEvent } from '@openagentsinc/sync-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeMemorySyncD1 } from '../../../../packages/sync-worker/src/test-fixtures'
import { OpenAgentsDatabase } from '../bindings'
import { OpenAgentsWorkerConfig } from '../config'
import { createQueuedAgentRun, parseGithubRepository } from '../omni-runs'
import { OmniDispatchService } from '../omni/dispatch-service'
import { RunnerEventQueueMessage, RunnerEventsQueue } from '../runtime'
import {
  makeOmniDispatchServiceTestLayer,
  makeOpenAgentsDatabaseTestLayer,
  makeOpenAgentsWorkerConfigTestLayer,
  makeRunnerEventsQueueTestLayer,
} from './service-fixtures'

const timestamp = IsoTimestamp.make('2026-06-04T00:00:00.000Z')

describe('service test fixtures', () => {
  test('provides decoded Worker config with plain Vitest', async () => {
    const config = await Effect.runPromise(
      OpenAgentsWorkerConfig.pipe(
        Effect.provide(makeOpenAgentsWorkerConfigTestLayer()),
      ),
    )

    expect(config.app.origin).toBe('https://openagents.com')
    expect(config.github.clientId).toBe('github-client')
  })

  test('provides fake D1 binding layers', async () => {
    const db = makeMemorySyncD1()
    const provided = await Effect.runPromise(
      OpenAgentsDatabase.pipe(
        Effect.provide(makeOpenAgentsDatabaseTestLayer(db)),
      ),
    )

    expect(provided).toBe(db)
  })

  test('provides fake SHC dispatch service layers', async () => {
    const fixture = makeOmniDispatchServiceTestLayer()
    const queued = createQueuedAgentRun({
      appOrigin: 'https://openagents.com',
      goal: 'Run fixture smoke.',
      repository: parseGithubRepository('OpenAgentsInc/autopilot-omega'),
      userId: 'github:1',
    })
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* OmniDispatchService

        return yield* service.dispatchAgentRun(queued.run.assignment, {
          dispatchMode: 'live',
        })
      }).pipe(Effect.provide(fixture.layer)),
    )

    expect(result.status).toBe('queued')
    expect(fixture.calls[0]?.kind).toBe('agent-run')
  })

  test('provides fake queue producer layers', async () => {
    const fixture = makeRunnerEventsQueueTestLayer()

    await Effect.runPromise(
      RunnerEventsQueue.send(
        new RunnerEventQueueMessage({
          events: [
            new OmniRunnerEvent({
              createdAt: timestamp,
              payload: { ok: true },
              sequence: 1,
              source: 'runner',
              summary: 'runner started',
              type: 'runner.log',
            }),
          ],
          parentId: 'agent_run_1',
          receivedAt: timestamp,
          schemaVersion: 'openagents.runner_event_queue.v1',
        }),
      ).pipe(Effect.provide(fixture.layer)),
    )

    expect(fixture.sentBodies).toEqual([
      {
        events: [
          {
            createdAt: timestamp,
            payload: { ok: true },
            sequence: 1,
            source: 'runner',
            summary: 'runner started',
            type: 'runner.log',
          },
        ],
        parentId: 'agent_run_1',
        receivedAt: timestamp,
        schemaVersion: 'openagents.runner_event_queue.v1',
      },
    ])
  })
})
