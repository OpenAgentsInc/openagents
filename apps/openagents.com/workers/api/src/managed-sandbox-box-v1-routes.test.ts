import { BoxApi, Configuration, ResponseError } from '@asciidev/box-sdk'
import { Effect } from 'effect'
import {
  type IncomingMessage,
  type ServerResponse,
  createServer,
} from 'node:http'
import { afterEach, describe, expect, test } from 'vite-plus/test'

import type { OpenAgentsWorkerEnv } from './bindings'
import { managedSandboxBoxV1RuntimeForEnv } from './managed-sandbox-box-v1-adapter'
import {
  BoxV1FacadeError,
  type BoxV1Principal,
  makeBoxV1Routes,
  unavailableBoxV1Runtime,
} from './managed-sandbox-box-v1-routes'
import {
  BoxV1MemoryAuthority,
  boxV1TestPolicy,
  boxV1TestPrincipal,
  makeBoxV1MemoryRuntime,
} from './managed-sandbox-box-v1.test-support'

type TestEnv = Readonly<{ enabled: boolean }>

const bearer = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  return authorization?.startsWith('Bearer ')
    ? authorization.slice('Bearer '.length)
    : undefined
}

const foreignPrincipal: BoxV1Principal = {
  actorRef: 'agent:foreign-box-sdk',
  ownerRef: 'owner.foreign-box-sdk',
  tenantRef: 'tenant.foreign-box-sdk',
  login: 'foreign-box-sdk',
  email: null,
}

const authError = (status: 401 | 503) =>
  new BoxV1FacadeError({
    code: status === 401 ? 'authentication_required' : 'upstream_unavailable',
    status,
    message:
      status === 401 ? 'test bearer is invalid' : 'test authentication fault',
    retryable: status === 503,
  })

const makeHandler = (
  options: {
    authority?: BoxV1MemoryAuthority
    runtime?: ReturnType<typeof makeBoxV1MemoryRuntime>
    authenticationFault?: boolean
  } = {},
) => {
  const authority = options.authority ?? new BoxV1MemoryAuthority()
  const runtime = options.runtime ?? makeBoxV1MemoryRuntime()
  const routes = makeBoxV1Routes<TestEnv>({
    enabled: env => env.enabled,
    authenticate: request => {
      if (options.authenticationFault === true)
        return Effect.fail(authError(503))
      const token = bearer(request)
      if (token === 'test-token') return Effect.succeed(boxV1TestPrincipal)
      if (token === 'foreign-token') return Effect.succeed(foreignPrincipal)
      return Effect.fail(authError(401))
    },
    policy: () => Effect.succeed(boxV1TestPolicy),
    store: () => Effect.succeed(authority),
    runtime: () => Effect.succeed(runtime),
    now: () => new Date('2026-07-19T18:30:00.000Z'),
  })
  return {
    authority,
    handle: (request: Request, enabled = true): Promise<Response> => {
      const effect = routes.routeBoxV1Request(request, { enabled })
      if (effect === undefined)
        throw new Error(`unmatched test route: ${request.url}`)
      return Effect.runPromise(effect)
    },
  }
}

const fetchFor = (
  handle: (request: Request) => Promise<Response>,
): typeof fetch =>
  (async (input: RequestInfo | URL, init?: RequestInit) =>
    handle(new Request(input, init))) as typeof fetch

const apiFor = (basePath: string, token: string, fetchApi?: typeof fetch) =>
  new BoxApi(
    new Configuration({
      basePath,
      accessToken: token,
      ...(fetchApi === undefined ? {} : { fetchApi }),
    }),
  )

const retryHeaders =
  (operation: string) =>
  async ({ init }: { init: RequestInit }): Promise<RequestInit> => ({
    ...init,
    headers: {
      ...(init.headers as Record<string, string>),
      'idempotency-key': `sdk-retry-${operation}`,
    },
  })

const expectResponseError = async (
  promise: Promise<unknown>,
  status: number,
  code: string,
): Promise<void> => {
  const caught = await promise
    .then(() => undefined)
    .catch((error: unknown) => error)
  expect(caught).toBeInstanceOf(ResponseError)
  const response = (caught as ResponseError).response
  expect(response.status).toBe(status)
  expect(await response.json()).toEqual(
    expect.objectContaining({
      code,
      requestId: expect.stringMatching(/^request\.box\./),
    }),
  )
}

