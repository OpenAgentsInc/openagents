import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  CLOUD_GCP_RUNTIME_DISPATCH_CLIENT_GROUP_ID,
  dispatchCloudGcpRuntimeTurn,
  runCloudGcpRuntimeDispatch,
  type CloudGcpAdmittedWorkContext,
  type CloudGcpMintFn,
  type CloudGcpPlacementLaunchFn,
  type CloudGcpRuntimeDispatchDependencies,
} from './khala-cloud-runtime-dispatch'
import { decodeWorkContextB64 } from './khala-cloud-runtime-inference-block'

// A no-op SQL (mint/revoke are faked, executePush is faked; the SQL handle is
// only passed through, never queried directly in these tests).
const noopSql = (() => Promise.resolve([])) as unknown as SyncSql

type RecordedEvent = {
  userId: string
  mutationId: number
  kind: string
  toolName: string | undefined
  finishReason: string | undefined
  clientId: string
}

const makeRecordingExecutePush = (
  behavior: (event: RecordedEvent) => 'applied' | 'rejected' = () => 'applied',
) => {
  const recorded: Array<RecordedEvent> = []
  const executePush = (input: {
    readonly userId: string
    readonly request: {
      readonly clientId: string
      readonly mutations: ReadonlyArray<{ mutationId: number; argsJson: string }>
    }
  }): Promise<PushResponse> => {
    const envelope = input.request.mutations[0]!
    const event = JSON.parse(envelope.argsJson) as {
      kind: string
      toolName?: string
      finishReason?: string
    }
    const rec: RecordedEvent = {
      clientId: input.request.clientId,
      finishReason: event.finishReason,
      kind: event.kind,
      mutationId: envelope.mutationId,
      toolName: event.toolName,
      userId: input.userId,
    }
    recorded.push(rec)
    return Promise.resolve({
      lastMutationId: envelope.mutationId,
      protocolVersion: 1,
      results: [{ mutationId: envelope.mutationId, status: behavior(rec) }],
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded }
}

let uuidN = 0
const detUuid = () => `uuid-${(uuidN += 1)}`

const mintCalls: Array<{ ownerUserId: string; ttlSeconds: number | undefined }> = []
const revokeCalls: Array<string> = []

const fakeMint =
  (raw = 'oa_agent_RAWTOKEN0123456789abcdef'): CloudGcpMintFn =>
  (_sql, input) => {
    mintCalls.push({ ownerUserId: input.ownerUserId, ttlSeconds: input.ttlSeconds })
    return Promise.resolve({
      createdAt: '2026-07-07T00:00:00.000Z',
      credentialId: `agentcred.seam-a.${mintCalls.length}`,
      expiresAt: '2026-07-07T00:10:00.000Z',
      ownerUserId: input.ownerUserId,
      rawToken: raw,
      tokenPrefix: raw.slice(0, 20),
    })
  }

const fakeRevoke = (_sql: SyncSql, input: { credentialId: string }) => {
  revokeCalls.push(input.credentialId)
  return Promise.resolve(1)
}

const admitted: CloudGcpAdmittedWorkContext = {
  commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
  eventCount: 0,
  objective: 'seam-a live turn',
  ownerUserId: 'github:14167547',
  repo: 'octocat/Hello-World',
  threadId: 'thread.t1',
  turnId: 'turn.t1',
  workContextRef: 'work-context.agent-computer.wc1',
}

const okLaunch =
  (captured: { b64?: string; repoBindingRef?: string | undefined } = {}): CloudGcpPlacementLaunchFn =>
  input => {
    captured.b64 = input.workContextB64
    captured.repoBindingRef = input.repoBindingRef
    return Promise.resolve({
      agentComputerState: 'provisioning',
      lifecycleReceiptRefs: ['receipt.cloud.gce.provisioning.1'],
      ok: true,
      placementRef: 'placement.cloud-coding.run_gce_1',
      sessionId: input.sessionId,
    })
  }

const baseDeps = (
  overrides: Partial<CloudGcpRuntimeDispatchDependencies> = {},
): CloudGcpRuntimeDispatchDependencies => ({
  armed: true,
  inference: {
    baseUrl: 'https://staging.example',
    model: 'openagents/khala',
    noMeterSecret: 'no-meter',
    provider: 'vertex-gemini',
    ttlSeconds: 600,
  },
  launch: okLaunch(),
  mint: fakeMint(),
  now: () => '2026-07-07T00:00:00.000Z',
  revoke: fakeRevoke,
  sql: noopSql,
  uuid: detUuid,
  ...overrides,
})

const reset = () => {
  uuidN = 0
  mintCalls.length = 0
  revokeCalls.length = 0
}

describe('dispatchCloudGcpRuntimeTurn', () => {
  test('happy path: claim -> mint -> launch(work_context_b64) -> status -> finished(stop); token NOT revoked', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const captured: { b64?: string; repoBindingRef?: string | undefined } = {}
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch: okLaunch(captured) }),
      admitted,
    )
    expect(result.outcome).toBe('launched')
    expect(result.placementRef).toBe('placement.cloud-coding.run_gce_1')
    // token kept alive for the async guest inference call (TTL-bounded).
    expect(result.tokenRevoked).toBe(false)
    expect(revokeCalls).toHaveLength(0)
    expect(result.credentialId).toBe('agentcred.seam-a.1')

    // event stream shape + owner attribution.
    expect(push.recorded.map(r => r.kind)).toEqual([
      'turn.started',
      'text.delta',
      'text.completed',
      'turn.finished',
    ])
    expect(push.recorded.map(r => r.mutationId)).toEqual([1, 2, 3, 4])
    expect(push.recorded.every(r => r.userId === 'github:14167547')).toBe(true)
    expect(push.recorded.at(-1)?.finishReason).toBe('stop')
    const clientIds = new Set(push.recorded.map(r => r.clientId))
    expect(clientIds.size).toBe(1)
    expect([...clientIds][0]).toContain(CLOUD_GCP_RUNTIME_DISPATCH_CLIENT_GROUP_ID)

    // mint linked to owner, TTL forwarded.
    expect(mintCalls).toEqual([{ ownerUserId: 'github:14167547', ttlSeconds: 600 }])

    // the forwarded blob decodes to a work-context carrying the minted bearer
    // + single-charge no-meter secret + owner attribution.
    expect(captured.b64).toBeDefined()
    const wc = decodeWorkContextB64(captured.b64!)
    expect(wc.repo).toBe('octocat/Hello-World')
    expect(wc.inference.agentToken).toBe('oa_agent_RAWTOKEN0123456789abcdef')
    expect(wc.inference.ownerUserId).toBe('github:14167547')
    expect(wc.inference.noMeterSecret).toBe('no-meter')
    expect(wc.inference.provider).toBe('vertex-gemini')
  })

  test('forwards a repo binding ref to placement when present', async () => {
    reset()
    const captured: { b64?: string; repoBindingRef?: string | undefined } = {}
    await dispatchCloudGcpRuntimeTurn(
      baseDeps({
        executePush: makeRecordingExecutePush().executePush,
        launch: okLaunch(captured),
      }),
      { ...admitted, repoBindingRef: 'repo-binding.mobile.thread-1' },
    )
    expect(captured.repoBindingRef).toBe('repo-binding.mobile.thread-1')
  })

  test('launch refused: finished(error) and token IS revoked', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const refuse: CloudGcpPlacementLaunchFn = () =>
      Promise.resolve({ ok: false, reason: 'cloud_placement_http_503' })
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch: refuse }),
      admitted,
    )
    expect(result.outcome).toBe('failed')
    expect(result.reason).toBe('cloud_placement_http_503')
    expect(result.tokenRevoked).toBe(true)
    expect(revokeCalls).toEqual(['agentcred.seam-a.1'])
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started', 'turn.finished'])
    expect(push.recorded.at(-1)?.finishReason).toBe('error')
  })

  test('a thrown launch revokes the token and never leaks it', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const boom: CloudGcpPlacementLaunchFn = () => {
      throw new Error('network exploded')
    }
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch: boom }),
      admitted,
    )
    expect(result.outcome).toBe('failed')
    expect(result.tokenRevoked).toBe(true)
    expect(revokeCalls).toEqual(['agentcred.seam-a.1'])
  })

  test('lost claim race skips: no mint, no launch, no revoke', async () => {
    reset()
    const push = makeRecordingExecutePush(e =>
      e.kind === 'turn.started' ? 'rejected' : 'applied',
    )
    let launched = false
    const launch: CloudGcpPlacementLaunchFn = input => {
      launched = true
      return okLaunch()(input)
    }
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch }),
      admitted,
    )
    expect(result.outcome).toBe('skipped')
    expect(launched).toBe(false)
    expect(mintCalls).toHaveLength(0)
    expect(revokeCalls).toHaveLength(0)
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started'])
  })
})

