import type { GitReceivePackRequest } from '../../../../pylon/src/git-receive-pack'
import {
  GitReceivePackParseError,
  parseGitReceivePackRequest,
} from '../../../../pylon/src/git-receive-pack'
import { Effect } from 'effect'

import type { ForgeCoordinationStore } from './forge-coordination-store'
import {
  type ForgeGitCanonicalStore,
  ForgeGitCanonicalStoreError,
} from './forge-git-canonical-store'
import {
  type ForgeGitPackfileArchiveStore,
  forgeGitPackfileObjectFormatForCapabilities,
} from './forge-git-packfile-archive-store'
import type {
  ForgeGitAccessTokenSession,
  ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

export const FORGE_GIT_RECEIVE_PACK_ADVERTISEMENT_CONTENT_TYPE =
  'application/x-git-receive-pack-advertisement'
export const FORGE_GIT_RECEIVE_PACK_REQUEST_CONTENT_TYPE =
  'application/x-git-receive-pack-request'
export const FORGE_GIT_RECEIVE_PACK_RESULT_CONTENT_TYPE =
  'application/x-git-receive-pack-result'

const maxReceivePackBytesDefault = 16 * 1024 * 1024
const textEncoder = new TextEncoder()
const zeroSha1 = '0'.repeat(40)

type ForgeGitIntakeRouteDependencies<Bindings> = Readonly<{
  makeArchiveStore: (env: Bindings) => ForgeGitPackfileArchiveStore
  makeCanonicalStore: (env: Bindings) => ForgeGitCanonicalStore
  makeCoordinationStore: (env: Bindings) => ForgeCoordinationStore
  makeTenantGitAuthStore: (env: Bindings) => ForgeTenantGitAuthStore
  maxReceivePackBytes?: number
  nowIso?: () => string
}>

type ForgeGitRouteMatch = Readonly<{
  kind: 'advertise' | 'receive-pack'
  tenantRef: string
  repositoryRef: string
}>

class ForgeGitHttpError extends Error {
  constructor(
    readonly status: number,
    readonly errorCode: string,
    readonly reason?: string,
  ) {
    super(reason ?? errorCode)
    this.name = 'ForgeGitHttpError'
  }
}

const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')
  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token
    : undefined
}

const safeDecodePathSegment = (value: string): string | undefined => {
  try {
    const decoded = decodeURIComponent(value)
    return decoded.trim() === '' ? undefined : decoded
  } catch {
    return undefined
  }
}

const normalizeRepositoryRef = (value: string): string =>
  value.endsWith('.git') ? value.slice(0, -'.git'.length) : value

const matchForgeGitRoute = (request: Request): ForgeGitRouteMatch | undefined => {
  const url = new URL(request.url)
  const parts = url.pathname.split('/').filter(part => part !== '')
  if (parts[0] !== 'git') {
    return undefined
  }

  const tenantRef = parts[1] === undefined ? undefined : safeDecodePathSegment(parts[1])
  const repositorySegment =
    parts[2] === undefined ? undefined : safeDecodePathSegment(parts[2])
  if (tenantRef === undefined || repositorySegment === undefined) {
    return undefined
  }

  if (
    request.method === 'GET' &&
    parts.length === 5 &&
    parts[3] === 'info' &&
    parts[4] === 'refs' &&
    url.searchParams.get('service') === 'git-receive-pack'
  ) {
    return {
      kind: 'advertise',
      repositoryRef: normalizeRepositoryRef(repositorySegment),
      tenantRef,
    }
  }

  if (
    parts.length === 4 &&
    parts[3] === 'git-receive-pack' &&
    (request.method === 'POST' || request.method === 'GET')
  ) {
    return {
      kind: 'receive-pack',
      repositoryRef: normalizeRepositoryRef(repositorySegment),
      tenantRef,
    }
  }

  return undefined
}