const admittedCalls = (api: BoxApi) => [
  () => api.me(),
  () => api.limits(),
  () => api.boxes(),
  () => api.create(),
  () => api.get({ boxId: 'sandbox.box.missing' }),
  () =>
    api.update({
      boxId: 'sandbox.box.missing',
      updateBoxRequest: { ttlSeconds: 60 },
    }),
  () => api.remove({ boxId: 'sandbox.box.missing' }),
  () => api.stop({ boxId: 'sandbox.box.missing' }),
  () => api.resume({ boxId: 'sandbox.box.missing' }),
  () =>
    api.prompt({
      boxId: 'sandbox.box.missing',
      promptRequest: { provider: 'codex', prompt: 'test' },
    }),
  () =>
    api.promptRunStatus({
      boxId: 'sandbox.box.missing',
      promptId: 'turn.box.missing',
    }),
  () => api.events({ boxId: 'sandbox.box.missing', sort: 'asc' }),
  () => api.interrupt({ boxId: 'sandbox.box.missing' }),
  () =>
    api.readFile({ boxId: 'sandbox.box.missing', path: 'workspace/README.md' }),
  () =>
    api.writeFile({
      boxId: 'sandbox.box.missing',
      fileWriteRequest: { path: 'workspace/README.md', content: 'test' },
    }),
  () =>
    api.command({
      boxId: 'sandbox.box.missing',
      commandRequest: { command: 'pwd' },
    }),
  () =>
    api.artifact({ boxId: 'sandbox.box.missing', path: 'workspace/README.md' }),
]

