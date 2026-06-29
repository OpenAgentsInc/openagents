import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./predeploy-parallel-dispatch-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

describe('predeploy parallel dispatch smoke', () => {
  test('registers, heartbeats, and dispatches five dummy Codex tasks', async () => {
    const assignmentBodies: Array<Record<string, unknown>> = []
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )
        const headers = new Headers(init?.headers)
        expect(headers.get('authorization')).toBe('Bearer oa_agent_test')

        if (url.pathname === '/api/pylons/register') {
          const body = JSON.parse(String(init?.body)) as Record<string, unknown>
          expect(body.pylonRef).toBe('pylon.predeploy.test')
          expect(body.capabilityRefs).toEqual(['capability.pylon.local_codex'])
          return json({ pylon: { pylonRef: body.pylonRef } }, { status: 201 })
        }

        if (url.pathname === '/api/pylons/pylon.predeploy.test/heartbeat') {
          const body = JSON.parse(String(init?.body)) as {
            capacityRefs: ReadonlyArray<string>
          }
          expect(body.capacityRefs).toContain('capacity.coding.codex.ready=5')
          expect(body.capacityRefs).toContain(
            'capacity.coding.codex.account.640900000000000000000000.ready=1',
          )
          return json({ event: { eventKind: 'heartbeat' } }, { status: 201 })
        }

        if (url.pathname === '/api/operator/pylons/assignments') {
          const body = JSON.parse(String(init?.body)) as Record<string, unknown>
          assignmentBodies.push(body)
          return json(
            {
              assignment: {
                assignmentRef: body.assignmentRef,
              },
            },
            { status: 201 },
          )
        }

        return json({ error: 'unexpected' }, { status: 404 })
      },
    )

    const output = await smoke.runPredeployParallelDispatchSmoke({
      approveStagingMutation: true,
      baseUrl: 'https://staging.example',
      fetchImpl,
      parallelism: 5,
      pylonRef: 'pylon.predeploy.test',
      runRef: 'run_test',
      token: 'oa_agent_test',
    })

    expect(output.ok).toBe(true)
    expect(output.assignmentResults).toHaveLength(5)
    expect(fetchImpl).toHaveBeenCalledTimes(7)
    expect(assignmentBodies.map(body => body.assignmentRef)).toEqual([
      'assignment.public.issue6409.run_test.0',
      'assignment.public.issue6409.run_test.1',
      'assignment.public.issue6409.run_test.2',
      'assignment.public.issue6409.run_test.3',
      'assignment.public.issue6409.run_test.4',
    ])
    expect(
      assignmentBodies.map(body => {
        const codingAssignment = body.codingAssignment as {
          codex: { accountRefHash: string }
        }
        return codingAssignment.codex.accountRefHash
      }),
    ).toEqual([
      'account.pylon.codex.640900000000000000000000',
      'account.pylon.codex.640900000000000000000001',
      'account.pylon.codex.640900000000000000000002',
      'account.pylon.codex.640900000000000000000003',
      'account.pylon.codex.640900000000000000000004',
    ])
  })

  test('fails the gate when any concurrent task hits duplicate_active_assignment', async () => {
    const fetchImpl = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        const url = new URL(
          input instanceof Request ? input.url : String(input),
        )

        if (url.pathname === '/api/pylons/register') {
          return json({ pylon: { pylonRef: 'pylon.predeploy.test' } })
        }

        if (url.pathname === '/api/pylons/pylon.predeploy.test/heartbeat') {
          return json({ event: { eventKind: 'heartbeat' } })
        }

        if (url.pathname === '/api/operator/pylons/assignments') {
          const body = JSON.parse(String(init?.body)) as {
            assignmentRef: string
          }
          if (body.assignmentRef.endsWith('.3')) {
            return json(
              {
                dispatchGate: {
                  blockerRefs: [
                    'blocker.public.pylon_dispatch.duplicate_active_assignment',
                  ],
                },
                error: 'pylon_api_conflict',
              },
              { status: 409 },
            )
          }
          return json({ assignment: { assignmentRef: body.assignmentRef } })
        }

        return json({ error: 'unexpected' }, { status: 404 })
      },
    )

    await expect(
      smoke.runPredeployParallelDispatchSmoke({
        approveStagingMutation: true,
        fetchImpl,
        parallelism: 5,
        pylonRef: 'pylon.predeploy.test',
        runRef: 'run_test',
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('duplicate_active_assignment')
  })

  test('requires explicit staging mutation approval', async () => {
    await expect(
      smoke.runPredeployParallelDispatchSmoke({
        fetchImpl: vi.fn(),
        parallelism: 5,
        pylonRef: 'pylon.predeploy.test',
        runRef: 'run_test',
        token: 'oa_agent_test',
      }),
    ).rejects.toThrow('Refusing staging mutation smoke')
  })
})
