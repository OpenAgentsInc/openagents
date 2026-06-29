import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  EmailAddress,
  ResendEmailSender,
  WorkerSecret,
  type ResendEmailConfig,
} from './config'
import type {
  EmailLedgerSendResult,
  PrivateWorkspaceInviteEmailInput,
} from './email'
import { makeTeamWorkspaceInviteRoutes } from './team-workspace-invite-routes'
import {
  type TeamWorkspaceInviteAcceptInput,
  type TeamWorkspaceInviteAcceptResult,
  type TeamWorkspaceInviteCreateInput,
  type TeamWorkspaceInviteCreateResult,
  type TeamWorkspaceInviteRecord,
  type TeamWorkspaceInviteRole,
  type TeamWorkspaceInviteStore,
  normalizeTeamWorkspaceInviteEmail,
} from './team-workspace-invites'

const nowIso = '2026-06-16T12:00:00.000Z'
const futureIso = '2026-06-19T12:00:00.000Z'
const pastIso = '2026-06-15T12:00:00.000Z'

type Bindings = Readonly<{
  resend?: ResendEmailConfig | undefined
  operatorToken?: string
  session?: Readonly<{
    user: Readonly<{
      email: string
      name: string
      userId: string
    }>
  }>
}>

const ctx = {
  passThroughOnException: () => {},
  waitUntil: () => {},
} as unknown as ExecutionContext

const inviteRecord = (
  input: Readonly<{
    email: string
    expiresAt?: string
    id: string
    projectId?: string | null
    role?: TeamWorkspaceInviteRole
    status?: TeamWorkspaceInviteRecord['status']
    teamId: string
    token: string
  }>,
): TeamWorkspaceInviteRecord => ({
  acceptedAt: null,
  acceptedByUserId: null,
  createdAt: nowIso,
  emailMessageId: null,
  expiresAt: input.expiresAt ?? futureIso,
  id: input.id,
  inviteeEmail: input.email,
  inviteeEmailNormalized:
    normalizeTeamWorkspaceInviteEmail(input.email) ?? input.email,
  invitedByActorRef: 'operator:admin_api',
  lastSentAt: null,
  metadataJson: '{}',
  projectId: input.projectId ?? null,
  revokedAt: null,
  role: input.role ?? 'member',
  sendCount: 0,
  status: input.status ?? 'pending',
  teamId: input.teamId,
  tokenHash: `hash:${input.token}`,
  updatedAt: nowIso,
})

class MemoryInviteStore implements TeamWorkspaceInviteStore {
  readonly invites = new Map<string, TeamWorkspaceInviteRecord>()
  readonly tokenByInviteId = new Map<string, string>()
  readonly memberships = new Map<
    string,
    Readonly<{ role: string; teamId: string; userId: string }>
  >()
  tokenCounter = 0

  createOrRefreshInvite = async (
    input: TeamWorkspaceInviteCreateInput,
  ): Promise<TeamWorkspaceInviteCreateResult> => {
    const normalized = normalizeTeamWorkspaceInviteEmail(input.email)

    if (normalized === undefined) {
      return { _tag: 'InvalidEmail' }
    }

    if (input.teamId === 'missing-team') {
      return { _tag: 'TeamNotFound' }
    }

    if (input.projectId === 'missing-project') {
      return { _tag: 'ProjectNotFound' }
    }

    const token = `token-${this.tokenCounter++}`
    const existing = [...this.invites.values()].find(
      invite =>
        invite.teamId === input.teamId &&
        invite.projectId === (input.projectId ?? null) &&
        invite.inviteeEmailNormalized === normalized &&
        invite.status === 'pending',
    )

    if (existing !== undefined) {
      const refreshed = {
        ...existing,
        expiresAt: input.expiresAt ?? futureIso,
        role: input.role ?? existing.role,
        tokenHash: `hash:${token}`,
        updatedAt: nowIso,
      }
      this.invites.set(refreshed.id, refreshed)
      this.tokenByInviteId.set(refreshed.id, token)

      return { _tag: 'Refreshed', invite: refreshed, token }
    }

    const invite = inviteRecord({
      email: input.email,
      id: input.id ?? `invite-${this.invites.size}`,
      teamId: input.teamId,
      token,
      ...(input.expiresAt === undefined ? {} : { expiresAt: input.expiresAt }),
      ...(input.projectId === undefined ? {} : { projectId: input.projectId }),
      ...(input.role === undefined ? {} : { role: input.role }),
    })
    this.invites.set(invite.id, invite)
    this.tokenByInviteId.set(invite.id, token)

    return { _tag: 'Created', invite, token }
  }

