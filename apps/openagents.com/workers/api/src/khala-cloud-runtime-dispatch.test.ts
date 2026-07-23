import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  CloudCodingAdapterError,
  type CloudCodingRuntimeAdapter,
  type CloudCodingSession,
} from './cloud/cloud-coding-session-routes'
import {
  CLOUD_GCP_RUNTIME_DISPATCH_CLIENT_GROUP_ID,
  applySarahManagedCloudHarnessFallback,
  dispatchCloudGcpRuntimeTurn,
  finalizeManagedCloudProviderLease,
  hasAvailableSarahManagedCloudProviderCapacity,
  hasSarahManagedCloudProviderCapacity,
  makeCloudCodingAdapterLaunchSeam,
  managedAgentComputerGrantIssueInput,
  planCloudGcpRuntimeAccountDispatch,
  providerFailoverFailureClassForGuestFailure,
  readQueuedManagedCloudTurns,
  reconcileTerminalManagedCloudProviderLeases,
  recoverStaleRunningManagedCloudTurns,
  runCloudGcpRuntimeDispatch,
  resolveManagedCloudRepositoryCommit,
  type CloudGcpAdmittedWorkContext,
  type CloudGcpMintFn,
  type CloudGcpPlacementLaunchFn,
  type CloudGcpRuntimeDispatchDependencies,
} from './khala-cloud-runtime-dispatch'
import { decodeWorkContextB64 } from './khala-cloud-runtime-inference-block'
import type { ProviderAccountLeaseService } from './provider-account-lease-service'

// A no-op SQL (mint/revoke are faked, executePush is faked; the SQL handle is
// only passed through, never queried directly in these tests).
const noopSql = (() => Promise.resolve([])) as unknown as SyncSql

describe('managed cloud repository commit resolution', () => {
  test('preserves SHAs and resolves branch refs to immutable GitHub commits', async () => {
    const sha = 'a'.repeat(40)
    expect(await resolveManagedCloudRepositoryCommit('OpenAgentsInc/openagents', sha)).toBe(sha)
    const seen: string[] = []
    const resolved = await resolveManagedCloudRepositoryCommit(
      'OpenAgentsInc/openagents',
      'main',
      (async input => {
        seen.push(String(input))
        return new Response(JSON.stringify({ sha: 'b'.repeat(40) }))
      }) as typeof fetch,
    )
    expect(resolved).toBe('b'.repeat(40))
    expect(seen[0]).toContain('/commits/main')
  })

  test('fails closed on malformed or unavailable refs', async () => {
    expect(await resolveManagedCloudRepositoryCommit('invalid', 'main')).toBeNull()
    expect(await resolveManagedCloudRepositoryCommit(
      'OpenAgentsInc/openagents', 'missing',
      (async () => new Response(null, { status: 404 })) as typeof fetch,
    )).toBeNull()
  })
})

describe('managed cloud queued turn projection', () => {
  test('preserves an immutable Sarah dispatch commit and uses branch-only writeback', async () => {
    const commit = 'c'.repeat(40)
    const sql = (() =>
      Promise.resolve([
        {
          event_count: 0,
          goal_body: 'Repair issue #9191.',
          harness_execution_target_id: 'harness.opencode',
          owner_user_id: 'owner.fixture',
          repository_name: 'openagents',
          repository_owner: 'OpenAgentsInc',
          repository_ref: commit,
          thread_id: 'thread.sarah_cloud.fixture',
          turn_id: 'turn.sarah_cloud.fixture',
          work_context_ref: 'work_context.thread.sarah_cloud.fixture',
        },
      ])) as unknown as SyncSql

    const turns = await readQueuedManagedCloudTurns(sql, 1)

    expect(turns).toEqual([
      expect.objectContaining({
        commit,
        harnessId: 'opencode',
        repo: 'OpenAgentsInc/openagents',
        writeback: { mode: 'branch_only' },
      }),
    ])
    expect(turns[0]).not.toHaveProperty('branch')
  })
})

