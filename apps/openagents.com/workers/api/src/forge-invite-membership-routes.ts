import type { ForgeGitAccessScope } from '@openagentsinc/forge-protocol'
import { Effect } from 'effect'
import { npubEncode } from 'nostr-effect/nip19'

import { sha256Hex, timingSafeEqual } from './agent-registration'
import {
  type ForgeInviteMembershipStore,
  forgeRoleRefsForTeamInvite,
} from './forge-invite-membership-store'
import {
  ForgeInvitePolicyError,
  decodeForgeNpub,
  makeForgeInvitePolicyAuthority,
  verifyForgeNip98Proof,
} from './forge-invite-policy'
import type { ForgeTenantGitAuthStore } from './forge-tenant-git-auth-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isRecord } from './json-boundary'
import { randomUuid } from './runtime-primitives'
import type {
  TeamWorkspaceInviteRecord,
  TeamWorkspaceInviteStore,
} from './team-workspace-invites'

export type ForgeBrowserSession = Readonly<{
  user: Readonly<{
    email?: string | null | undefined
    name?: string | null | undefined
    userId: string
  }>
}>

export type ForgeInviteMembershipRouteDependencies<
  Bindings,
  Session extends ForgeBrowserSession = ForgeBrowserSession,
> = Readonly<{
  appendRefreshedSessionCookies?: (
    response: Response,
    session: Session,
  ) => Response
  gitServiceAuthorizationToken?: (env: Bindings) => string | undefined
  isPublicWebReadRepository?: (
    env: Bindings,
    tenantRef: string,
    repositoryRef: string,
  ) => boolean | Promise<boolean>
  makeGitAuthStore: (env: Bindings) => ForgeTenantGitAuthStore
  makeMembershipStore: (env: Bindings) => ForgeInviteMembershipStore
  makeTeamWorkspaceInviteStore?: (env: Bindings) => TeamWorkspaceInviteStore
  nowIso: () => string
  readAcceptedInvite: (
    env: Bindings,
    inviteId: string,
    userId: string,
  ) => Promise<TeamWorkspaceInviteRecord | undefined>
  resolveTeamRefForTenant: (
    env: Bindings,
    tenantRef: string,
  ) => Promise<string | undefined>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

const membershipPath = '/api/forge/membership'
const bindHumanPath = '/api/forge/membership/bind'
const attachAgentPath = '/api/forge/membership/agents'
const gitCredentialsPath = '/api/forge/membership/git-credentials'
const bootstrapOwnerPath = '/api/forge/bootstrap-owner'
const gitAuthorizeInternalPath = '/internal/forge/git-authorize'
const relayAdmitInternalPath = '/internal/forge/relay-admit'
const collaborationReadAuthorizeInternalPath =
  '/internal/forge/collaboration-read-authorize'
const webReadAuthorizeInternalPath = '/internal/forge/web-read-authorize'
/**
 * The established tenant uses `tenant.openagents`. Forge's public Git namespace
 * is intentionally `openagents`, so the same core team can bootstrap its one
 * owner through the normal invite/proof path before it admits the real Omega
 * repository. Keep this an exact allowlist; this endpoint is never a general
 * tenant bootstrap mechanism.
 */
const bootstrapTenantRefs = new Set(['tenant.openagents', 'openagents'])
const bootstrapTeamTenantRef = 'tenant.openagents'
const bootstrapTeamRef = 'team_openagents_core'
const bootstrapAdminAuthorizationHeader = 'x-openagents-admin-authorization'

const errorResponse = (error: unknown): Response => {
  if (error instanceof ForgeInvitePolicyError) {
    const status =
      error.code === 'credential_missing' || error.code === 'credential_invalid'
        ? 401
        : error.code === 'binding_conflict' || error.code === 'key_burned'
          ? 409
          : 403
    return noStoreJsonResponse(
      { error: `forge_${error.code}`, reason: error.reason },
      { status },
    )
  }
  // Keep the public response generic, but leave enough redacted evidence in
  // Cloud Run to repair an unexpected bootstrap dependency failure. Do not log
  // the request, NIP-98 proof, token, or exception message here.
  console.warn('forge_membership_unexpected_error', {
    errorName: error instanceof Error ? error.name : typeof error,
  })
  return noStoreJsonResponse(
    { error: 'forge_membership_error' },
    { status: 500 },
  )
}

const routeEffect = (run: () => Promise<Response>) =>
  Effect.promise(async () => {
    try {
      return await run()
    } catch (error) {
      return errorResponse(error)
    }
  })

const unauthorized = (): Response =>
  noStoreJsonResponse({ error: 'forge_session_required' }, { status: 401 })

const forbidden = (): Response =>
  noStoreJsonResponse({ error: 'forge_membership_required' }, { status: 403 })

const readJsonBytes = async (
  request: Request,
): Promise<Readonly<{ bytes: Uint8Array; value: unknown }>> => {
  const bytes = new Uint8Array(await request.arrayBuffer())
  try {
    return {
      bytes,
      value: JSON.parse(new TextDecoder().decode(bytes)),
    }
  } catch {
    return { bytes, value: undefined }
  }
}

const requiredText = (
  body: Record<string, unknown>,
  key: string,
): string | undefined => {
  const value = body[key]
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const gitScopes = (
  value: unknown,
): ReadonlyArray<ForgeGitAccessScope> | undefined => {
  if (!Array.isArray(value) || value.length === 0) {
    return undefined
  }
  const scopes = [...new Set(value)]
  return scopes.every(
    scope =>
      scope === 'git:upload-pack' ||
      scope === 'git:receive-pack' ||
      scope === 'git:admin',
  )
    ? (scopes as Array<ForgeGitAccessScope>)
    : undefined
}

const roleAllowsGitScopes = (
  roleRefs: ReadonlyArray<string>,
  scopes: ReadonlyArray<ForgeGitAccessScope>,
): boolean =>
  roleRefs.includes('forge:admin') ||
  (roleRefs.includes('forge:member') &&
    scopes.every(
      scope => scope === 'git:upload-pack' || scope === 'git:receive-pack',
    )) ||
  (roleRefs.includes('forge:viewer') &&
    scopes.every(scope => scope === 'git:upload-pack'))

const bindingRefFor = async (
  actorKind: 'agent' | 'human',
  tenantRef: string,
  accountRef: string,
  pubkey: string,
): Promise<string> =>
  `forge_actor.${actorKind}.${(
    await sha256Hex(`${tenantRef}\n${accountRef}\n${pubkey}`)
  ).slice(0, 32)}`

const bearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  if (authorization === null) {
    return undefined
  }
  const [scheme, token] = authorization.trim().split(/\s+/, 2)
  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token
    : undefined
}

const bootstrapBody = (
  value: unknown,
): Readonly<{ npub: string; tenantRef: string }> | undefined => {
  if (!isRecord(value)) return undefined
  const keys = Object.keys(value).sort()
  if (keys.length !== 2 || keys[0] !== 'npub' || keys[1] !== 'tenantRef') {
    return undefined
  }
  const npub = requiredText(value, 'npub')
  const tenantRef = requiredText(value, 'tenantRef')
  return npub === undefined || tenantRef === undefined ? undefined : { npub, tenantRef }
}

const requestWithBootstrapAdminAuthorization = (request: Request): Request => {
  const authorization = request.headers.get(bootstrapAdminAuthorizationHeader)
  const headers = new Headers()
  if (authorization !== null) headers.set('authorization', authorization)
  return new Request(request.url, { headers, method: request.method })
}

export const requireForgeBrowserMember = async <
  Bindings,
  Session extends ForgeBrowserSession,
>(
  dependencies: ForgeInviteMembershipRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  tenantRef: string,
): Promise<
  | Readonly<{ _tag: 'Unauthorized' }>
  | Readonly<{ _tag: 'Forbidden'; session: Session }>
  | Readonly<{
      _tag: 'Member'
      bindingRef: string
      session: Session
    }>
> => {
  const session = await dependencies.requireBrowserSession(request, env, ctx)
  if (session === undefined) {
    return { _tag: 'Unauthorized' }
  }
  const binding = await dependencies
    .makeMembershipStore(env)
    .readActorBindingByAccount(tenantRef, session.user.userId, 'human')
  return binding?.membershipState === 'active'
    ? { _tag: 'Member', bindingRef: binding.bindingRef, session }
    : { _tag: 'Forbidden', session }
}

export const makeForgeInviteMembershipRoutes = <
  Bindings,
  Session extends ForgeBrowserSession = ForgeBrowserSession,
>(
  dependencies: ForgeInviteMembershipRouteDependencies<Bindings, Session>,
) => ({
  routeForgeInviteMembershipRequest(
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) {
    const path = new URL(request.url).pathname
    if (
      path !== membershipPath &&
      path !== bindHumanPath &&
      path !== attachAgentPath &&
      path !== gitCredentialsPath &&
      path !== bootstrapOwnerPath &&
      path !== gitAuthorizeInternalPath &&
      path !== relayAdmitInternalPath &&
      path !== collaborationReadAuthorizeInternalPath &&
      path !== webReadAuthorizeInternalPath
    ) {
      return undefined
    }

    let browserSession: Session | undefined
    return routeEffect(async () => {
      const store = dependencies.makeMembershipStore(env)
      const nowIso = dependencies.nowIso()
      if (path === bootstrapOwnerPath) {
        if (request.method !== 'POST') return methodNotAllowed(['POST'])
        if (
          dependencies.requireAdminApiToken === undefined ||
          dependencies.makeTeamWorkspaceInviteStore === undefined ||
          !(await dependencies.requireAdminApiToken(
            requestWithBootstrapAdminAuthorization(request),
            env,
          ))
        ) {
          return unauthorized()
        }
        const { bytes, value } = await readJsonBytes(request)
        const body = bootstrapBody(value)
        if (body === undefined || !bootstrapTenantRefs.has(body.tenantRef)) {
          return noStoreJsonResponse(
            { error: 'forge_owner_bootstrap_body_invalid' },
            { status: 400 },
          )
        }
        const authorization = request.headers.get('authorization')
        if (authorization?.startsWith('Nostr ') !== true) {
          throw new ForgeInvitePolicyError({
            code: 'credential_missing',
            reason: 'A signed NIP-98 owner proof is required.',
          })
        }
        const proof = await verifyForgeNip98Proof({
          authorization,
          body: bytes,
          method: request.method,
          nowIso,
          url: request.url,
        })
        const pubkey = decodeForgeNpub(body.npub)
        if (pubkey !== proof.actorPubkey) {
          throw new ForgeInvitePolicyError({
            code: 'credential_invalid',
            reason: 'The signed NIP-98 key does not match the submitted npub.',
          })
        }
        if (
          (await dependencies.resolveTeamRefForTenant(env, bootstrapTeamTenantRef)) !==
          bootstrapTeamRef
        ) {
          return forbidden()
        }
        // The public Forge namespace is distinct from the established product
        // tenant. Create its normal tenant record before its first owner
        // binding; do not rely on an out-of-band database seed.
        await dependencies.makeGitAuthStore(env).upsertTenant({
          displayName: 'OpenAgents Forge',
          nowIso,
          tenantRef: body.tenantRef,
        })
        if (
          (await store.readActorBindingByNostrPubkey(body.tenantRef, pubkey)) !==
          undefined
        ) {
          return noStoreJsonResponse(
            { error: 'forge_owner_bootstrap_already_completed' },
            { status: 409 },
          )
        }
        const identityDigest = await sha256Hex(
          `${body.tenantRef}\n${pubkey}\nforge-owner-bootstrap-v1`,
        )
        const accountRef = `forge_owner.${identityDigest.slice(0, 32)}`
        const inviteRef = `forge_owner_bootstrap.${identityDigest.slice(0, 32)}`
        const bindingRef = await bindingRefFor(
          'human',
          body.tenantRef,
          accountRef,
          pubkey,
        )
        const inviteResult = await dependencies
          .makeTeamWorkspaceInviteStore(env)
          .createOrRefreshInvite({
            email: `forge-owner-${identityDigest.slice(0, 24)}@bootstrap.openagents.invalid`,
            expiresInHours: 1,
            id: inviteRef,
            invitedByActorRef: 'operator:forge-owner-bootstrap',
            metadataJson: JSON.stringify({
              npub: body.npub,
              schema: 'openagents.forge.owner-bootstrap.v1',
              source: `nip98:${proof.eventId}`,
            }),
            role: 'admin',
            teamId: bootstrapTeamRef,
          })
        if (
          inviteResult._tag !== 'Created' &&
          inviteResult._tag !== 'Refreshed'
        ) {
          return noStoreJsonResponse(
            { error: 'forge_owner_bootstrap_invite_unavailable' },
            { status: 503 },
          )
        }
        const accepted = await dependencies
          .makeTeamWorkspaceInviteStore(env)
          .acceptInvite({
            sessionEmail: inviteResult.invite.inviteeEmail,
            token: inviteResult.token,
            userId: accountRef,
          })
        if (accepted._tag !== 'Accepted' && accepted._tag !== 'AlreadyAccepted') {
          return noStoreJsonResponse(
            { error: 'forge_owner_bootstrap_invite_acceptance_failed' },
            { status: 503 },
          )
        }
        if (
          !(await store.consumeNip98Replay({
            actorPubkey: proof.actorPubkey,
            authorityGeneration: 1,
            bodyDigest: proof.bodyDigest,
            canonicalPath: path,
            consumedAt: nowIso,
            consumptionRef: `forge_nip98_consumption.${proof.eventId}`,
            eventCreatedAt: proof.eventCreatedAt,
            eventId: proof.eventId,
            expiresAt: new Date(
              Date.parse(proof.eventCreatedAt) + 60_000,
            ).toISOString(),
            httpMethod: request.method,
            requestDigest: proof.requestDigest,
            result: 'accepted',
            tenantRef: body.tenantRef,
          }))
        ) {
          throw new ForgeInvitePolicyError({
            code: 'nip98_replayed',
            reason: 'This NIP-98 event was already consumed.',
          })
        }
        const binding = await store.bindHuman({
          acceptedAt: accepted.invite.acceptedAt ?? nowIso,
          accountRef,
          bindingEventCreatedAt: proof.eventCreatedAt,
          bindingEventId: proof.eventId,
          bindingRef,
          displayName: 'Forge owner',
          expiresAt: accepted.invite.expiresAt,
          inviteBindingRef: `forge_invite_binding.${accepted.invite.id}`,
          inviteDigest: accepted.invite.tokenHash,
          inviteRef: accepted.invite.id,
          invitedSubjectRef: accountRef,
          inviterBindingRef: accepted.invite.invitedByActorRef,
          issuedAt: accepted.invite.createdAt,
          nostrPubkey: pubkey,
          provenanceSourceRefs: [
            `team_workspace_invite:${accepted.invite.id}`,
            `nip98:${proof.eventId}`,
            'forge_owner_bootstrap:v1',
          ],
          roleRefs: forgeRoleRefsForTeamInvite(accepted.invite.role),
          teamRef: accepted.invite.teamId,
          tenantRef: body.tenantRef,
        })
        return noStoreJsonResponse(
          {
            receipt: {
              bindingRef: binding.bindingRef,
              inviteBindingRef: `forge_invite_binding.${accepted.invite.id}`,
              inviteRef: accepted.invite.id,
              npub: body.npub,
              replayConsumptionRef: `forge_nip98_consumption.${proof.eventId}`,
              schema: 'openagents.forge.owner-bootstrap-receipt.v1',
              tenantRef: body.tenantRef,
            },
          },
          { status: 201 },
        )
      }
      if (path === collaborationReadAuthorizeInternalPath) {
        if (request.method !== 'POST') return methodNotAllowed(['POST'])
        const configuredToken = dependencies.gitServiceAuthorizationToken?.(env)
        const presentedToken = bearerToken(request)
        if (
          configuredToken === undefined ||
          presentedToken === undefined ||
          !(await timingSafeEqual(presentedToken, configuredToken))
        ) {
          return unauthorized()
        }
        const { value } = await readJsonBytes(request.clone())
        if (!isRecord(value)) {
          return noStoreJsonResponse(
            { error: 'forge_collaboration_authorization_body_invalid' },
            { status: 400 },
          )
        }
        const tenantRef = requiredText(value, 'tenantRef')
        const repositoryRef = requiredText(value, 'repositoryRef')
        if (tenantRef === undefined || repositoryRef === undefined) {
          return noStoreJsonResponse(
            { error: 'forge_collaboration_authorization_body_invalid' },
            { status: 400 },
          )
        }
        const member = await requireForgeBrowserMember(
          dependencies,
          request,
          env,
          ctx,
          tenantRef,
        )
        if (member._tag === 'Unauthorized') return unauthorized()
        if (member._tag === 'Forbidden') return forbidden()
        const binding = await store.readActorBindingByAccount(
          tenantRef,
          member.session.user.userId,
          'human',
        )
        if (
          binding === undefined ||
          !binding.roleRefs.some(
            role =>
              role === 'forge:viewer' ||
              role === 'forge:member' ||
              role === 'forge:admin',
          )
        ) {
          return forbidden()
        }
        return noStoreJsonResponse({
          access: { mode: 'member', canWrite: false },
          repositoryRef,
          tenantRef,
        })
      }
      if (path === relayAdmitInternalPath) {
        if (request.method !== 'POST') return methodNotAllowed(['POST'])
        const configuredToken = dependencies.gitServiceAuthorizationToken?.(env)
        const presentedToken = bearerToken(request)
        if (
          configuredToken === undefined ||
          presentedToken === undefined ||
          !(await timingSafeEqual(presentedToken, configuredToken))
        ) {
          return unauthorized()
        }
        const { value } = await readJsonBytes(request)
        if (!isRecord(value)) {
          return noStoreJsonResponse(
            { error: 'forge_relay_admission_body_invalid' },
            { status: 400 },
          )
        }
        const tenantRef = requiredText(value, 'tenantRef')
        const pubkey = requiredText(value, 'pubkey')
        if (
          tenantRef === undefined ||
          pubkey === undefined ||
          !/^[0-9a-f]{64}$/u.test(pubkey)
        ) {
          return noStoreJsonResponse(
            { error: 'forge_relay_admission_body_invalid' },
            { status: 400 },
          )
        }
        const binding = await store.readActorBindingByNostrPubkey(tenantRef, pubkey)
        if (
          binding === undefined ||
          binding.membershipState !== 'active' ||
          !binding.roleRefs.some(
            role => role === 'forge:member' || role === 'forge:admin',
          )
        ) {
          return forbidden()
        }
        return noStoreJsonResponse({
          bindingRef: binding.bindingRef,
          tenantRef,
        })
      }
      if (
        path === gitAuthorizeInternalPath ||
        path === webReadAuthorizeInternalPath
      ) {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }
        const configuredToken = dependencies.gitServiceAuthorizationToken?.(env)
        const presentedToken = bearerToken(request)
        if (
          configuredToken === undefined ||
          presentedToken === undefined ||
          !(await timingSafeEqual(presentedToken, configuredToken))
        ) {
          return unauthorized()
        }
        const { value } = await readJsonBytes(request)
        if (!isRecord(value)) {
          return noStoreJsonResponse(
            { error: 'forge_git_authorization_body_invalid' },
            { status: 400 },
          )
        }
        const tenantRef = requiredText(value, 'tenantRef')
        const repositoryRef = requiredText(value, 'repositoryRef')
        if (path === webReadAuthorizeInternalPath) {
          if (tenantRef === undefined || repositoryRef === undefined) {
            return noStoreJsonResponse(
              { error: 'forge_web_read_authorization_body_invalid' },
              { status: 400 },
            )
          }
          const session = await dependencies.requireBrowserSession(
            request,
            env,
            ctx,
          )
          const binding =
            session === undefined
              ? undefined
              : await store.readActorBindingByAccount(
                  tenantRef,
                  session.user.userId,
                  'human',
                )
          if (binding?.membershipState === 'active') {
            return noStoreJsonResponse({
              access: {
                canWrite:
                  binding.roleRefs.includes('forge:admin') ||
                  binding.roleRefs.includes('forge:member'),
                mode: 'member',
              },
              repository: {
                maintainers: [
                  {
                    displayName: binding.displayName,
                    ...(binding.nostrPubkey === null
                      ? {}
                      : { nostrPubkey: binding.nostrPubkey }),
                  },
                ],
                publicWebRead: await Promise.resolve(
                  dependencies.isPublicWebReadRepository?.(
                    env,
                    tenantRef,
                    repositoryRef,
                  ) ?? false,
                ),
              },
            })
          }
          const publicWebRead = await Promise.resolve(
            dependencies.isPublicWebReadRepository?.(
              env,
              tenantRef,
              repositoryRef,
            ) ?? false,
          )
          if (publicWebRead) {
            return noStoreJsonResponse({
              access: {
                canWrite: false,
                mode: 'public_web_read',
              },
              repository: {
                maintainers: [],
                publicWebRead: true,
              },
            })
          }
          return session === undefined ? unauthorized() : forbidden()
        }
        const transportAuthorization = requiredText(
          value,
          'transportAuthorization',
        )
        const requiredScope = value.requiredScope
        if (
          tenantRef === undefined ||
          repositoryRef === undefined ||
          transportAuthorization === undefined ||
          (requiredScope !== 'git:upload-pack' &&
            requiredScope !== 'git:receive-pack')
        ) {
          return noStoreJsonResponse(
            { error: 'forge_git_authorization_body_invalid' },
            { status: 400 },
          )
        }
        const session = await makeForgeInvitePolicyAuthority({
          policyStore: store,
          tokenStore: dependencies.makeGitAuthStore(env),
        }).authorizeGitTransport({
          nowIso,
          repositoryRef,
          request: new Request(
            `https://forge-git.internal/${encodeURIComponent(tenantRef)}/${encodeURIComponent(repositoryRef)}`,
            { headers: { authorization: transportAuthorization } },
          ),
          requiredScope,
          tenantRef,
        })
        return noStoreJsonResponse({ session })
      }

      const session = await dependencies.requireBrowserSession(
        request,
        env,
        ctx,
      )
      if (session === undefined) {
        return unauthorized()
      }
      browserSession = session

      if (path === membershipPath) {
        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }
        const tenantRef =
          new URL(request.url).searchParams.get('tenantRef')?.trim() ?? ''
        if (tenantRef === '') {
          return noStoreJsonResponse(
            { error: 'forge_tenant_required' },
            { status: 400 },
          )
        }
        if (
          (await dependencies.resolveTeamRefForTenant(env, tenantRef)) ===
          undefined
        ) {
          return forbidden()
        }
        const binding = await store.readActorBindingByAccount(
          tenantRef,
          session.user.userId,
          'human',
        )
        if (binding?.membershipState !== 'active') {
          return forbidden()
        }
        return noStoreJsonResponse({
          membership: {
            actorKind: binding.actorKind,
            bindingRef: binding.bindingRef,
            membershipState: binding.membershipState,
            roleRefs: binding.roleRefs,
            tenantRef: binding.tenantRef,
          },
        })
      }

      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }
      const { bytes, value } = await readJsonBytes(request)
      if (!isRecord(value)) {
        return noStoreJsonResponse(
          { error: 'forge_membership_body_invalid' },
          { status: 400 },
        )
      }
      const tenantRef = requiredText(value, 'tenantRef')
      if (tenantRef === undefined) {
        return noStoreJsonResponse(
          { error: 'forge_tenant_required' },
          { status: 400 },
        )
      }
      const teamRef = await dependencies.resolveTeamRefForTenant(env, tenantRef)
      if (teamRef === undefined) {
        return forbidden()
      }
      if (path === gitCredentialsPath) {
        const repositoryRef = requiredText(value, 'repositoryRef')
        const requestedScopes = gitScopes(value.scopes)
        const subjectBindingRef =
          requiredText(value, 'subjectBindingRef') ??
          (
            await store.readActorBindingByAccount(
              tenantRef,
              session.user.userId,
              'human',
            )
          )?.bindingRef
        const refRestrictions = value.refRestrictions
        if (
          repositoryRef === undefined ||
          requestedScopes === undefined ||
          subjectBindingRef === undefined ||
          (refRestrictions !== undefined &&
            (!Array.isArray(refRestrictions) ||
              !refRestrictions.every(item => typeof item === 'string')))
        ) {
          return noStoreJsonResponse(
            { error: 'forge_git_credential_request_invalid' },
            { status: 400 },
          )
        }
        const owner = await store.readActorBindingByAccount(
          tenantRef,
          session.user.userId,
          'human',
        )
        const subject = await store.readActorBindingByRef(
          tenantRef,
          subjectBindingRef,
        )
        if (
          owner?.membershipState !== 'active' ||
          subject?.membershipState !== 'active' ||
          (subject.bindingRef !== owner.bindingRef &&
            subject.ownerBindingRef !== owner.bindingRef) ||
          !roleAllowsGitScopes(subject.roleRefs, requestedScopes)
        ) {
          return forbidden()
        }
        const tokenRef = `forge_git_token.${randomUuid()}`
        const credential = await dependencies
          .makeGitAuthStore(env)
          .mintGitAccessToken({
            expiresAt: new Date(
              Date.parse(nowIso) + 8 * 60 * 60 * 1_000,
            ).toISOString(),
            nowIso,
            refRestrictions:
              refRestrictions === undefined
                ? []
                : (refRestrictions as Array<string>),
            repositoryRef,
            scopes: requestedScopes,
            sourceRefs: [
              `forge_actor_binding:${subject.bindingRef}`,
              `team:${teamRef}`,
            ],
            subjectRef: subject.bindingRef,
            tenantRef,
            tokenRef,
          })
        return noStoreJsonResponse(
          {
            credential: {
              expiresAt: credential.record.expires_at,
              repositoryRef,
              scopes: credential.scopes.map(scope => scope.scope),
              subjectBindingRef: subject.bindingRef,
              token: credential.token,
              tokenRef,
            },
          },
          { status: 201 },
        )
      }
      const authorization = request.headers.get('authorization')
      if (authorization?.startsWith('Nostr ') !== true) {
        throw new ForgeInvitePolicyError({
          code: 'credential_missing',
          reason: 'A signed NIP-98 identity proof is required.',
        })
      }
      const proof = await verifyForgeNip98Proof({
        authorization,
        body: bytes,
        method: request.method,
        nowIso,
        url: request.url,
      })

      if (path === bindHumanPath) {
        const inviteRef = requiredText(value, 'inviteRef')
        const npub = requiredText(value, 'npub')
        if (inviteRef === undefined || npub === undefined) {
          return noStoreJsonResponse(
            { error: 'forge_invite_and_npub_required' },
            { status: 400 },
          )
        }
        const pubkey = decodeForgeNpub(npub)
        if (pubkey !== proof.actorPubkey) {
          throw new ForgeInvitePolicyError({
            code: 'credential_invalid',
            reason: 'The signed NIP-98 key does not match the submitted npub.',
          })
        }
        const invite = await dependencies.readAcceptedInvite(
          env,
          inviteRef,
          session.user.userId,
        )
        if (
          invite === undefined ||
          invite.acceptedAt === null ||
          invite.teamId !== teamRef
        ) {
          return forbidden()
        }
        const bindingRef = await bindingRefFor(
          'human',
          tenantRef,
          session.user.userId,
          pubkey,
        )
        if (
          !(await store.consumeNip98Replay({
            actorPubkey: proof.actorPubkey,
            authorityGeneration: 1,
            bodyDigest: proof.bodyDigest,
            canonicalPath: path,
            consumedAt: nowIso,
            consumptionRef: `forge_nip98_consumption.${proof.eventId}`,
            eventCreatedAt: proof.eventCreatedAt,
            eventId: proof.eventId,
            expiresAt: new Date(
              Date.parse(proof.eventCreatedAt) + 60_000,
            ).toISOString(),
            httpMethod: request.method,
            requestDigest: proof.requestDigest,
            result: 'accepted',
            tenantRef,
          }))
        ) {
          throw new ForgeInvitePolicyError({
            code: 'nip98_replayed',
            reason: 'This NIP-98 event was already consumed.',
          })
        }
        const binding = await store.bindHuman({
          acceptedAt: invite.acceptedAt,
          accountRef: session.user.userId,
          bindingEventCreatedAt: proof.eventCreatedAt,
          bindingEventId: proof.eventId,
          bindingRef,
          displayName:
            session.user.name?.trim() || session.user.email?.trim() || 'Member',
          expiresAt: invite.expiresAt,
          inviteBindingRef: `forge_invite_binding.${invite.id}`,
          inviteDigest: invite.tokenHash,
          inviteRef: invite.id,
          invitedSubjectRef: session.user.userId,
          inviterBindingRef: invite.invitedByActorRef,
          issuedAt: invite.createdAt,
          nostrPubkey: pubkey,
          provenanceSourceRefs: [
            `team_workspace_invite:${invite.id}`,
            `nip98:${proof.eventId}`,
          ],
          roleRefs: forgeRoleRefsForTeamInvite(invite.role),
          teamRef: invite.teamId,
          tenantRef,
        })
        return noStoreJsonResponse(
          {
            binding: {
              actorKind: binding.actorKind,
              bindingRef: binding.bindingRef,
              membershipState: binding.membershipState,
              npub: nip19For(binding.nostrPubkey),
              roleRefs: binding.roleRefs,
              tenantRef: binding.tenantRef,
            },
          },
          { status: 201 },
        )
      }

      const owner = await store.readActorBindingByAccount(
        tenantRef,
        session.user.userId,
        'human',
      )
      if (owner?.membershipState !== 'active' || owner.nostrPubkey === null) {
        return forbidden()
      }
      const displayName = requiredText(value, 'displayName')
      const agentAccountRef = requiredText(value, 'agentAccountRef')
      const ownerAuthTag = value.ownerAuthTag
      if (
        displayName === undefined ||
        agentAccountRef === undefined ||
        !Array.isArray(ownerAuthTag) ||
        !ownerAuthTag.every(item => typeof item === 'string')
      ) {
        return noStoreJsonResponse(
          { error: 'forge_agent_binding_body_invalid' },
          { status: 400 },
        )
      }
      const bindingRef = await bindingRefFor(
        'agent',
        tenantRef,
        agentAccountRef,
        proof.actorPubkey,
      )
      if (
        !(await store.consumeNip98Replay({
          actorPubkey: proof.actorPubkey,
          authorityGeneration: owner.bindingGeneration,
          bodyDigest: proof.bodyDigest,
          canonicalPath: path,
          consumedAt: nowIso,
          consumptionRef: `forge_nip98_consumption.${proof.eventId}`,
          eventCreatedAt: proof.eventCreatedAt,
          eventId: proof.eventId,
          expiresAt: new Date(
            Date.parse(proof.eventCreatedAt) + 60_000,
          ).toISOString(),
          httpMethod: request.method,
          requestDigest: proof.requestDigest,
          result: 'accepted',
          tenantRef,
        }))
      ) {
        throw new ForgeInvitePolicyError({
          code: 'nip98_replayed',
          reason: 'This NIP-98 event was already consumed.',
        })
      }
      const binding = await store.attachAgent({
        accountRef: agentAccountRef,
        bindingEventCreatedAt: proof.eventCreatedAt,
        bindingEventId: proof.eventId,
        bindingRef,
        displayName,
        nostrPubkey: proof.actorPubkey,
        nowIso,
        ownerAuthTag,
        ownerBindingRef: owner.bindingRef,
        sourceRefs: [`nip98:${proof.eventId}`],
        tenantRef,
      })
      return noStoreJsonResponse(
        {
          binding: {
            actorKind: binding.actorKind,
            bindingRef: binding.bindingRef,
            membershipState: binding.membershipState,
            ownerBindingRef: binding.ownerBindingRef,
            roleRefs: binding.roleRefs,
            tenantRef: binding.tenantRef,
          },
        },
        { status: 201 },
      )
    }).pipe(
      Effect.map(response =>
        browserSession === undefined ||
        dependencies.appendRefreshedSessionCookies === undefined
          ? response
          : dependencies.appendRefreshedSessionCookies(
              response,
              browserSession,
            ),
      ),
    )
  },
})

const nip19For = (pubkey: string | null): string | null => {
  if (pubkey === null) {
    return null
  }
  // The canonical actor row stores one hex key. npub is a derived projection.
  return npubEncode(pubkey)
}