  acceptInvite = async (
    input: TeamWorkspaceInviteAcceptInput,
  ): Promise<TeamWorkspaceInviteAcceptResult> => {
    const invite = [...this.invites.values()].find(
      candidate => candidate.tokenHash === `hash:${input.token}`,
    )

    if (invite === undefined) {
      return { _tag: 'NotFound' }
    }

    const membershipId = `membership-${invite.teamId}-${input.userId}`

    if (invite.status === 'accepted') {
      return invite.acceptedByUserId === input.userId
        ? { _tag: 'AlreadyAccepted', invite, membershipId }
        : { _tag: 'InviteUnavailable', status: invite.status }
    }

    if (invite.status !== 'pending') {
      return { _tag: 'InviteUnavailable', status: invite.status }
    }

    if (Date.parse(invite.expiresAt) <= Date.parse(nowIso)) {
      const expired = { ...invite, status: 'expired' as const }
      this.invites.set(invite.id, expired)

      return { _tag: 'Expired', invite: expired }
    }

    if (
      normalizeTeamWorkspaceInviteEmail(input.sessionEmail) !==
      invite.inviteeEmailNormalized
    ) {
      return { _tag: 'WrongUser' }
    }

    const accepted = {
      ...invite,
      acceptedAt: nowIso,
      acceptedByUserId: input.userId,
      status: 'accepted' as const,
      updatedAt: nowIso,
    }
    this.invites.set(invite.id, accepted)
    this.memberships.set(membershipId, {
      role: invite.role,
      teamId: invite.teamId,
      userId: input.userId,
    })

    return { _tag: 'Accepted', invite: accepted, membershipId }
  }

  recordEmailAttempt = async (input: {
    attemptedAt: string
    emailMessageId: string
    inviteId: string
  }): Promise<TeamWorkspaceInviteRecord | undefined> => {
    const invite = this.invites.get(input.inviteId)

    if (invite === undefined) {
      return undefined
    }

    const updated = {
      ...invite,
      emailMessageId: input.emailMessageId,
      lastSentAt: input.attemptedAt,
      sendCount: invite.sendCount + 1,
      updatedAt: input.attemptedAt,
    }
    this.invites.set(updated.id, updated)

    return updated
  }
}

const testResendConfig = (): ResendEmailConfig => ({
  apiKey: Redacted.make(WorkerSecret.make('re_test')),
  fromEmail: ResendEmailSender.make('OpenAgents <ops@openagents.com>'),
  replyToEmail: EmailAddress.make('ops@openagents.com'),
})

const makeRoutes = (
  store: MemoryInviteStore,
  options: Readonly<{
    emailResult?: EmailLedgerSendResult | undefined
    emailCalls?: Array<PrivateWorkspaceInviteEmailInput> | undefined
  }> = {},
) =>
  makeTeamWorkspaceInviteRoutes<Bindings>({
    appendRefreshedSessionCookies: response => response,
    appOrigin: () => 'https://openagents.com',
    getResendEmailConfig: env => env.resend,
    makeStore: () => store,
    nowIso: () => nowIso,
    requireAdminApiToken: async request =>
      request.headers.get('authorization') === 'Bearer admin-token',
    requireBrowserSession: async (_request, env) => env.session,
    sendInviteEmailWithLedger: (_env, _config, input) => {
      options.emailCalls?.push(input)

      return Effect.succeed(
        options.emailResult ?? {
          emailMessageId: 'email_msg_invite',
          ok: true,
          providerMessageId: 'resend_invite',
        },
      )
    },
  })

const routeRequest = async (
  store: MemoryInviteStore,
  request: Request,
  env: Bindings = {},
  options: Parameters<typeof makeRoutes>[1] = {},
): Promise<Response> => {
  const effect = makeRoutes(store, options).routeTeamWorkspaceInviteRequest(
    request,
    env,
    ctx,
  )

  if (effect === undefined) {
    throw new Error('Expected team workspace invite route to handle request.')
  }

  return Effect.runPromise(effect)
}