const exerciseAdmittedCorpus = async (api: BoxApi): Promise<string> => {
  const me = await api.me()
  expect(me.user.login).toBe('openagents-box-sdk')
  expect(await api.me()).toEqual(me)

  const limits = await api.limits()
  expect(limits.canStart).toBe(true)
  expect(await api.limits()).toEqual(limits)

  const created = await api.create(
    { createBoxRequest: { ttlSeconds: 3_600, noEnv: true } },
    retryHeaders('create-a'),
  )
  const boxId = created.box.id
  expect(created.box).toMatchObject({
    url: undefined,
    ip: undefined,
    desktopAvailable: false,
    desktopUrl: undefined,
    snapshotAvailable: false,
    subdomain: undefined,
  })
  const createReplay = await api.create(
    { createBoxRequest: { ttlSeconds: 3_600, noEnv: true } },
    retryHeaders('create-a'),
  )
  expect(createReplay.box.id).toBe(boxId)

  await api.create(
    { createBoxRequest: { ttlSeconds: 1_800, noEnv: true } },
    retryHeaders('create-b'),
  )
  const firstList = await api.boxes({ limit: 1, sort: 'desc' })
  expect(firstList.boxes).toHaveLength(1)
  expect(firstList.pageInfo?.hasMore).toBe(true)
  const listCursor = firstList.pageInfo?.nextCursor
  if (listCursor === undefined) throw new Error('expected Box list cursor')
  const secondList = await api.boxes({
    limit: 1,
    sort: 'desc',
    cursor: listCursor,
  })
  expect(secondList.boxes).toHaveLength(1)
  expect(secondList.boxes[0]?.id).not.toBe(firstList.boxes[0]?.id)
  expect(await api.boxes({ limit: 1, sort: 'desc' })).toEqual(firstList)

  const info = await api.get({ boxId })
  expect(info.box.id).toBe(boxId)
  expect(await api.get({ boxId })).toEqual(info)

  const updated = await api.update(
    { boxId, updateBoxRequest: { ttlSeconds: 2_400 } },
    retryHeaders('update'),
  )
  const updateReplay = await api.update(
    { boxId, updateBoxRequest: { ttlSeconds: 2_400 } },
    retryHeaders('update'),
  )
  expect(updateReplay.box.updatedAt).toEqual(updated.box.updatedAt)

  const written = await api.writeFile({
    boxId,
    fileWriteRequest: {
      path: 'workspace/result.txt',
      content: 'sdk-compatible',
    },
  })
  expect(written.size).toBe(14)
  expect(
    await api.writeFile({
      boxId,
      fileWriteRequest: {
        path: 'workspace/result.txt',
        content: 'sdk-compatible',
      },
    }),
  ).toEqual(written)
  const read = await api.readFile({ boxId, path: 'workspace/result.txt' })
  expect(read.content).toBe('sdk-compatible')
  expect(await api.readFile({ boxId, path: 'workspace/result.txt' })).toEqual(
    read,
  )

  const command = await api.command({
    boxId,
    commandRequest: { command: 'pwd' },
  })
  expect(command.stdout).toBe('workspace\n')
  expect(
    await api.command({ boxId, commandRequest: { command: 'pwd' } }),
  ).toEqual(command)
  const artifact = await api.artifact({ boxId, path: 'workspace/result.txt' })
  expect(await artifact.text()).toBe('sdk-compatible')
  expect(
    await (await api.artifact({ boxId, path: 'workspace/result.txt' })).text(),
  ).toBe('sdk-compatible')

  const prompted = await api.prompt(
    {
      boxId,
      promptRequest: {
        provider: 'codex',
        model: 'gpt-5.4',
        prompt: 'Inspect the repository.',
      },
    },
    retryHeaders('prompt'),
  )
  const promptReplay = await api.prompt(
    {
      boxId,
      promptRequest: {
        provider: 'codex',
        model: 'gpt-5.4',
        prompt: 'Inspect the repository.',
      },
    },
    retryHeaders('prompt'),
  )
  expect(promptReplay.promptId).toBe(prompted.promptId)
  const promptStatus = await api.promptRunStatus({
    boxId,
    promptId: prompted.promptId,
  })
  expect(promptStatus.promptRun.status).toBe('running')
  expect(
    await api.promptRunStatus({ boxId, promptId: prompted.promptId }),
  ).toEqual(promptStatus)

  const firstEvents = await api.events({ boxId, limit: 1, sort: 'asc' })
  expect(firstEvents.events).toHaveLength(1)
  const eventReplay = await api.events({ boxId, limit: 1, sort: 'asc' })
  expect(eventReplay).toEqual(firstEvents)
  const oldGenerationCursor = firstEvents.pageInfo?.nextCursor
  expect(oldGenerationCursor).toMatch(/^boxc\.1\./)
  const sequences: Array<number> = []
  let cursor: string | null | undefined = null
  do {
    const page = await api.events({ boxId, limit: 1, sort: 'asc', cursor })
    const sequence = (
      page.events[0]?.data as { nativeEventSequence?: number } | undefined
    )?.nativeEventSequence
    if (sequence !== undefined) sequences.push(sequence)
    cursor = page.pageInfo?.nextCursor
  } while (cursor !== null && cursor !== undefined)
  expect(sequences).toEqual([...sequences].sort((left, right) => left - right))

  const interrupted = await api.interrupt({ boxId }, retryHeaders('interrupt'))
  const interruptReplay = await api.interrupt(
    { boxId },
    retryHeaders('interrupt'),
  )
  expect(interruptReplay.status).toBe(interrupted.status)

  const stopped = await api.stop({ boxId }, retryHeaders('stop'))
  const stopReplay = await api.stop({ boxId }, retryHeaders('stop'))
  expect(stopReplay.box?.state).toBe(stopped.box?.state)

  const resumed = await api.resume({ boxId }, retryHeaders('resume'))
  const resumeReplay = await api.resume({ boxId }, retryHeaders('resume'))
  expect(resumeReplay.box?.state).toBe(resumed.box?.state)
  if (oldGenerationCursor === null || oldGenerationCursor === undefined) {
    throw new Error('expected an event cursor before resume')
  }
  await expectResponseError(
    api.events({ boxId, cursor: oldGenerationCursor, sort: 'asc' }),
    409,
    'conflict',
  )

  await expectResponseError(
    api.update(
      { boxId, updateBoxRequest: { ttlSeconds: 1_200 } },
      retryHeaders('update'),
    ),
    409,
    'conflict',
  )

  const removed = await api.remove({ boxId }, retryHeaders('remove'))
  const removeReplay = await api.remove({ boxId }, retryHeaders('remove'))
  expect(removed.status).toBe('deleted')
  expect(removeReplay).toEqual(removed)

  await expectResponseError(
    api.fork({ boxId }),
    501,
    'capability_not_implemented',
  )
  return boxId
}