const routeErrorResponse = (error: unknown) => {
  if (error instanceof ForgeGitHttpError) {
    return noStoreJsonResponse(
      {
        error: error.errorCode,
        ...(error.reason === undefined ? {} : { reason: error.reason }),
      },
      { status: error.status },
    )
  }
  if (error instanceof ForgeGitCanonicalStoreError) {
    return noStoreJsonResponse(
      { error: error.errorCode, reason: error.message },
      { status: error.status },
    )
  }
  if (error instanceof GitReceivePackParseError) {
    return noStoreJsonResponse(
      {
        error: 'forge_git_receive_pack_malformed',
        reason: error.message,
      },
      { status: 400 },
    )
  }

  return noStoreJsonResponse(
    { error: 'forge_git_receive_pack_storage_error' },
    { status: 500 },
  )
}

const routeEffect = <A>(run: () => Promise<A>) =>
  Effect.promise(async () => {
    try {
      return await run()
    } catch (error) {
      return routeErrorResponse(error)
    }
  })

const authenticateGit = async <Bindings>(
  request: Request,
  env: Bindings,
  match: ForgeGitRouteMatch,
  dependencies: ForgeGitIntakeRouteDependencies<Bindings>,
  nowIso: string,
): Promise<ForgeGitAccessTokenSession> => {
  const token = readBearerToken(request)
  if (token === undefined) {
    throw new ForgeGitHttpError(401, 'forge_git_unauthorized')
  }

  const session = await dependencies
    .makeTenantGitAuthStore(env)
    .authenticateGitAccessToken({
      nowIso,
      repositoryRef: match.repositoryRef,
      requiredScope: 'git:receive-pack',
      token,
    })
  if (session === undefined) {
    throw new ForgeGitHttpError(401, 'forge_git_unauthorized')
  }
  if (session.tenantRef !== match.tenantRef) {
    throw new ForgeGitHttpError(403, 'forge_git_tenant_forbidden')
  }

  return session
}

const pktLine = (payload: string): Uint8Array => {
  const payloadBytes = textEncoder.encode(payload)
  const length = (payloadBytes.byteLength + 4).toString(16).padStart(4, '0')
  const headerBytes = textEncoder.encode(length)
  const bytes = new Uint8Array(headerBytes.byteLength + payloadBytes.byteLength)
  bytes.set(headerBytes, 0)
  bytes.set(payloadBytes, headerBytes.byteLength)
  return bytes
}

const concatBytes = (chunks: ReadonlyArray<Uint8Array>): Uint8Array => {
  const bytes = new Uint8Array(
    chunks.reduce((total, chunk) => total + chunk.byteLength, 0),
  )
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  return bytes
}

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer => {
  const copy = new Uint8Array(bytes.byteLength)
  copy.set(bytes)
  return copy.buffer
}

const smartGitBody = (chunks: ReadonlyArray<Uint8Array>): Uint8Array =>
  concatBytes(chunks)

const flushPkt = textEncoder.encode('0000')

const buildInfoRefsAdvertisement = async (
  canonicalStore: ForgeGitCanonicalStore,
  tenantRef: string,
  repositoryRef: string,
) => {
  const refs = await canonicalStore.listRefs(tenantRef, repositoryRef, {
    limit: 500,
    state: 'active',
  })
  const capabilities = 'report-status object-format=sha1 agent=openagents-forge'
  const refLines =
    refs.length === 0
      ? [pktLine(`${zeroSha1} capabilities^{}\0${capabilities}\n`)]
      : refs.map((ref, index) =>
          pktLine(
            `${ref.object_id} ${ref.ref_name}${index === 0 ? `\0${capabilities}` : ''}\n`,
          ),
        )

  return smartGitBody([
    pktLine('# service=git-receive-pack\n'),
    flushPkt,
    ...refLines,
    flushPkt,
  ])
}

const receivePackStatusBody = (parsed: GitReceivePackRequest): Uint8Array =>
  smartGitBody([
    pktLine('unpack ok\n'),
    ...parsed.commands.map(command => pktLine(`ok ${command.refName}\n`)),
    flushPkt,
  ])

const safeRefPart = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'repo' : safe.slice(0, 96)
}

const compactIsoPart = (nowIso: string): string =>
  nowIso.replace(/[^0-9]/g, '').slice(0, 17)