describe('runCloudGcpRuntimeDispatch', () => {
  test('FAIL-CLOSED when not armed: no read, no mint, no launch', async () => {
    reset()
    let read = false
    let launched = false
    const summary = await runCloudGcpRuntimeDispatch(
      baseDeps({
        armed: false,
        launch: input => {
          launched = true
          return okLaunch()(input)
        },
        readAdmitted: () => {
          read = true
          return Promise.resolve([admitted])
        },
      }),
    )
    expect(summary).toEqual({ failed: 0, launched: 0, scanned: 0, skipped: 0 })
    expect(read).toBe(false)
    expect(launched).toBe(false)
    expect(mintCalls).toHaveLength(0)
  })

  test('armed: reads admitted work-contexts and tallies a batch', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const summary = await runCloudGcpRuntimeDispatch(
      baseDeps({
        executePush: push.executePush,
        readAdmitted: () =>
          Promise.resolve([
            admitted,
            { ...admitted, turnId: 'turn.t2', workContextRef: 'work-context.wc2' },
          ]),
      }),
    )
    expect(summary).toEqual({ failed: 0, launched: 2, scanned: 2, skipped: 0 })
  })

  test('armed but no reader configured is a clean no-op', async () => {
    reset()
    const summary = await runCloudGcpRuntimeDispatch(baseDeps())
    expect(summary).toEqual({ failed: 0, launched: 0, scanned: 0, skipped: 0 })
  })
})