const servers: Array<ReturnType<typeof createServer>> = []

afterEach(async () => {
  await Promise.all(
    servers.splice(0).map(
      server =>
        new Promise<void>(resolve => {
          server.close(() => resolve())
        }),
    ),
  )
})

const requestBody = async (
  request: IncomingMessage,
): Promise<Uint8Array | undefined> => {
  const chunks: Array<Uint8Array> = []
  for await (const chunk of request) {
    chunks.push(
      typeof chunk === 'string' ? new TextEncoder().encode(chunk) : chunk,
    )
  }
  if (chunks.length === 0) return undefined
  const size = chunks.reduce((total, chunk) => total + chunk.byteLength, 0)
  const body = new Uint8Array(size)
  let offset = 0
  for (const chunk of chunks) {
    body.set(chunk, offset)
    offset += chunk.byteLength
  }
  return body
}

const writeNodeResponse = async (
  target: ServerResponse,
  response: Response,
): Promise<void> => {
  target.statusCode = response.status
  response.headers.forEach((value, key) => target.setHeader(key, value))
  target.end(new Uint8Array(await response.arrayBuffer()))
}

const startStagingServer = async (
  handle: (request: Request) => Promise<Response>,
): Promise<string> => {
  const server = createServer(async (incoming, outgoing) => {
    try {
      const body = await requestBody(incoming)
      const address = server.address()
      if (address === null || typeof address === 'string')
        throw new Error('missing server address')
      const request = new Request(
        `http://127.0.0.1:${address.port}${incoming.url ?? '/'}`,
        {
          method: incoming.method ?? 'GET',
          headers: incoming.headers as HeadersInit,
          ...(body === undefined
            ? {}
            : { body: new TextDecoder().decode(body) }),
        },
      )
      await writeNodeResponse(outgoing, await handle(request))
    } catch (error) {
      outgoing.statusCode = 500
      outgoing.end(
        error instanceof Error ? error.message : 'staging server failure',
      )
    }
  })
  servers.push(server)
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => resolve())
  })
  const address = server.address()
  if (address === null || typeof address === 'string')
    throw new Error('missing server address')
  return `http://127.0.0.1:${address.port}/v1`
}