const firstNonDeleteNewObjectId = (parsed: GitReceivePackRequest): string =>
  parsed.commands.find(command => command.action !== 'delete')?.newObjectId ??
  parsed.commands[0]!.newObjectId

const baseObjectId = (parsed: GitReceivePackRequest): string =>
  parsed.commands[0]!.oldObjectId

const sourceRefsForIntake = (
  parsed: GitReceivePackRequest,
  receivePackRef: string,
): ReadonlyArray<string> => [
  ...parsed.sourceRefs,
  'issue.public.github.OpenAgentsInc.openagents.6771',
  receivePackRef,
]

const assertSessionAllowsReceivePackRefs = (
  session: ForgeGitAccessTokenSession,
  parsed: GitReceivePackRequest,
): void => {
  if (session.refRestrictions.length === 0) {
    return
  }

  const allowed = new Set(session.refRestrictions)
  const forbidden = parsed.commands.find(command => !allowed.has(command.refName))

  if (forbidden !== undefined) {
    throw new ForgeGitHttpError(
      403,
      'forge_git_ref_forbidden',
      `Token ${session.tokenRef} is not scoped to ${forbidden.refName}.`,
    )
  }
}

const readReceivePackBody = async (
  request: Request,
  maxReceivePackBytes: number,
): Promise<ArrayBuffer> => {
  const contentLength = request.headers.get('content-length')
  if (
    contentLength !== null &&
    Number.isFinite(Number(contentLength)) &&
    Number(contentLength) > maxReceivePackBytes
  ) {
    throw new ForgeGitHttpError(413, 'forge_git_receive_pack_too_large')
  }

  const body = await request.arrayBuffer()
  if (body.byteLength > maxReceivePackBytes) {
    throw new ForgeGitHttpError(413, 'forge_git_receive_pack_too_large')
  }
  return body
}

const writeCoordinationRows = async (
  store: ForgeCoordinationStore,
  input: Readonly<{
    tenantRef: string
    repositoryRef: string
    issueRef: string
    changeRef: string
    prRef: string
    baseHead: string
    patchHead: string
    actorRef: string
    statusRef: string
    sourceRefs: ReadonlyArray<string>
    nowIso: string
  }>,
) => {
  await store.upsertIssue({
    issueRef: input.issueRef,
    priorityRef: 'prio.forge.git-intake',
    sourceRefs: input.sourceRefs,
    state: 'open',
    tenantRef: input.tenantRef,
    title: `Git intake for ${input.repositoryRef}`,
    nowIso: input.nowIso,
  })
  await store.upsertChange({
    baseHead: input.baseHead,
    blockerRefs: [],
    changeRef: input.changeRef,
    issueRef: input.issueRef,
    patchHead: input.patchHead,
    prRef: input.prRef,
    sourceRefs: input.sourceRefs,
    state: 'open',
    tenantRef: input.tenantRef,
    verificationRef: null,
    nowIso: input.nowIso,
  })
  await store.recordStatus({
    actorRef: input.actorRef,
    createdAt: input.nowIso,
    sourceRefs: input.sourceRefs,
    state: 'open',
    statusRef: input.statusRef,
    subjectRef: input.changeRef,
    tenantRef: input.tenantRef,
  })
}

const receivePackResponseHeaders = (
  input: Readonly<{
    receivePackRef: string
    packfileRef: string
    changeRef: string
  }>,
): Headers => {
  const headers = new Headers({
    'cache-control': 'no-store',
    'content-type': FORGE_GIT_RECEIVE_PACK_RESULT_CONTENT_TYPE,
    'x-openagents-forge-change-ref': input.changeRef,
    'x-openagents-forge-packfile-ref': input.packfileRef,
    'x-openagents-forge-receive-pack-ref': input.receivePackRef,
  })
  return headers
}