const operatorCreateRequest = (body: unknown, token = 'admin-token'): Request =>
  new Request('https://openagents.com/api/operator/team-workspace-invites', {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

describe('team workspace invite routes', () => {
  test('requires an operator token to create a private workspace invite', async () => {
    const response = await routeRequest(
      new MemoryInviteStore(),
      operatorCreateRequest(
        { email: 'teammate@example.com', teamId: 'team_1' },
        'bad',
      ),
    )

    expect(response.status).toBe(401)
  })

  test('creates an operator invite without echoing the target email', async () => {
    const store = new MemoryInviteStore()
    const response = await routeRequest(
      store,
      operatorCreateRequest({
        email: 'Teammate@Example.COM',
        projectId: 'project_1',
        role: 'member',
        teamId: 'team_1',
      }),
      { resend: testResendConfig() },
    )
    const body = await response.json<Record<string, unknown>>()
    const text = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body.acceptUrl).toBe(
      'https://openagents.com/api/team-workspace-invites/accept?token=token-0',
    )
    expect(text).not.toContain('Teammate@Example.COM')
    expect(text).not.toContain('teammate@example.com')
    expect(body.invite).toMatchObject({
      emailMessageId: null,
      id: 'invite-0',
      projectId: 'project_1',
      role: 'member',
      status: 'pending',
      teamId: 'team_1',
    })
    expect(body.email).toMatchObject({
      emailMessageId: 'email_msg_invite',
      providerMessageId: 'resend_invite',
      status: 'accepted',
    })
    expect(store.invites.get('invite-0')).toMatchObject({
      emailMessageId: 'email_msg_invite',
      lastSentAt: nowIso,
      sendCount: 1,
    })
  })

  test('creates an invite with a safe missing-email-config fallback', async () => {
    const store = new MemoryInviteStore()
    const response = await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        teamId: 'team_1',
      }),
    )
    const body = await response.json<Record<string, unknown>>()

    expect(response.status).toBe(201)
    expect(body.email).toMatchObject({
      errorName: 'email_config_missing',
      status: 'missing_config',
    })
    expect(JSON.stringify(body)).not.toContain('teammate@example.com')
    expect(store.invites.get('invite-0')).toMatchObject({
      emailMessageId: null,
      sendCount: 0,
    })
  })

  test('reports provider invite-email failures without failing invite creation', async () => {
    const store = new MemoryInviteStore()
    const emailCalls: Array<PrivateWorkspaceInviteEmailInput> = []
    const response = await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        recipientDisplayName: 'Teammate <One>',
        teamId: 'team_1',
        workspaceLabel: 'Private <Workspace>',
      }),
      { resend: testResendConfig() },
      {
        emailCalls,
        emailResult: {
          emailMessageId: 'email_msg_failed',
          errorMessage: 'Domain is not verified.',
          errorName: 'validation_error',
          ok: false,
        },
      },
    )
    const body = await response.json<Record<string, unknown>>()

    expect(response.status).toBe(201)
    expect(body.email).toMatchObject({
      emailMessageId: 'email_msg_failed',
      errorName: 'validation_error',
      status: 'failed',
    })
    expect(emailCalls).toHaveLength(1)
    expect(emailCalls[0]).toMatchObject({
      displayName: 'Teammate <One>',
      idempotencyKey: 'team_workspace_invite:invite-0:1',
      workspaceLabel: 'Private <Workspace>',
    })
    expect(JSON.stringify(body)).not.toContain('teammate@example.com')
    expect(store.invites.get('invite-0')).toMatchObject({
      emailMessageId: 'email_msg_failed',
      sendCount: 1,
    })
  })

  test('refreshes duplicate pending invites with a new accept URL', async () => {
    const store = new MemoryInviteStore()
    const first = await routeRequest(
      store,
      operatorCreateRequest({ email: 'teammate@example.com', teamId: 'team_1' }),
      { resend: testResendConfig() },
    )
    const second = await routeRequest(
      store,
      operatorCreateRequest({ email: 'teammate@example.com', teamId: 'team_1' }),
      { resend: testResendConfig() },
    )
    const firstBody = await first.json<Record<string, unknown>>()
    const secondBody = await second.json<Record<string, unknown>>()

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    expect(firstBody.acceptUrl).not.toBe(secondBody.acceptUrl)
    expect(secondBody.invite).toMatchObject({ id: 'invite-0' })
    expect(secondBody.email).toMatchObject({
      emailMessageId: 'email_msg_invite',
      status: 'accepted',
    })
    expect(store.invites.get('invite-0')).toMatchObject({ sendCount: 2 })
  })

  test('accepts an invite for the matching signed-in email and activates membership', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        projectId: 'project_1',
        teamId: 'team_1',
      }),
    )
    const response = await routeRequest(
      store,
      new Request('https://openagents.com/api/team-workspace-invites/accept', {
        body: JSON.stringify({ token: 'token-0' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {
        session: {
          user: {
            email: 'teammate@example.com',
            name: 'Teammate',
            userId: 'email:teammate@example.com',
          },
        },
      },
    )
    const body = await response.json<Record<string, unknown>>()

    expect(response.status).toBe(200)
    expect(body.membership).toMatchObject({
      role: 'member',
      status: 'active',
      teamId: 'team_1',
    })
    expect(
      store.memberships.get('membership-team_1-email:teammate@example.com'),
    ).toMatchObject({
      role: 'member',
      teamId: 'team_1',
      userId: 'email:teammate@example.com',
    })
    expect(JSON.stringify(body)).not.toContain('teammate@example.com')
    expect(JSON.stringify(body)).not.toContain('email:teammate')
  })

  test('sends unauthenticated GET invite clicks through email login and back to the invite', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        projectId: 'project_1',
        teamId: 'team_1',
      }),
    )
    const response = await routeRequest(
      store,
      new Request(
        'https://openagents.com/api/team-workspace-invites/accept?token=token-0',
      ),
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      'https://openagents.com/login/email?returnTo=%2Fapi%2Fteam-workspace-invites%2Faccept%3Ftoken%3Dtoken-0',
    )
    expect(store.memberships.size).toBe(0)
  })

  test('sends wrong-account GET invite clicks through logout before retrying acceptance', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        projectId: 'project_1',
        teamId: 'team_1',
      }),
    )
    const response = await routeRequest(
      store,
      new Request(
        'https://openagents.com/api/team-workspace-invites/accept?token=token-0',
      ),
      {
        session: {
          user: {
            email: 'other@example.com',
            name: 'Other',
            userId: 'email:other@example.com',
          },
        },
      },
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      'https://openagents.com/auth/logout?returnTo=%2Fapi%2Fteam-workspace-invites%2Faccept%3Ftoken%3Dtoken-0',
    )
    expect(store.memberships.size).toBe(0)
  })

  test('rejects an invite accepted by a different signed-in email', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({ email: 'teammate@example.com', teamId: 'team_1' }),
    )
    const response = await routeRequest(
      store,
      new Request('https://openagents.com/api/team-workspace-invites/accept', {
        body: JSON.stringify({ token: 'token-0' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {
        session: {
          user: {
            email: 'other@example.com',
            name: 'Other',
            userId: 'email:other@example.com',
          },
        },
      },
    )

    expect(response.status).toBe(403)
  })

  test('expires stale pending invites instead of accepting them', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({
        email: 'teammate@example.com',
        expiresAt: pastIso,
        teamId: 'team_1',
      }),
    )
    const response = await routeRequest(
      store,
      new Request('https://openagents.com/api/team-workspace-invites/accept', {
        body: JSON.stringify({ token: 'token-0' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {
        session: {
          user: {
            email: 'teammate@example.com',
            name: 'Teammate',
            userId: 'email:teammate@example.com',
          },
        },
      },
    )
    const body = await response.json<Record<string, unknown>>()

    expect(response.status).toBe(410)
    expect(body.invite).toMatchObject({ status: 'expired' })
  })

  test('rejects revoked invites without activating membership', async () => {
    const store = new MemoryInviteStore()
    await routeRequest(
      store,
      operatorCreateRequest({ email: 'teammate@example.com', teamId: 'team_1' }),
    )
    const invite = store.invites.get('invite-0')

    if (invite === undefined) {
      throw new Error('Expected invite to exist.')
    }

    store.invites.set(invite.id, { ...invite, status: 'revoked' })

    const response = await routeRequest(
      store,
      new Request('https://openagents.com/api/team-workspace-invites/accept', {
        body: JSON.stringify({ token: 'token-0' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {
        session: {
          user: {
            email: 'teammate@example.com',
            name: 'Teammate',
            userId: 'email:teammate@example.com',
          },
        },
      },
    )
    const body = await response.json<Record<string, unknown>>()

    expect(response.status).toBe(409)
    expect(body).toMatchObject({
      error: 'team_workspace_invite_unavailable',
      status: 'revoked',
    })
    expect(store.memberships.size).toBe(0)
  })
})
