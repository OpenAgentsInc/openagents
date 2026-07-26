import { attachOwnerAttestation } from '@openagentsinc/sarah/community'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'
import * as nip19 from 'nostr-effect/nip19'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test } from 'vitest'

import { makeForgeInviteMembershipRoutes } from './forge-invite-membership-routes'
import { makeD1ForgeInviteMembershipStore } from './forge-invite-membership-store'
import { makeD1ForgeTenantGitAuthStore } from './forge-tenant-git-auth-store'
import type { TeamWorkspaceInviteRecord } from './team-workspace-invites'

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values
    return this
  }
  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Record<string, unknown>>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db
        .prepare(this.sql)
        .all(...(this.bound as never[])) as Array<T>,
    }
  }
  async run(): Promise<{
    meta: { changes: number }
    results: []
    success: true
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return {
      meta: { changes: Number(result.changes) },
      results: [],
      success: true,
    }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(new URL(`../migrations/${name}`, import.meta.url), 'utf8')

const nowIso = '2026-07-25T20:00:30.000Z'
const eventIso = '2026-07-25T20:00:00.000Z'
const tenantRef = 'tenant.openagents'
const session = {
  user: {
    email: 'owner@example.com',
    name: 'Owner',
    userId: 'github:owner',
  },
}

const acceptedInvite: TeamWorkspaceInviteRecord = {
  acceptedAt: eventIso,
  acceptedByUserId: session.user.userId,
  createdAt: '2026-07-25T19:00:00.000Z',
  emailMessageId: null,
  expiresAt: '2026-07-26T19:00:00.000Z',
  id: 'team_workspace_invite.owner',
  inviteeEmail: session.user.email,
  inviteeEmailNormalized: session.user.email,
  invitedByActorRef: 'forge_actor.inviter',
  lastSentAt: null,
  metadataJson: '{}',
  projectId: null,
  revokedAt: null,
  role: 'member',
  sendCount: 1,
  status: 'accepted',
  teamId: 'team_openagents_core',
  tokenHash: 'a'.repeat(64),
  updatedAt: eventIso,
}

const signedJsonRequest = (
  url: string,
  value: unknown,
  secret: Uint8Array,
): Request => {
  const body = JSON.stringify(value)
  const bytes = new TextEncoder().encode(body)
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor(Date.parse(eventIso) / 1_000),
      kind: 27_235,
      tags: [
        ['u', url],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(bytes)],
      ],
    },
    secret,
  )
  return new Request(url, {
    body,
    headers: {
      authorization: `Nostr ${btoa(JSON.stringify(event))}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })
}

const jsonRequest = (url: string, value: unknown): Request =>
  new Request(url, {
    body: JSON.stringify(value),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')

const makeHarness = async () => {
  const database = new DatabaseSync(':memory:')
  database.exec('PRAGMA foreign_keys = ON')
  database.exec(migration('0253_forge_tenant_git_access_tokens.sql'))
  database.exec(migration('0256_forge_tenant_isolation_posture.sql'))
  database.exec(
    "ALTER TABLE forge_git_access_tokens ADD COLUMN ref_restrictions_json TEXT NOT NULL DEFAULT '[]'",
  )
  database.exec(migration('0316_forge_invite_membership.sql'))
  const db = new SqliteD1(database) as unknown as D1Database
  const gitAuth = makeD1ForgeTenantGitAuthStore(db)
  await gitAuth.upsertTenant({
    displayName: 'OpenAgents',
    nowIso,
    tenantRef,
  })
  const membership = makeD1ForgeInviteMembershipStore(db)
  const routes = makeForgeInviteMembershipRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.append(
        'set-cookie',
        'oa_session=refreshed; Path=/; HttpOnly',
      )
      return response
    },
    gitServiceAuthorizationToken: () => 'forge-git-service-secret',
    makeGitAuthStore: () => gitAuth,
    makeMembershipStore: () => membership,
    nowIso: () => nowIso,
    readAcceptedInvite: (_env, inviteId, userId) =>
      Promise.resolve(
        inviteId === acceptedInvite.id &&
          userId === acceptedInvite.acceptedByUserId
          ? acceptedInvite
          : undefined,
      ),
    resolveTeamRefForTenant: (_env, requestedTenantRef) =>
      Promise.resolve(
        requestedTenantRef === tenantRef ? acceptedInvite.teamId : undefined,
      ),
    requireBrowserSession: () => Promise.resolve(session),
  })
  const run = (request: Request): Promise<Response> => {
    const effect = routes.routeForgeInviteMembershipRequest(
      request,
      {},
      {} as ExecutionContext,
    )
    if (effect === undefined) {
      throw new Error(`unmatched route: ${request.url}`)
    }
    return Effect.runPromise(effect)
  }
  return { gitAuth, membership, routes, run }
}

describe('Forge invite membership routes', () => {
  test('binds an accepted account to its signed npub and gates membership reads', async () => {
    const { run } = await makeHarness()
    const secret = generateSecretKey()
    const npub = nip19.npubEncode(getPublicKey(secret))
    const bindUrl = 'https://openagents.com/api/forge/membership/bind'
    const bind = await run(
      signedJsonRequest(
        bindUrl,
        {
          inviteRef: acceptedInvite.id,
          npub,
          tenantRef,
        },
        secret,
      ),
    )
    const bindBody = (await bind.json()) as {
      binding: { bindingRef: string; npub: string }
    }

    expect(bind.status).toBe(201)
    expect(bindBody.binding.npub).toBe(npub)
    expect(bind.headers.get('cache-control')).toContain('no-store')
    expect(bind.headers.get('set-cookie')).toContain('oa_session=refreshed')

    const collaborationRead = await run(
      new Request(
        'https://openagents.com/internal/forge/collaboration-read-authorize',
        {
          body: JSON.stringify({
            repositoryRef: 'repo.openagents.openagents',
            tenantRef,
          }),
          headers: {
            authorization: 'Bearer forge-git-service-secret',
            'content-type': 'application/json',
          },
          method: 'POST',
        },
      ),
    )
    expect(collaborationRead.status).toBe(200)
    expect(await collaborationRead.json()).toMatchObject({
      access: { canWrite: false, mode: 'member' },
      tenantRef,
    })

    const replay = await run(
      signedJsonRequest(
        bindUrl,
        {
          inviteRef: acceptedInvite.id,
          npub,
          tenantRef,
        },
        secret,
      ),
    )
    expect(replay.status).toBe(403)
    expect(await replay.json()).toMatchObject({
      error: 'forge_nip98_replayed',
    })

    const membership = await run(
      new Request(
        `https://openagents.com/api/forge/membership?tenantRef=${tenantRef}`,
      ),
    )
    expect(membership.status).toBe(200)
    expect(await membership.json()).toMatchObject({
      membership: {
        bindingRef: bindBody.binding.bindingRef,
        membershipState: 'active',
        tenantRef,
      },
    })
    expect(
      (
        await run(
          new Request(
            'https://openagents.com/api/forge/membership?tenantRef=tenant.external',
          ),
        )
      ).status,
    ).toBe(403)
  })

  test('denies an uninvited or revoked browser session', async () => {
    const { gitAuth, membership, routes, run } = await makeHarness()
    const uninvited = makeForgeInviteMembershipRoutes({
      makeGitAuthStore: () => gitAuth,
      makeMembershipStore: () => membership,
      nowIso: () => nowIso,
      readAcceptedInvite: () => Promise.resolve(undefined),
      resolveTeamRefForTenant: () => Promise.resolve(acceptedInvite.teamId),
      requireBrowserSession: () =>
        Promise.resolve({
          user: { email: 'other@example.com', userId: 'github:other' },
        }),
    })
    const request = new Request(
      `https://openagents.com/api/forge/membership?tenantRef=${tenantRef}`,
    )
    const uninvitedEffect = uninvited.routeForgeInviteMembershipRequest(
      request,
      {},
      {} as ExecutionContext,
    )
    if (uninvitedEffect === undefined) {
      throw new Error('membership route did not match')
    }
    expect((await Effect.runPromise(uninvitedEffect)).status).toBe(403)

    const signedOut = makeForgeInviteMembershipRoutes({
      makeGitAuthStore: () => gitAuth,
      makeMembershipStore: () => membership,
      nowIso: () => nowIso,
      readAcceptedInvite: () => Promise.resolve(undefined),
      resolveTeamRefForTenant: () => Promise.resolve(acceptedInvite.teamId),
      requireBrowserSession: () => Promise.resolve(undefined),
    })
    const signedOutEffect = signedOut.routeForgeInviteMembershipRequest(
      request,
      {},
      {} as ExecutionContext,
    )
    if (signedOutEffect === undefined) {
      throw new Error('membership route did not match')
    }
    expect((await Effect.runPromise(signedOutEffect)).status).toBe(401)

    const secret = generateSecretKey()
    const bindUrl = 'https://openagents.com/api/forge/membership/bind'
    expect(
      (
        await run(
          signedJsonRequest(
            bindUrl,
            {
              inviteRef: acceptedInvite.id,
              npub: nip19.npubEncode(getPublicKey(secret)),
              tenantRef,
            },
            secret,
          ),
        )
      ).status,
    ).toBe(201)
    const binding = await membership.readActorBindingByAccount(
      tenantRef,
      session.user.userId,
      'human',
    )
    if (binding === undefined) {
      throw new Error('missing bound member')
    }
    await membership.tombstoneMember({
      bindingRef: binding.bindingRef,
      burnReasonRef: 'forge.member.revoked',
      nowIso,
      sourceRefs: ['membership:revoked'],
      tenantRef,
    })
    expect((await run(request)).status).toBe(403)

    expect(
      routes.routeForgeInviteMembershipRequest(
        new Request('https://openagents.com/api/not-forge'),
        {},
        {} as ExecutionContext,
      ),
    ).toBeUndefined()
  })

  test('mints scoped credentials only for the invited owner and owned agent', async () => {
    const { gitAuth, membership, run } = await makeHarness()
    const ownerSecret = generateSecretKey()
    const ownerNpub = nip19.npubEncode(getPublicKey(ownerSecret))
    const bindUrl = 'https://openagents.com/api/forge/membership/bind'
    const bind = await run(
      signedJsonRequest(
        bindUrl,
        {
          inviteRef: acceptedInvite.id,
          npub: ownerNpub,
          tenantRef,
        },
        ownerSecret,
      ),
    )
    const bindBody = (await bind.json()) as {
      binding: { bindingRef: string }
    }
    expect(bind.status).toBe(201)

    const agentSecret = generateSecretKey()
    const agentPubkey = getPublicKey(agentSecret)
    const agentUrl = 'https://openagents.com/api/forge/membership/agents'
    const agent = await run(
      signedJsonRequest(
        agentUrl,
        {
          agentAccountRef: 'agent:fixture',
          displayName: 'Fixture agent',
          ownerAuthTag: attachOwnerAttestation({
            agentPubkey,
            operatorSeckeyHex: bytesToHex(ownerSecret),
          }),
          tenantRef,
        },
        agentSecret,
      ),
    )
    const agentBody = (await agent.json()) as {
      binding: { bindingRef: string }
    }
    expect(agent.status).toBe(201)

    const credentialUrl =
      'https://openagents.com/api/forge/membership/git-credentials'
    const minted = await run(
      jsonRequest(credentialUrl, {
        repositoryRef: 'repo.openagents.openagents',
        scopes: ['git:upload-pack', 'git:receive-pack'],
        subjectBindingRef: agentBody.binding.bindingRef,
        tenantRef,
      }),
    )
    const mintedBody = (await minted.json()) as {
      credential: {
        subjectBindingRef: string
        token: string
        tokenRef: string
      }
    }
    expect(minted.status, JSON.stringify(mintedBody)).toBe(201)
    expect(mintedBody.credential.subjectBindingRef).toBe(
      agentBody.binding.bindingRef,
    )
    expect(mintedBody.credential.token).toMatch(/^oa_forge_git_/)
    expect(
      await gitAuth.authenticateGitAccessToken({
        nowIso,
        repositoryRef: 'repo.openagents.openagents',
        requiredScope: 'git:receive-pack',
        token: mintedBody.credential.token,
      }),
    ).toMatchObject({
      subjectRef: agentBody.binding.bindingRef,
      tenantRef,
    })

    const internalAuthorize = () =>
      run(
        new Request('https://openagents.com/internal/forge/git-authorize', {
          body: JSON.stringify({
            repositoryRef: 'repo.openagents.openagents',
            requiredScope: 'git:receive-pack',
            tenantRef,
            transportAuthorization: `Bearer ${mintedBody.credential.token}`,
          }),
          headers: {
            authorization: 'Bearer forge-git-service-secret',
            'content-type': 'application/json',
          },
          method: 'POST',
        }),
      )
    const authorized = await internalAuthorize()
    expect(authorized.status).toBe(200)
    expect(await authorized.json()).toMatchObject({
      session: {
        subjectRef: agentBody.binding.bindingRef,
        tenantRef,
      },
    })

    const webReadAuthorize = () =>
      run(
        new Request(
          'https://openagents.com/internal/forge/web-read-authorize',
          {
            body: JSON.stringify({
              repositoryRef: 'repo.openagents.openagents',
              tenantRef,
            }),
            headers: {
              authorization: 'Bearer forge-git-service-secret',
              'content-type': 'application/json',
              cookie: 'oa_session=browser-member',
            },
            method: 'POST',
          },
        ),
      )
    const webReadAuthorized = await webReadAuthorize()
    expect(webReadAuthorized.status).toBe(200)
    expect(await webReadAuthorized.json()).toMatchObject({
      access: {
        canWrite: true,
        mode: 'member',
      },
      repository: {
        maintainers: [
          {
            displayName: session.user.name,
          },
        ],
        publicWebRead: false,
      },
    })
    const publicWebReadRoutes = makeForgeInviteMembershipRoutes({
      gitServiceAuthorizationToken: () => 'forge-git-service-secret',
      isPublicWebReadRepository: (
        _env,
        requestedTenantRef,
        repositoryRef,
      ) =>
        requestedTenantRef === tenantRef &&
        repositoryRef === 'repo.openagents.openagents',
      makeGitAuthStore: () => gitAuth,
      makeMembershipStore: () => membership,
      nowIso: () => nowIso,
      readAcceptedInvite: () => Promise.resolve(undefined),
      resolveTeamRefForTenant: () =>
        Promise.resolve(acceptedInvite.teamId),
      requireBrowserSession: () => Promise.resolve(undefined),
    })
    const publicWebReadRequest = new Request(
      'https://openagents.com/internal/forge/web-read-authorize',
      {
        body: JSON.stringify({
          repositoryRef: 'repo.openagents.openagents',
          tenantRef,
        }),
        headers: {
          authorization: 'Bearer forge-git-service-secret',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
    )
    const publicWebReadEffect =
      publicWebReadRoutes.routeForgeInviteMembershipRequest(
        publicWebReadRequest,
        {},
        {} as ExecutionContext,
      )
    if (publicWebReadEffect === undefined) {
      throw new Error('public web-read authorization route did not match')
    }
    const publicWebRead = await Effect.runPromise(publicWebReadEffect)
    expect(publicWebRead.status).toBe(200)
    expect(await publicWebRead.json()).toEqual({
      access: {
        canWrite: false,
        mode: 'public_web_read',
      },
      repository: {
        maintainers: [],
        publicWebRead: true,
      },
    })

    await membership.tombstoneMember({
      bindingRef: bindBody.binding.bindingRef,
      burnReasonRef: 'forge.member.revoked',
      nowIso,
      sourceRefs: ['membership:revoked'],
      tenantRef,
    })
    expect(
      (
        await run(
          jsonRequest(credentialUrl, {
            repositoryRef: 'repo.openagents.openagents',
            scopes: ['git:upload-pack'],
            subjectBindingRef: agentBody.binding.bindingRef,
            tenantRef,
          }),
        )
      ).status,
    ).toBe(403)
    const revokedReplay = await internalAuthorize()
    expect(revokedReplay.status).toBe(403)
    expect(await revokedReplay.json()).toMatchObject({
      error: 'forge_membership_tombstoned',
    })
    const revokedWebRead = await webReadAuthorize()
    expect(revokedWebRead.status).toBe(403)
    expect(await revokedWebRead.json()).toMatchObject({
      error: 'forge_membership_required',
    })
  })
})