export const makeForgeGitIntakeRoutes = <Bindings>(
  dependencies: ForgeGitIntakeRouteDependencies<Bindings>,
) => ({
  routeForgeGitIntakeRequest(request: Request, env: Bindings) {
    const match = matchForgeGitRoute(request)
    if (match === undefined) {
      return undefined
    }

    if (match.kind === 'receive-pack' && request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return routeEffect(async () => {
      const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
      const session = await authenticateGit(
        request,
        env,
        match,
        dependencies,
        nowIso,
      )

      if (match.kind === 'advertise') {
        const body = await buildInfoRefsAdvertisement(
          dependencies.makeCanonicalStore(env),
          match.tenantRef,
          match.repositoryRef,
        )
        return new Response(arrayBufferFromBytes(body), {
          headers: {
            'cache-control': 'no-store',
            'content-type': FORGE_GIT_RECEIVE_PACK_ADVERTISEMENT_CONTENT_TYPE,
          },
          status: 200,
        })
      }

      const requestBody = await readReceivePackBody(
        request,
        dependencies.maxReceivePackBytes ?? maxReceivePackBytesDefault,
      )
      const parsed = parseGitReceivePackRequest(requestBody)
      assertSessionAllowsReceivePackRefs(session, parsed)
      const objectFormat = forgeGitPackfileObjectFormatForCapabilities(
        parsed.capabilities,
      )
      const canonicalStore = dependencies.makeCanonicalStore(env)

      await canonicalStore.preflightReceivePack({
        objectFormat,
        packfileBytes: parsed.packfileBytes,
        packfileSha256: parsed.packfileSha256,
        refUpdates: parsed.commands,
        repositoryRef: match.repositoryRef,
        tenantRef: match.tenantRef,
      })

      const repoPart = safeRefPart(match.repositoryRef)
      const digestPart = parsed.packfileSha256.slice(0, 16)
      const stampPart = compactIsoPart(nowIso)
      const receivePackRef = `receive-pack.forge.${repoPart}.${stampPart}.${digestPart}`
      const packfileRef = `packfile.forge.${repoPart}.${digestPart}`
      const changeRef = `change.forge.${repoPart}.${digestPart}`
      const issueRef = `issue.forge.git-intake.${repoPart}`
      const prRef = `pr.forge.git-intake.${repoPart}.${digestPart}`
      const statusRef = `status.forge.git-intake.${repoPart}.${digestPart}.open`
      const sourceRefs = sourceRefsForIntake(parsed, receivePackRef)

      const archive = await dependencies.makeArchiveStore(env).putPackfile({
        body: arrayBufferFromBytes(parsed.packfile),
        capabilities: parsed.capabilities,
        changeRef,
        objectFormat,
        packfileBytes: parsed.packfileBytes,
        packfileRef,
        packfileSha256: parsed.packfileSha256,
        receivePackRef,
        refUpdates: parsed.commands,
        repositoryRef: match.repositoryRef,
        sourceRefs,
        tenantRef: match.tenantRef,
        nowIso,
      })

      await canonicalStore.applyReceivePack({
        changeRef,
        objectFormat,
        packfileBytes: parsed.packfileBytes,
        packfileRef: archive.record.packfile_ref,
        packfileSha256: archive.record.packfile_sha256,
        receivePackRef,
        refUpdates: parsed.commands,
        repositoryRef: match.repositoryRef,
        sourceRefs,
        subjectRef: session.subjectRef,
        tenantRef: match.tenantRef,
        tokenRef: session.tokenRef,
        nowIso,
      })

      await writeCoordinationRows(dependencies.makeCoordinationStore(env), {
        actorRef: session.subjectRef,
        baseHead: baseObjectId(parsed),
        changeRef,
        issueRef,
        patchHead: firstNonDeleteNewObjectId(parsed),
        prRef,
        repositoryRef: match.repositoryRef,
        sourceRefs,
        statusRef,
        tenantRef: match.tenantRef,
        nowIso,
      })

      return new Response(arrayBufferFromBytes(receivePackStatusBody(parsed)), {
        headers: receivePackResponseHeaders({
          changeRef,
          packfileRef: archive.record.packfile_ref,
          receivePackRef,
        }),
        status: 200,
      })
    })
  },
})