describe('SBX-03 Box-v1 compatibility facade', () => {
  test('passes the exact unmodified SDK lifecycle corpus against the local fake', async () => {
    const harness = makeHandler()
    const api = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(harness.handle),
    )
    const boxId = await exerciseAdmittedCorpus(api)
    const rawProjection = (await (
      await harness.handle(
        new Request(`https://local-box.test/v1/boxes/${boxId}`, {
          headers: { authorization: 'Bearer test-token' },
        }),
      )
    ).json()) as { box: Record<string, unknown> }
    expect(rawProjection.box).toMatchObject({
      url: null,
      ip: null,
      desktopAvailable: false,
      desktopUrl: null,
      snapshotAvailable: false,
      subdomain: null,
    })
    await expectResponseError(
      apiFor(
        'https://local-box.test/v1',
        'foreign-token',
        fetchFor(harness.handle),
      ).get({ boxId }),
      403,
      'permission_denied',
    )
  })

  test('passes the same unmodified SDK corpus over the loopback staging HTTP service', async () => {
    const harness = makeHandler()
    const basePath = await startStagingServer(harness.handle)
    await exerciseAdmittedCorpus(apiFor(basePath, 'test-token'))
  })

  test('returns stable authentication errors and authentication faults for every admitted method', async () => {
    const normal = makeHandler()
    const invalid = apiFor(
      'https://local-box.test/v1',
      'invalid-token',
      fetchFor(normal.handle),
    )
    for (const call of admittedCalls(invalid)) {
      await expectResponseError(call(), 401, 'authentication_required')
    }

    const faulted = makeHandler({ authenticationFault: true })
    const unavailable = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(faulted.handle),
    )
    for (const call of admittedCalls(unavailable)) {
      await expectResponseError(call(), 503, 'upstream_unavailable')
    }
  })

  test('does not fake runtime success and remains default-off', async () => {
    const harness = makeHandler({ runtime: unavailableBoxV1Runtime })
    const disabled = await harness.handle(
      new Request('https://local-box.test/v1/me', {
        headers: { authorization: 'Bearer test-token' },
      }),
      false,
    )
    expect(disabled.status).toBe(404)

    const api = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(harness.handle),
    )
    const created = await api.create({}, retryHeaders('runtime-unavailable'))
    await expectResponseError(
      api.readFile({ boxId: created.box.id, path: 'workspace/README.md' }),
      503,
      'upstream_unavailable',
    )
    await expectResponseError(
      api.prompt({
        boxId: created.box.id,
        promptRequest: { provider: 'codex', prompt: 'test' },
      }),
      503,
      'upstream_unavailable',
    )
  })

  test('enforces SBX-05 guest I/O admission and projects bounded receipts', async () => {
    const harness = makeHandler()
    const api = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(harness.handle),
    )
    const boxId = (
      await api.create(
        { createBoxRequest: { noEnv: true } },
        retryHeaders('sbx05-create'),
      )
    ).box.id

    const writeResponse = await harness.handle(
      new Request(`https://local-box.test/v1/boxes/${boxId}/files`, {
        method: 'PUT',
        headers: {
          authorization: 'Bearer test-token',
          'content-type': 'application/json',
          'idempotency-key': 'sbx05-write',
        },
        body: JSON.stringify({
          path: 'workspace/result.txt',
          content: 'sdk-compatible',
        }),
      }),
    )
    expect(writeResponse.status).toBe(200)
    expect(await writeResponse.json()).toMatchObject({
      type: 'file.written',
      size: 14,
      openagents: {
        action: 'write_file',
        outcome: 'succeeded',
        resourceGeneration: 1,
        processTerminated: true,
        descendantsRemaining: 0,
        scratchCleaned: true,
        ingressClosed: true,
        egressDenied: true,
        pathPolicy: 'resolved_beneath_workspace_root',
        symlinkTraversal: false,
        secretScan: 'clean',
      },
    })

    const artifact = await api.artifact({
      boxId,
      path: 'workspace/result.txt',
    })
    expect(await artifact.text()).toBe('sdk-compatible')
    const rawArtifact = await harness.handle(
      new Request(
        `https://local-box.test/v1/boxes/${boxId}/artifacts?path=workspace%2Fresult.txt`,
        { headers: { authorization: 'Bearer test-token' } },
      ),
    )
    expect(rawArtifact.headers.get('x-openagents-artifact-ref')).toMatch(
      /^artifact\.sha256\.[a-f0-9]{64}$/,
    )
    expect(rawArtifact.headers.get('x-openagents-source-generation')).toBe('1')
    expect(rawArtifact.headers.get('x-openagents-retention-until')).toBe(
      '2026-07-20T18:30:00.000Z',
    )
    expect(rawArtifact.headers.get('x-openagents-receipt-ref')).toMatch(
      /^receipt\./,
    )

    for (const path of [
      '/etc/passwd',
      'workspace/../etc/passwd',
      'workspace//secret',
      'workspace/./secret',
      'workspace\\secret',
    ]) {
      await expectResponseError(
        api.readFile({ boxId, path }),
        400,
        'validation_failed',
      )
    }

    await expectResponseError(
      api.writeFile({
        boxId,
        fileWriteRequest: {
          path: 'workspace/oversized.txt',
          content: 'x'.repeat(1_048_577),
        },
      }),
      400,
      'validation_failed',
    )

    const resource = harness.authority.resources.get(boxId)
    if (resource === undefined) throw new Error('expected managed sandbox')
    harness.authority.resources.set(boxId, {
      ...resource,
      capabilities: resource.capabilities.map(capability =>
        capability.kind === 'command'
          ? { ...capability, state: 'revoked' as const }
          : capability,
      ),
    })
    await expectResponseError(
      api.command({ boxId, commandRequest: { command: 'pwd' } }),
      403,
      'permission_denied',
    )
  })

  test('validates the production private artifact response against exact returned bytes', async () => {
    const harness = makeHandler()
    const api = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(harness.handle),
    )
    const boxId = (
      await api.create(
        { createBoxRequest: { noEnv: true } },
        retryHeaders('sbx05-adapter-create'),
      )
    ).box.id
    const resource = harness.authority.resources.get(boxId)
    if (resource === undefined) throw new Error('expected managed sandbox')

    const originalFetch = globalThis.fetch
    let capturedUrl = ''
    let capturedAuthorization = ''
    let capturedBody: Record<string, unknown> = {}
    globalThis.fetch = (async (input, init) => {
      capturedUrl = String(input)
      capturedAuthorization =
        new Headers(init?.headers).get('authorization') ?? ''
      capturedBody = JSON.parse(String(init?.body)) as Record<string, unknown>
      return new Response(
        JSON.stringify({
          schemaVersion: 'openagents.managed_sandbox_guest_io.v1',
          action: 'read_artifact',
          operationRef: 'operation.sbx05.adapter',
          sandboxRef: resource.sandboxRef,
          resourceGeneration: resource.resourceGeneration,
          contentBase64: 'YXJ0aWZhY3Q=',
          receipt: {
            schemaVersion: 'openagents.managed_sandbox_guest_io_receipt.v1',
            receiptRef: 'receipt.sbx05.adapter',
            operationRef: 'operation.sbx05.adapter',
            sandboxRef: resource.sandboxRef,
            resourceGeneration: resource.resourceGeneration,
            capabilityRef: 'capability.sbx05.adapter',
            action: 'read_artifact',
            outcome: 'succeeded',
            pathDigest: `sha256:${'c'.repeat(64)}`,
            startedAt: '2026-07-19T18:30:00.000Z',
            finishedAt: '2026-07-19T18:30:01.000Z',
            bytesRead: 8,
            bytesWritten: 0,
            cpuMillis: 1,
            networkBytes: 0,
            processTerminated: true,
            descendantsRemaining: 0,
            scratchCleaned: true,
            ingressClosed: true,
            egressDenied: true,
            pathPolicy: 'resolved_beneath_workspace_root',
            symlinkTraversal: false,
            secretScan: 'clean',
            evidenceRefs: ['evidence.sbx05.adapter'],
          },
          artifact: {
            schemaVersion: 'openagents.managed_sandbox_artifact_receipt.v1',
            artifactRef:
              'artifact.sha256.c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c',
            contentDigest:
              'sha256:c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c',
            byteLength: 8,
            sourceGeneration: resource.resourceGeneration,
            sourcePathDigest: `sha256:${'c'.repeat(64)}`,
            retentionUntil: '2026-07-20T18:30:00.000Z',
            contentType: 'application/octet-stream',
            evidenceRefs: ['evidence.sbx05.adapter'],
          },
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }) as typeof fetch

    try {
      const runtime = await Effect.runPromise(
        managedSandboxBoxV1RuntimeForEnv({
          OA_CLOUD_CONTROL_URL: 'https://control.test',
          OA_CLOUD_CONTROL_TOKEN: 'private-test-bearer',
        } as unknown as OpenAgentsWorkerEnv),
      )
      const result = await Effect.runPromise(
        runtime.artifact({
          principal: boxV1TestPrincipal,
          resource,
          operationRef: 'operation.sbx05.adapter',
          idempotencyRef: 'idempotency.sbx05.adapter',
          capabilityRef: 'capability.sbx05.adapter',
          capabilityState: 'active',
          capabilityExpiresAt: '2026-07-19T21:00:00.000Z',
          requestedAt: '2026-07-19T18:30:00.000Z',
          limits: {
            workspaceRootRef: 'workspace.managed-sandbox',
            maxFileBytes: 1_048_576,
            maxArtifactBytes: 10_000_000,
            maxOutputBytes: 131_072,
            maxDurationMillis: 30_000,
            maxCpuMillis: 30_000,
            maxProcesses: 32,
            maxNetworkBytes: 0,
            networkPolicyRef: 'network-policy.managed-sandbox.deny-all',
          },
          path: 'workspace/result.bin',
          retentionUntil: '2026-07-20T18:30:00.000Z',
        }),
      )
      expect(new TextDecoder().decode(result.bytes)).toBe('artifact')
      expect(result.artifact.contentDigest).toBe(
        'sha256:c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c',
      )
      expect(capturedUrl).toBe(
        'https://control.test/v1/managed-sandbox/runtime/io',
      )
      expect(capturedAuthorization).toBe('Bearer private-test-bearer')
      expect(capturedBody).toMatchObject({
        schemaVersion: 'openagents.managed_sandbox_guest_io.v1',
        action: 'read_artifact',
        sandboxRef: resource.sandboxRef,
        resourceGeneration: resource.resourceGeneration,
        path: 'workspace/result.bin',
      })
      expect(JSON.stringify(capturedBody)).not.toContain('private-test-bearer')
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test('streams Codex and Claude turn events through the unmodified SDK cursor helpers', async () => {
    const base = makeBoxV1MemoryRuntime()
    const runtime: ReturnType<typeof makeBoxV1MemoryRuntime> = {
      ...base,
      sync: input => {
        if (input.afterTurnSequence !== 1) return Effect.succeed([])
        return Effect.succeed([
          {
            _tag: 'RuntimeTextDelta' as const,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.turn.resourceGeneration,
            turnEventSequence: 2,
            content: `${input.turn.runtime.provider} working`,
            observedAt: '2026-07-19T18:30:01.000Z',
          },
          {
            _tag: 'RuntimeToolStarted' as const,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.turn.resourceGeneration,
            turnEventSequence: 3,
            toolCallRef: `tool.${input.turn.runtime.provider}.1`,
            toolName: 'shell',
            observedAt: '2026-07-19T18:30:02.000Z',
          },
          {
            _tag: 'RuntimeToolCompleted' as const,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.turn.resourceGeneration,
            turnEventSequence: 4,
            toolCallRef: `tool.${input.turn.runtime.provider}.1`,
            toolName: 'shell',
            outcome: 'succeeded' as const,
            evidenceRefs: [`evidence.${input.turn.runtime.provider}.tool.1`],
            observedAt: '2026-07-19T18:30:03.000Z',
          },
          {
            _tag: 'RuntimeUsageRecorded' as const,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.turn.resourceGeneration,
            turnEventSequence: 5,
            usage: {
              inputTokens: 9,
              outputTokens: 4,
              providerUsageRef: `usage.${input.turn.runtime.provider}.1`,
              exact: true,
            },
            observedAt: '2026-07-19T18:30:04.000Z',
          },
          {
            _tag: 'RuntimeSettled' as const,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.turn.resourceGeneration,
            turnEventSequence: 6,
            finishReason: 'structural_completion' as const,
            observedAt: '2026-07-19T18:30:05.000Z',
          },
        ])
      },
    }
    const harness = makeHandler({ runtime })
    const api = apiFor(
      'https://local-box.test/v1',
      'test-token',
      fetchFor(harness.handle),
    )
    const boxId = (
      await api.create(
        { createBoxRequest: { noEnv: true } },
        retryHeaders('runtime-events-create'),
      )
    ).box.id

    for (const provider of ['codex', 'claude'] as const) {
      const prompted = await api.prompt(
        {
          boxId,
          promptRequest: {
            provider,
            model: provider === 'codex' ? 'gpt-5.6' : 'claude-sonnet-4-5',
            prompt: `Run the ${provider} component proof.`,
          },
        },
        retryHeaders(`runtime-events-${provider}`),
      )
      const status = await api.promptRunStatus({
        boxId,
        promptId: prompted.promptId,
      })
      expect(status.promptRun).toMatchObject({ status: 'finished', done: true })
    }

    const page = await api.events({ boxId, limit: 100, sort: 'asc' })
    const runtimeTypes = page.events.map(event => event.type)
    expect(
      runtimeTypes.filter(type => type === 'prompt.response'),
    ).toHaveLength(2)
    expect(runtimeTypes.filter(type => type === 'prompt.usage')).toHaveLength(2)
    expect(
      runtimeTypes.filter(type => type === 'prompt.finished'),
    ).toHaveLength(2)
  })
})