describe('Sarah managed cloud harness fallback', () => {
  const turn: CloudGcpAdmittedWorkContext = {
    commit: 'c'.repeat(40),
    eventCount: 0,
    ownerUserId: 'owner.fixture',
    repo: 'OpenAgentsInc/openagents',
    threadId: 'thread.sarah_cloud.fixture',
    turnId: 'turn.sarah_cloud.fixture',
    workContextRef: 'work_context.thread.sarah_cloud.fixture',
  }
  const account = (
    provider: 'chatgpt_codex' | 'google_gemini',
    overrides: Partial<{
      health: string
      publicStatus: string
      hasSecretRef: boolean
    }> = {},
  ) => ({
    health: overrides.health ?? 'healthy',
    hasSecretRef: overrides.hasSecretRef ?? true,
    provider,
    publicStatus: overrides.publicStatus ?? 'connected',
  })

  test('keeps Codex primary when both providers are eligible', () => {
    expect(
      applySarahManagedCloudHarnessFallback(turn, [
        account('chatgpt_codex'),
        account('google_gemini'),
      ]),
    ).toBe(turn)
  })

  test('admits either the Codex primary or Gemini fallback as owner capacity', () => {
    expect(
      hasSarahManagedCloudProviderCapacity([account('chatgpt_codex')]),
    ).toBe(true)
    expect(
      hasSarahManagedCloudProviderCapacity([account('google_gemini')]),
    ).toBe(true)
    expect(
      hasSarahManagedCloudProviderCapacity([
        account('chatgpt_codex', { health: 'requires_reauth' }),
        account('google_gemini', { hasSecretRef: false }),
      ]),
    ).toBe(false)
  })

  test('preflight excludes provider accounts with live leases', () => {
    const codex = {
      ...account('chatgpt_codex'),
      providerAccountRef: 'provider-account.codex.owner',
    }
    const gemini = {
      ...account('google_gemini'),
      providerAccountRef: 'provider-account.gemini.owner',
    }
    expect(
      hasAvailableSarahManagedCloudProviderCapacity(
        [codex, gemini],
        new Set([codex.providerAccountRef]),
      ),
    ).toBe(true)
    expect(
      hasAvailableSarahManagedCloudProviderCapacity(
        [codex, gemini],
        new Set([codex.providerAccountRef, gemini.providerAccountRef]),
      ),
    ).toBe(false)
  })

  test('selects OpenCode before claim when Codex is dead and Gemini is eligible', () => {
    expect(
      applySarahManagedCloudHarnessFallback(turn, [
        account('chatgpt_codex', { health: 'requires_reauth' }),
        account('google_gemini'),
      ]),
    ).toEqual({ ...turn, harnessId: 'opencode' })
  })

  test('does not change non-Sarah turns, explicit harnesses, or no-fallback state', () => {
    const gemini = account('google_gemini')
    expect(
      applySarahManagedCloudHarnessFallback(
        { ...turn, turnId: 'turn.user.fixture' },
        [gemini],
      ),
    ).toEqual({ ...turn, turnId: 'turn.user.fixture' })
    expect(
      applySarahManagedCloudHarnessFallback(
        { ...turn, harnessId: 'claude-code' },
        [gemini],
      ),
    ).toEqual({ ...turn, harnessId: 'claude-code' })
    expect(
      applySarahManagedCloudHarnessFallback(turn, [
        account('google_gemini', { hasSecretRef: false }),
      ]),
    ).toBe(turn)
  })
})

