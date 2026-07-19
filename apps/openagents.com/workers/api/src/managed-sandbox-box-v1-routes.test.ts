import { BoxApi, Configuration, ResponseError } from '@asciidev/box-sdk'
import { Effect } from 'effect'
import {
  type IncomingMessage,
  type ServerResponse,
  createServer,
} from 'node:http'
import { afterEach, describe, expect, test } from 'vite-plus/test'

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
})