type RecordedEvent = {
  userId: string
  mutationId: number
  kind: string
  toolName: string | undefined
  finishReason: string | undefined
  resultRef: string | undefined
  reasonRef: string | undefined
  sequence: number
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
      resultRef?: string
      reasonRef?: string
      sequence: number
    }
    const rec: RecordedEvent = {
      clientId: input.request.clientId,
      finishReason: event.finishReason,
      kind: event.kind,
      mutationId: envelope.mutationId,
      resultRef: event.resultRef,
      reasonRef: event.reasonRef,
      sequence: event.sequence,
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

const withPinnedAccount = (
  turn: CloudGcpAdmittedWorkContext,
  accountRefHash: string,
): CloudGcpAdmittedWorkContext => ({
  ...turn,
  codexContinuity: {
    accountRefHash,
    authGrantRef: `grant.${accountRefHash}`,
    providerAccountRef: `provider.${accountRefHash}`,
  },
})

const okLaunch =
  (captured: {
    b64?: string
    repoBindingRef?: string | undefined
    authGrantRef?: string | undefined
    providerAccountRef?: string | undefined
  } = {}): CloudGcpPlacementLaunchFn =>
  input => {
    captured.authGrantRef = input.authGrantRef
    captured.b64 = input.workContextB64
    captured.providerAccountRef = input.providerAccountRef
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
    pylonRef: 'pylon.agent-computer.fixture',
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

describe('managed cloud interrupted-turn recovery', () => {
  test('settles only post-lease stale running turns without replaying work', async () => {
    reset()
    const queryValues: unknown[] = []
    const sql = ((_strings: TemplateStringsArray, ...values: unknown[]) => {
      queryValues.push(...values)
      return Promise.resolve([
        {
          event_count: 1,
          owner_user_id: 'owner.fixture',
          thread_id: 'thread.fixture',
          turn_id: 'turn.sarah_cloud.fixture',
        },
      ])
    }) as unknown as SyncSql
    const push = makeRecordingExecutePush()

    const recovered = await recoverStaleRunningManagedCloudTurns(
      baseDeps({
        executePush: push.executePush,
        now: () => '2026-07-23T09:15:00.000Z',
        sql,
      }),
    )

    expect(recovered).toBe(1)
    expect(queryValues).toContain('2026-07-23T08:35:00.000Z')
    expect(push.recorded).toEqual([
      expect.objectContaining({
        kind: 'turn.interrupted',
        reasonRef: 'managed_cloud_worker_generation_lost',
        sequence: 1,
        userId: 'owner.fixture',
      }),
    ])
    expect(mintCalls).toHaveLength(0)
    expect(revokeCalls).toHaveLength(0)
  })
})

describe('managed cloud terminal provider failover', () => {
  test.each([
    ['auth_rejected', 'token_invalidated'],
    ['account_exhausted', 'quota_exhausted'],
    ['account_rate_limited', 'rate_limited'],
    ['network_failed', 'provider_outage'],
    ['model_unavailable', 'provider_outage'],
    ['exec_timeout', 'launch_timeout'],
    ['exec_failed', 'runner_failure'],
  ] as const)('maps %s to %s', (guestFailureClass, providerFailureClass) => {
    expect(
      providerFailoverFailureClassForGuestFailure(guestFailureClass),
    ).toBe(providerFailureClass)
  })

  test('records one terminal failover receipt instead of generically releasing the lease', async () => {
    const failovers: Array<Parameters<ProviderAccountLeaseService['failover']>[0]> = []
    const releases: Array<Parameters<ProviderAccountLeaseService['release']>[0]> = []
    const service: Pick<ProviderAccountLeaseService, 'failover' | 'release'> = {
      failover: input => {
        failovers.push(input)
        return Promise.resolve(undefined)
      },
      release: input => {
        releases.push(input)
        return Promise.resolve(true)
      },
    }

    await finalizeManagedCloudProviderLease(
      service,
      {
        ...admitted,
        providerAccountLeaseRef: 'provider-account-lease.terminal-1',
      },
      {
        outcome: 'failed',
        providerFailureClass: 'token_invalidated',
        reason: 'agent_computer_guest_auth_rejected',
        tokenRevoked: true,
      },
      '2026-07-23T12:00:00.000Z',
    )

    expect(releases).toEqual([])
    expect(failovers).toEqual([
      {
        assignmentId: admitted.turnId,
        attemptNumber: 1,
        expiresAt: '2026-07-23T12:35:00.000Z',
        failureClass: 'token_invalidated',
        maxAttempts: 1,
        now: '2026-07-23T12:00:00.000Z',
        orderId: null,
        previousLeaseRef: 'provider-account-lease.terminal-1',
        requestedAction: 'agent_computer_codex_turn',
        runId: admitted.threadId,
        selectedByActor: 'sarah_managed_cloud_dispatch',
        source: 'managed_cloud_runtime_terminal_failover',
        userId: admitted.ownerUserId,
      },
    ])
  })

  test('keeps the existing release path for an ordinary placement failure', async () => {
    const failovers: Array<Parameters<ProviderAccountLeaseService['failover']>[0]> = []
    const releases: Array<Parameters<ProviderAccountLeaseService['release']>[0]> = []
    const service: Pick<ProviderAccountLeaseService, 'failover' | 'release'> = {
      failover: input => {
        failovers.push(input)
        return Promise.resolve(undefined)
      },
      release: input => {
        releases.push(input)
        return Promise.resolve(true)
      },
    }

    await finalizeManagedCloudProviderLease(
      service,
      {
        ...admitted,
        providerAccountLeaseRef: 'provider-account-lease.ordinary-1',
      },
      {
        outcome: 'failed',
        reason: 'capacity_unavailable',
        tokenRevoked: true,
      },
      '2026-07-23T12:00:00.000Z',
    )

    expect(failovers).toEqual([])
    expect(releases).toEqual([
      {
        failureClass: 'capacity_unavailable',
        leaseRef: 'provider-account-lease.ordinary-1',
        now: '2026-07-23T12:00:00.000Z',
        status: 'failed',
        terminalOutcome: 'managed_cloud_capacity_unavailable',
        userId: admitted.ownerUserId,
      },
    ])
  })

  test('keeps the provider lease active after asynchronous placement acceptance', async () => {
    const releases: Array<
      Parameters<ProviderAccountLeaseService['release']>[0]
    > = []
    await finalizeManagedCloudProviderLease(
      {
        failover: () => Promise.resolve(undefined),
        release: input => {
          releases.push(input)
          return Promise.resolve(true)
        },
      },
      {
        ...admitted,
        providerAccountLeaseRef: 'provider-account-lease.async-1',
      },
      {
        outcome: 'launched',
        placementRef: 'placement.async-1',
        sessionId: 'ccs.async-1',
        tokenRevoked: false,
      },
      '2026-07-23T12:00:00.000Z',
    )
    expect(releases).toEqual([])
  })
})

describe('managed cloud terminal provider lease reconciliation', () => {
  test('requires a writeback receipt before it selects a completed turn', async () => {
    let query = ''
    const sql = ((strings: TemplateStringsArray) => {
      query = strings.join('?')
      return Promise.resolve([])
    }) as unknown as SyncSql

    await reconcileTerminalManagedCloudProviderLeases(
      sql,
      { release: () => Promise.resolve(true) },
      '2026-07-23T12:00:00.000Z',
      2,
    )

    expect(query).toContain("t.status = 'completed'")
    expect(query).toContain("e.kind = 'writeback.recorded'")
  })

  test('releases completed turns as succeeded and failed turns as failed', async () => {
    const sql = (() =>
      Promise.resolve([
        {
          lease_ref: 'provider-account-lease.completed-1',
          owner_user_id: 'owner.fixture',
          status: 'completed',
          turn_id: 'turn.completed-1',
        },
        {
          lease_ref: 'provider-account-lease.failed-1',
          owner_user_id: 'owner.fixture',
          status: 'failed',
          turn_id: 'turn.failed-1',
        },
      ])) as unknown as SyncSql
    const releases: Array<
      Parameters<ProviderAccountLeaseService['release']>[0]
    > = []

    const released = await reconcileTerminalManagedCloudProviderLeases(
      sql,
      {
        release: input => {
          releases.push(input)
          return Promise.resolve(true)
        },
      },
      '2026-07-23T12:00:00.000Z',
      2,
    )

    expect(released).toBe(2)
    expect(releases).toEqual([
      {
        failureClass: null,
        leaseRef: 'provider-account-lease.completed-1',
        now: '2026-07-23T12:00:00.000Z',
        status: 'succeeded',
        terminalOutcome: 'managed_cloud_turn_completed',
        userId: 'owner.fixture',
      },
      {
        failureClass: 'managed_cloud_turn_failed',
        leaseRef: 'provider-account-lease.failed-1',
        now: '2026-07-23T12:00:00.000Z',
        status: 'failed',
        terminalOutcome: 'managed_cloud_turn_failed',
        userId: 'owner.fixture',
      },
    ])
  })
})

describe('dispatchCloudGcpRuntimeTurn', () => {
  test('derives an owner-scoped provider grant request from the claimed turn', () => {
    const selection = {
      _tag: 'gemini',
      harnessId: 'pi',
      model: 'gemini-3.5-flash',
      provider: 'google_gemini',
      requestedAction: 'agent_computer_gemini_turn',
    } as const

    expect(
      managedAgentComputerGrantIssueInput(
        admitted,
        selection,
        'provider-account.gemini.owner-1',
      ),
    ).toEqual({
      providerAccountRef: 'provider-account.gemini.owner-1',
      requestedAction: 'agent_computer_gemini_turn',
      runnerSessionId: admitted.turnId,
      threadId: admitted.threadId,
      userId: admitted.ownerUserId,
      workroomId: admitted.workContextRef,
    })
  })

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
    expect(wc.inference).toBeDefined()
    expect(wc.inference!.agentToken).toBe('oa_agent_RAWTOKEN0123456789abcdef')
    expect(wc.inference!.ownerUserId).toBe('github:14167547')
    expect(wc.inference!.noMeterSecret).toBe('no-meter')
    expect(wc.inference!.provider).toBe('vertex-gemini')
  })

  test.each(['goose', 'opencode', 'pi'] as const)(
    '%s receives only its exact Gemini grant and preserves execution/writeback identity',
    async harnessId => {
      reset()
      const captured: {
        authGrantRef?: string
        b64?: string
        providerAccountRef?: string
      } = {}
      const providerSecretBytes = 'provider-secret-bytes-must-not-leave-custody'
      const result = await dispatchCloudGcpRuntimeTurn(
        baseDeps({
          executePush: makeRecordingExecutePush().executePush,
          launch: okLaunch(captured),
          prepareAfterClaim: turn =>
            Promise.resolve({
              ...turn,
              harnessId,
              harnessRuntimeSecretGrant: {
                grantRef: `provider-runtime-secret-grant.${harnessId}.1`,
                kind: 'gemini_api_key',
                providerAccountRef: 'provider-account.gemini.owner-1',
                runnerSessionId: turn.turnId,
                secretRef: 'provider-secret.gemini.owner-1',
              },
              writeback: { baseBranch: 'main', mode: 'branch_only' },
            }),
        }),
        { ...admitted, harnessId },
      )

      expect(result.outcome).toBe('launched')
      expect(captured.authGrantRef).toBe(
        `provider-runtime-secret-grant.${harnessId}.1`,
      )
      expect(captured.providerAccountRef).toBe(
        'provider-account.gemini.owner-1',
      )
      const workContext = decodeWorkContextB64(captured.b64!)
      expect(workContext.inference).toBeUndefined()
      expect(workContext.codexTurn).toBeUndefined()
      expect(workContext.providerAuth).toBeUndefined()
      expect(workContext.harnessTurn).toEqual({
        harness: harnessId,
        model: 'gemini-3.5-flash',
        runtimeSecretGrant: {
          agentToken: 'oa_agent_RAWTOKEN0123456789abcdef',
          baseUrl: 'https://staging.example',
          grantRef: `provider-runtime-secret-grant.${harnessId}.1`,
          kind: 'gemini_api_key',
          ownerUserId: admitted.ownerUserId,
          pylonRef: 'pylon.agent-computer.fixture',
          providerAccountRef: 'provider-account.gemini.owner-1',
          runnerSessionId: admitted.turnId,
          secretRef: 'provider-secret.gemini.owner-1',
        },
      })
      expect(workContext).toMatchObject({
        commit: admitted.commit,
        repo: admitted.repo,
        threadRef: admitted.threadId,
        turnId: admitted.turnId,
        verificationCommand: {
          argv: ['git', 'diff', '--cached', '--check'],
          commandRef: 'verify.agent-computer.git_diff_cached_check',
          timeoutSeconds: 120,
        },
        workContextRef: admitted.workContextRef,
        writeback: {
          baseBranch: 'main',
          mode: 'branch_only',
          repositoryFullName: admitted.repo,
        },
      })
      expect(JSON.stringify(workContext)).not.toContain(providerSecretBytes)
    },
  )

  test('Claude receives the exact auth block without Codex or generic inference fallback', async () => {
    reset()
    const captured: { b64?: string } = {}
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({
        executePush: makeRecordingExecutePush().executePush,
        launch: okLaunch(captured),
        prepareAfterClaim: turn =>
          Promise.resolve({
            ...turn,
            claudeProviderAuthGrant: {
              authGrantRef: 'provider-auth-grant.claude.owner-1',
              kind: 'claude_agent_anthropic_api_key',
              providerAccountRef: 'provider-account.claude.owner-1',
              runnerSessionId: turn.turnId,
              secretRef: 'provider-secret.claude.owner-1',
            },
            harnessId: 'claude-code',
          }),
      }),
      { ...admitted, harnessId: 'claude-code' },
    )

    expect(result.outcome).toBe('launched')
    const workContext = decodeWorkContextB64(captured.b64!)
    expect(workContext.harnessTurn).toEqual({ harness: 'claude-code' })
    expect(workContext.claudeProviderAuth).toEqual({
      agentToken: 'oa_agent_RAWTOKEN0123456789abcdef',
      authGrantRef: 'provider-auth-grant.claude.owner-1',
      baseUrl: 'https://staging.example',
      kind: 'claude_agent_anthropic_api_key',
      providerAccountRef: 'provider-account.claude.owner-1',
      runnerSessionId: 'turn.t1',
      secretRef: 'provider-secret.claude.owner-1',
    })
    expect(workContext.inference).toBeUndefined()
    expect(workContext.codexTurn).toBeUndefined()
    expect(workContext.providerAuth).toBeUndefined()
  })

  test.each([
    ['cursor', 'cursor_api_key'],
    ['grok', 'xai_api_key'],
  ] as const)(
    '%s launches with an exact runtime-secret grant',
    async (harnessId, kind) => {
      reset()
      const captured: { b64?: string } = {}
      const push = makeRecordingExecutePush()
      const result = await dispatchCloudGcpRuntimeTurn(
        baseDeps({
          executePush: push.executePush,
          launch: okLaunch(captured),
          prepareAfterClaim: turn =>
            Promise.resolve({
              ...turn,
              harnessRuntimeSecretGrant: {
                grantRef: `grant.${harnessId}.1`,
                kind,
                providerAccountRef: `provider-account.${harnessId}.1`,
                runnerSessionId: turn.turnId,
                secretRef: `worker-secret.${harnessId}.1`,
              },
            }),
        }),
        { ...admitted, harnessId },
      )

      expect(result.outcome).toBe('launched')
      expect(decodeWorkContextB64(captured.b64!).harnessTurn).toMatchObject({
        harness: harnessId,
        runtimeSecretGrant: { kind },
      })
    },
  )

  test('a mismatched Gemini runner session fails closed and revokes the execution token', async () => {
    reset()
    let launched = false
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({
        executePush: makeRecordingExecutePush().executePush,
        launch: input => {
          launched = true
          return okLaunch()(input)
        },
        prepareAfterClaim: turn =>
          Promise.resolve({
            ...turn,
            harnessId: 'pi',
            harnessRuntimeSecretGrant: {
              grantRef: 'provider-runtime-secret-grant.pi.1',
              kind: 'gemini_api_key',
              providerAccountRef: 'provider-account.gemini.owner-1',
              runnerSessionId: 'turn.other-owner-session',
              secretRef: 'provider-secret.gemini.owner-1',
            },
          }),
      }),
      { ...admitted, harnessId: 'pi' },
    )

    expect(result).toMatchObject({
      outcome: 'failed',
      reason: 'agent_computer_gemini_grant_unavailable',
      tokenRevoked: true,
    })
    expect(launched).toBe(false)
    expect(revokeCalls).toEqual(['agentcred.seam-a.1'])
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

  test('reclaim resume: work-context re-primes pinned Codex account and emits continuity event', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const captured: { b64?: string; repoBindingRef?: string | undefined } = {}
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch: okLaunch(captured) }),
      {
        ...admitted,
        codexContinuity: {
          accountRefHash: 'acct_hash_1',
          authGrantRef: 'grant.codex.thread_1',
          maxReplayMessages: 12,
          previousTurnCount: 3,
          providerAccountRef: 'provider-account.codex.owner_1',
        },
        eventCount: 4,
      },
    )
    expect(result.outcome).toBe('launched')
    expect(push.recorded.map(r => r.kind)).toEqual([
      'turn.started',
      'tool.result',
      'text.delta',
      'text.completed',
      'turn.finished',
    ])
    expect(push.recorded.map(r => r.mutationId)).toEqual([1, 2, 3, 4, 5])
    expect(push.recorded[1]?.toolName).toBe('codex.continuity.rebuilt')
    expect(push.recorded[1]?.resultRef).toBe('continuity.codex.thread.t1.turn.t1')

    const wc = decodeWorkContextB64(captured.b64!)
    expect(wc.providerAuth).toEqual({
      agentToken: 'oa_agent_RAWTOKEN0123456789abcdef',
      authGrantRef: 'grant.codex.thread_1',
      baseUrl: 'https://staging.example',
      providerAccountRef: 'provider-account.codex.owner_1',
    })
    expect(wc.codexContinuity).toEqual({
      maxReplayMessages: 12,
      persistedCodexHome: false,
      previousTurnCount: 3,
      strategy: 'khala_sync_history_reprime',
    })
    expect(JSON.stringify(wc.codexContinuity)).not.toMatch(/CODEX_HOME|token|secret|authJson/i)
  })

  test('launch refused: finished(error) and token IS revoked', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const refuse: CloudGcpPlacementLaunchFn = () =>
      Promise.resolve({
        ok: false,
        providerFailureClass: 'provider_outage',
        reason: 'agent_computer_guest_network_failed',
      })
    const result = await dispatchCloudGcpRuntimeTurn(
      baseDeps({ executePush: push.executePush, launch: refuse }),
      admitted,
    )
    expect(result.outcome).toBe('failed')
    expect(result.reason).toBe('agent_computer_guest_network_failed')
    expect(result.providerFailureClass).toBe('provider_outage')
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

describe('planCloudGcpRuntimeAccountDispatch (CX-7 account serialization)', () => {
  test('same connected account serializes: first dispatches, second queues honestly', () => {
    const t1 = withPinnedAccount(admitted, 'acct_a')
    const t2 = withPinnedAccount(
      { ...admitted, threadId: 'thread.t2', turnId: 'turn.t2', workContextRef: 'wc2' },
      'acct_a',
    )
    const plan = planCloudGcpRuntimeAccountDispatch([t1, t2])
    expect(plan.dispatchable.map(decision => decision.turn.turnId)).toEqual(['turn.t1'])
    expect(plan.queued.map(decision => [decision.turn.turnId, decision.reason])).toEqual([
      ['turn.t2', 'account_busy_queued'],
    ])
  })

  test('two connected accounts are both dispatchable in the same tick', () => {
    const t1 = withPinnedAccount(admitted, 'acct_a')
    const t2 = withPinnedAccount(
      { ...admitted, threadId: 'thread.t2', turnId: 'turn.t2', workContextRef: 'wc2' },
      'acct_b',
    )
    const plan = planCloudGcpRuntimeAccountDispatch([t1, t2])
    expect(plan.queued).toEqual([])
    expect(plan.dispatchable.map(decision => decision.turn.turnId)).toEqual(['turn.t1', 'turn.t2'])
  })

  test('quota exhaustion rotates away from the pinned account when a fallback is available', () => {
    const t1 = withPinnedAccount(admitted, 'acct_exhausted')
    const plan = planCloudGcpRuntimeAccountDispatch([t1], {
      accounts: [
        {
          accountRefHash: 'acct_exhausted',
          authGrantRef: 'grant.exhausted',
          providerAccountRef: 'provider.exhausted',
          quotaState: 'exhausted',
        },
        {
          accountRefHash: 'acct_fallback',
          authGrantRef: 'grant.fallback',
          providerAccountRef: 'provider.fallback',
          quotaState: 'available',
        },
      ],
    })
    expect(plan.dispatchable).toHaveLength(1)
    expect(plan.dispatchable[0]?.rotation).toEqual({
      fromAccountRefHash: 'acct_exhausted',
      reason: 'account_exhausted',
      toAccountRefHash: 'acct_fallback',
    })
    expect(plan.dispatchable[0]?.turn.codexContinuity).toMatchObject({
      accountRefHash: 'acct_fallback',
      authGrantRef: 'grant.fallback',
      providerAccountRef: 'provider.fallback',
    })
  })

  test('active accounts count as busy before this tick starts', () => {
    const plan = planCloudGcpRuntimeAccountDispatch(
      [withPinnedAccount(admitted, 'acct_a')],
      { activeAccountRefHashes: ['acct_a'] },
    )
    expect(plan.dispatchable).toEqual([])
    expect(plan.queued[0]?.reason).toBe('account_busy_queued')
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
    expect(summary).toEqual({ failed: 0, launched: 0, queued: 0, rotated: 0, scanned: 0, skipped: 0 })
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
    expect(summary).toEqual({ failed: 0, launched: 2, queued: 0, rotated: 0, scanned: 2, skipped: 0 })
  })

  test('armed but no reader configured is a clean no-op', async () => {
    reset()
    const summary = await runCloudGcpRuntimeDispatch(baseDeps())
    expect(summary).toEqual({ failed: 0, launched: 0, queued: 0, rotated: 0, scanned: 0, skipped: 0 })
  })

  test('same account: records account_busy_queued and launches only one turn', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const summary = await runCloudGcpRuntimeDispatch(
      baseDeps({
        executePush: push.executePush,
        readAdmitted: () =>
          Promise.resolve([
            withPinnedAccount(admitted, 'acct_a'),
            withPinnedAccount(
              { ...admitted, threadId: 'thread.t2', turnId: 'turn.t2', workContextRef: 'wc2' },
              'acct_a',
            ),
          ]),
      }),
    )
    expect(summary).toEqual({ failed: 0, launched: 1, queued: 1, rotated: 0, scanned: 2, skipped: 0 })
    expect(push.recorded.some(event => event.toolName === 'codex.account_busy_queued')).toBe(true)
    expect(mintCalls).toHaveLength(1)
  })

  test('quota exhausted pinned account rotates to fallback and records typed receipt', async () => {
    reset()
    const push = makeRecordingExecutePush()
    const captured: { b64?: string } = {}
    const summary = await runCloudGcpRuntimeDispatch(
      baseDeps({
        accountStates: [
          {
            accountRefHash: 'acct_exhausted',
            authGrantRef: 'grant.exhausted',
            providerAccountRef: 'provider.exhausted',
            quotaState: 'exhausted',
          },
          {
            accountRefHash: 'acct_fallback',
            authGrantRef: 'grant.fallback',
            providerAccountRef: 'provider.fallback',
            quotaState: 'available',
          },
        ],
        executePush: push.executePush,
        launch: okLaunch(captured),
        readAdmitted: () => Promise.resolve([withPinnedAccount(admitted, 'acct_exhausted')]),
      }),
    )
    expect(summary).toEqual({ failed: 0, launched: 1, queued: 0, rotated: 1, scanned: 1, skipped: 0 })
    expect(push.recorded.find(event => event.toolName === 'account_exhausted')?.kind).toBe('tool.result')
    const wc = decodeWorkContextB64(captured.b64!)
    expect(wc.providerAuth).toMatchObject({
      authGrantRef: 'grant.fallback',
      providerAccountRef: 'provider.fallback',
    })
    expect(wc.codexTurn).toMatchObject({
      ownerUserId: admitted.ownerUserId,
    })
    expect(wc.verificationCommand).toEqual({
      argv: ['git', 'diff', '--cached', '--check'],
      commandRef: 'verify.agent-computer.git_diff_cached_check',
      timeoutSeconds: 120,
    })
    expect(wc.inference).toBeUndefined()
  })

  test('issues managed authority only after winning the durable claim', async () => {
    reset()
    let prepared = 0
    const rejected = makeRecordingExecutePush(event =>
      event.kind === 'turn.started' ? 'rejected' : 'applied')
    const skipped = await dispatchCloudGcpRuntimeTurn(baseDeps({
      executePush: rejected.executePush,
      prepareAfterClaim: turn => {
        prepared += 1
        return Promise.resolve(withPinnedAccount(turn, 'acct_owner'))
      },
    }), admitted)
    expect(skipped.outcome).toBe('skipped')
    expect(prepared).toBe(0)

    const applied = makeRecordingExecutePush()
    const captured: { b64?: string } = {}
    const launched = await dispatchCloudGcpRuntimeTurn(baseDeps({
      executePush: applied.executePush,
      launch: okLaunch(captured),
      prepareAfterClaim: turn => {
        prepared += 1
        return Promise.resolve(withPinnedAccount(turn, 'acct_owner'))
      },
    }), admitted)
    expect(launched.outcome).toBe('launched')
    expect(prepared).toBe(1)
    expect(decodeWorkContextB64(captured.b64!).providerAuth).toMatchObject({
      authGrantRef: 'grant.acct_owner',
      providerAccountRef: 'provider.acct_owner',
    })
  })

  test('a post-claim grant refusal settles error and never mints or launches', async () => {
    reset()
    let launched = false
    const push = makeRecordingExecutePush()
    const outcome = await dispatchCloudGcpRuntimeTurn(baseDeps({
      executePush: push.executePush,
      launch: input => {
        launched = true
        return okLaunch()(input)
      },
      prepareAfterClaim: () => Promise.reject(new Error('grant_unavailable')),
    }), admitted)
    expect(outcome.outcome).toBe('failed')
    expect(mintCalls).toHaveLength(0)
    expect(launched).toBe(false)
    expect(push.recorded.map(event => event.kind)).toEqual([
      'turn.started',
      'turn.finished',
    ])
    expect(push.recorded.at(-1)?.finishReason).toBe('error')
  })

  test('finalizes prepared lease state after launch success and refusal', async () => {
    reset()
    const finalized: Array<{
      leaseRef: string | undefined
      outcome: string
      reason: string | undefined
    }> = []
    const prepareAfterClaim = (turn: CloudGcpAdmittedWorkContext) =>
      Promise.resolve({
        ...withPinnedAccount(turn, 'acct_owner'),
        providerAccountLeaseRef: 'provider-account-lease_ref_managed',
      })
    const finalizeAfterDispatch: NonNullable<
      CloudGcpRuntimeDispatchDependencies['finalizeAfterDispatch']
    > = (turn, outcome) => {
      finalized.push({
        leaseRef: turn.providerAccountLeaseRef,
        outcome: outcome.outcome,
        reason: outcome.reason,
      })
      return Promise.resolve()
    }

    const launched = await dispatchCloudGcpRuntimeTurn(
      baseDeps({
        executePush: makeRecordingExecutePush().executePush,
        finalizeAfterDispatch,
        launch: okLaunch(),
        prepareAfterClaim,
      }),
      admitted,
    )
    const refused = await dispatchCloudGcpRuntimeTurn(
      baseDeps({
        executePush: makeRecordingExecutePush().executePush,
        finalizeAfterDispatch,
        launch: () =>
          Promise.resolve({ ok: false, reason: 'capacity_unavailable' }),
        prepareAfterClaim,
      }),
      { ...admitted, turnId: 'turn.t2' },
    )

    expect(launched.outcome).toBe('launched')
    expect(refused.outcome).toBe('failed')
    expect(finalized).toEqual([
      {
        leaseRef: 'provider-account-lease_ref_managed',
        outcome: 'launched',
        reason: undefined,
      },
      {
        leaseRef: 'provider-account-lease_ref_managed',
        outcome: 'failed',
        reason: 'capacity_unavailable',
      },
    ])
  })
})

describe('makeCloudCodingAdapterLaunchSeam', () => {
  const session = (
    over: Partial<CloudCodingSession> = {},
  ): CloudCodingSession => ({
    accountRef: 'agent:github:1',
    adapter: 'codex',
    agentComputerRef: 'agent-computer.run_gce_1',
    agentComputerState: 'provisioning',
    artifactRef: null,
    createdAt: '2026-07-07T00:00:00.000Z',
    lane: 'cloud-gcp',
    leaseRefs: [],
    lifecycleReceiptRefs: ['receipt.cloud.gce.provisioning.1'],
    placementRef: 'placement.cloud-coding.run_gce_1',
    repoRef: 'repo:octocat/Hello-World',
    repoTrustTier: 'private',
    resourceUsageReceiptRefs: [],
    sessionId: 'ccs.turn_t1',
    state: 'running',
    timeoutSeconds: 1800,
    workContextRef: 'work-context.agent-computer.wc1',
    ...over,
  })

  const launchInput = {
    authGrantRef: 'provider-auth-grant.g1',
    objective: 'seam-a',
    ownerUserId: 'github:14167547',
    providerAccountRef: 'provider-account.codex.1',
    repoRef: 'repo:octocat/Hello-World',
    sessionId: 'ccs.turn_t1',
    threadRef: 'thread.t1',
    timeoutSeconds: 1800,
    workContextB64: 'eyJhIjoxfQ==',
    workContextRef: 'work-context.agent-computer.wc1',
  }

  test('maps a successful adapter launch and passes work_context_b64 + owner account ref', async () => {
    let launchArg: unknown
    const adapter: CloudCodingRuntimeAdapter = {
      id: 'fake',
      get: () => Effect.sync((): undefined => undefined),
      launch: input => {
        launchArg = input
        return Effect.succeed(session())
      },
    }
    const seam = makeCloudCodingAdapterLaunchSeam(adapter)
    const result = await seam(launchInput)
    expect(result).toEqual({
      agentComputerState: 'provisioning',
      lifecycleReceiptRefs: ['receipt.cloud.gce.provisioning.1'],
      ok: true,
      placementRef: 'placement.cloud-coding.run_gce_1',
      sessionId: 'ccs.turn_t1',
    })
    const arg = launchArg as {
      accountRef: string
      lane: string
      request: { options: Record<string, unknown>; lane: string }
    }
    expect(arg.accountRef).toBe('agent:github:14167547')
    expect(arg.lane).toBe('cloud-gcp')
    expect(arg.request.lane).toBe('cloud-gcp')
    expect(arg.request.options.workContextB64).toBe('eyJhIjoxfQ==')
    expect(arg.request.options.authGrantRef).toBe('provider-auth-grant.g1')
    expect(arg.request.options.providerAccountRef).toBe(
      'provider-account.codex.1',
    )
  })

  test('maps a typed adapter failure to ok:false with the adapter reason', async () => {
    const adapter: CloudCodingRuntimeAdapter = {
      id: 'fake',
      get: () => Effect.sync((): undefined => undefined),
      launch: () =>
        Effect.fail(
          new CloudCodingAdapterError({
            adapterId: 'not-armed-cloud-gce',
            reason: 'cloud_gce_provisioning_not_armed',
          }),
        ),
    }
    const seam = makeCloudCodingAdapterLaunchSeam(adapter)
    const result = await seam(launchInput)
    expect(result).toEqual({
      ok: false,
      reason: 'cloud_gce_provisioning_not_armed',
    })
  })

  test('maps an allowlisted guest failure to a provider failover class without raw detail', async () => {
    const adapter: CloudCodingRuntimeAdapter = {
      id: 'fake',
      get: () => Effect.sync((): undefined => undefined),
      launch: () =>
        Effect.fail(
          new CloudCodingAdapterError({
            adapterId: 'openagents-cloud-control',
            guestFailureClass: 'account_rate_limited',
            reason: 'agent_computer_guest_account_rate_limited',
          }),
        ),
    }
    const seam = makeCloudCodingAdapterLaunchSeam(adapter)
    const result = await seam(launchInput)
    expect(result).toEqual({
      ok: false,
      providerFailureClass: 'rate_limited',
      reason: 'agent_computer_guest_account_rate_limited',
    })
    expect(JSON.stringify(result)).not.toContain('raw provider message')
  })
})
