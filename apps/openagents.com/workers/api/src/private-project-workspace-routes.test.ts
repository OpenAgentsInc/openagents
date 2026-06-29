import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  EmailAddress,
  type ResendEmailConfig,
  ResendEmailSender,
  WorkerSecret,
} from './config'
import type {
  EmailLedgerSendResult,
  PrivateWorkspaceInviteEmailInput,
} from './email'
import {
  type CreatePrefilledWorkspaceInput,
  type PrefilledWorkspaceRecord,
  type PrefilledWorkspaceServiceShape,
  makePrefilledWorkspaceRecord,
} from './prefilled-workspace'
import {
  type CreateOrUpdatePrivateProjectInput,
  type PrivateProjectWorkspaceProjectRecord,
  type PrivateProjectWorkspaceStore,
  type PrivateProjectWorkspaceTeamRecord,
  makePrivateProjectWorkspaceRoutes,
} from './private-project-workspace-routes'
import {
  type TeamWorkspaceInviteAcceptInput,
  type TeamWorkspaceInviteAcceptResult,
  type TeamWorkspaceInviteCreateInput,
  type TeamWorkspaceInviteCreateResult,
  type TeamWorkspaceInviteRecord,
  type TeamWorkspaceInviteStore,
  normalizeTeamWorkspaceInviteEmail,
} from './team-workspace-invites'

const nowIso = '2026-06-16T12:00:00.000Z'
const futureIso = '2026-06-19T12:00:00.000Z'

type Bindings = Readonly<{
  resend?: ResendEmailConfig | undefined
}>

const ctx = {
  passThroughOnException: () => {},
  waitUntil: () => {},
} as unknown as ExecutionContext

const testResendConfig = (): ResendEmailConfig => ({
  apiKey: Redacted.make(WorkerSecret.make('re_test')),
  fromEmail: ResendEmailSender.make('OpenAgents <ops@example.com>'),
  replyToEmail: EmailAddress.make('operator-copy@example.com'),
})

class MemoryPrivateProjectStore implements PrivateProjectWorkspaceStore {
  readonly projects = new Map<string, PrivateProjectWorkspaceProjectRecord>()
  readonly teams = new Map<string, PrivateProjectWorkspaceTeamRecord>()

  createOrUpdateProject = async (input: CreateOrUpdatePrivateProjectInput) => {
    const existingTeam = this.teams.get(input.teamSlug)
    const team =
      existingTeam ??
      ({
        id: `team_${this.teams.size}`,
        name: input.teamName,
        slug: input.teamSlug,
        status: 'active',
      } satisfies PrivateProjectWorkspaceTeamRecord)
    const existingProject = this.projects.get(`${team.id}:${input.projectSlug}`)
    const project =
      existingProject ??
      ({
        id: `project_${this.projects.size}`,
        name: input.projectName,
        slug: input.projectSlug,
        status: 'active',
        teamId: team.id,
      } satisfies PrivateProjectWorkspaceProjectRecord)

    this.teams.set(team.slug, { ...team, name: input.teamName })
    this.projects.set(`${team.id}:${project.slug}`, {
      ...project,
      name: input.projectName,
    })

    return {
      project: this.projects.get(`${team.id}:${project.slug}`)!,
      team: this.teams.get(team.slug)!,
    }
  }
}

class MemoryWorkspaceStore implements PrefilledWorkspaceServiceShape {
  readonly workspaces = new Map<string, PrefilledWorkspaceRecord>()
  workspaceCounter = 0

  createWorkspace = async (input: CreatePrefilledWorkspaceInput) => {
    const record = makePrefilledWorkspaceRecord(input, {
      makeId: prefix => `${prefix}_${this.workspaceCounter++}`,
      nowIso: () => nowIso,
    })
    this.workspaces.set(record.id, record)

    return record
  }

  readWorkspace = async (workspaceId: string) =>
    this.workspaces.get(workspaceId)

  readWorkspaceForHolder = async () => undefined

  readOrClaimWorkspaceForHolder = async () => undefined

  readPrivateWorkspaceForTeamMember = async () => undefined

  readPrivateWorkspaceByTarget = async (
    privateTeamId: string,
    privateProjectId: string | null,
  ) =>
    [...this.workspaces.values()].find(
      workspace =>
        workspace.accessMode === 'private_team' &&
        workspace.privateTeamId === privateTeamId &&
        workspace.privateProjectId === privateProjectId,
    )

  recordFirstRunForHolder = async () => undefined

  recordFirstRunForOperator = async () => undefined

  recordFirstRunForPrivateTeamMember = async () => undefined
}

const inviteRecord = (
  input: Readonly<{
    email: string
    id: string
    metadataJson?: string
    projectId: string
    role?: TeamWorkspaceInviteRecord['role']
    teamId: string
    token: string
  }>,
): TeamWorkspaceInviteRecord => ({
  acceptedAt: null,
  acceptedByUserId: null,
  createdAt: nowIso,
  emailMessageId: null,
  expiresAt: futureIso,
  id: input.id,
  inviteeEmail: input.email,
  inviteeEmailNormalized:
    normalizeTeamWorkspaceInviteEmail(input.email) ?? input.email,
  invitedByActorRef: 'operator:admin_api',
  lastSentAt: null,
  metadataJson: input.metadataJson ?? '{}',
  projectId: input.projectId,
  revokedAt: null,
  role: input.role ?? 'member',
  sendCount: 0,
  status: 'pending',
  teamId: input.teamId,
  tokenHash: `hash:${input.token}`,
  updatedAt: nowIso,
})

class MemoryInviteStore implements TeamWorkspaceInviteStore {
  readonly invites = new Map<string, TeamWorkspaceInviteRecord>()
  tokenCounter = 0

  createOrRefreshInvite = async (
    input: TeamWorkspaceInviteCreateInput,
  ): Promise<TeamWorkspaceInviteCreateResult> => {
    const normalized = normalizeTeamWorkspaceInviteEmail(input.email)

    if (normalized === undefined) {
      return { _tag: 'InvalidEmail' }
    }

    const projectId = input.projectId ?? null

    if (projectId === null) {
      return { _tag: 'ProjectNotFound' }
    }

    const existing = [...this.invites.values()].find(
      invite =>
        invite.teamId === input.teamId &&
        invite.projectId === projectId &&
        invite.inviteeEmailNormalized === normalized &&
        invite.status === 'pending',
    )
    const token = `token-${this.tokenCounter++}`

    if (existing !== undefined) {
      const refreshed = {
        ...existing,
        metadataJson: input.metadataJson ?? existing.metadataJson,
        role: input.role ?? existing.role,
        tokenHash: `hash:${token}`,
        updatedAt: nowIso,
      }
      this.invites.set(refreshed.id, refreshed)

      return { _tag: 'Refreshed', invite: refreshed, token }
    }

    const invite = inviteRecord({
      email: input.email,
      id: `invite_${this.invites.size}`,
      projectId,
      teamId: input.teamId,
      token,
      ...(input.metadataJson === undefined
        ? {}
        : { metadataJson: input.metadataJson }),
      ...(input.role === undefined ? {} : { role: input.role }),
    })
    this.invites.set(invite.id, invite)

    return { _tag: 'Created', invite, token }
  }

  acceptInvite = async (
    _input: TeamWorkspaceInviteAcceptInput,
  ): Promise<TeamWorkspaceInviteAcceptResult> => ({ _tag: 'NotFound' })

  recordEmailAttempt = async (input: {
    attemptedAt: string
    emailMessageId: string
    inviteId: string
  }) => {
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

const makeRoutes = (
  stores: Readonly<{
    invites: MemoryInviteStore
    privateProjects: MemoryPrivateProjectStore
    workspaces: MemoryWorkspaceStore
  }>,
  options: Readonly<{
    emailCalls?: Array<PrivateWorkspaceInviteEmailInput> | undefined
  }> = {},
) =>
  makePrivateProjectWorkspaceRoutes<Bindings>({
    appOrigin: () => 'https://openagents.com',
    getResendEmailConfig: env => env.resend,
    makeInviteStore: () => stores.invites,
    makePrivateProjectStore: () => stores.privateProjects,
    makeWorkspaceStore: () => stores.workspaces,
    nowIso: () => nowIso,
    requireAdminApiToken: async request =>
      request.headers.get('authorization') === 'Bearer admin-token',
    sendInviteEmailWithLedger: (_env, _config, input) => {
      options.emailCalls?.push(input)

      return Effect.succeed({
        emailMessageId: `email_msg_${options.emailCalls?.length ?? 0}`,
        ok: true,
        providerMessageId: `resend_${options.emailCalls?.length ?? 0}`,
      } satisfies EmailLedgerSendResult)
    },
  })

const stores = () => ({
  invites: new MemoryInviteStore(),
  privateProjects: new MemoryPrivateProjectStore(),
  workspaces: new MemoryWorkspaceStore(),
})

const routeRequest = async (
  request: Request,
  env: Bindings,
  testStores = stores(),
  options: Parameters<typeof makeRoutes>[1] = {},
): Promise<Response> => {
  const effect = makeRoutes(
    testStores,
    options,
  ).routePrivateProjectWorkspaceRequest(request, env, ctx)

  if (effect === undefined) {
    throw new Error(
      'Expected private project workspace route to handle request.',
    )
  }

  return Effect.runPromise(effect)
}

const operatorRequest = (body: unknown, token = 'admin-token'): Request =>
  new Request(
    'https://openagents.com/api/operator/private-project-workspaces',
    {
      body: JSON.stringify(body),
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    },
  )

const baseRequest = () => ({
  invitations: [
    {
      email: 'teammate@example.com',
      participantKind: 'internal_team_member',
      role: 'admin',
    },
    {
      email: 'partner@example.com',
      participantKind: 'external_partner',
      role: 'member',
    },
  ],
  project: {
    name: 'Private Project Workspace',
  },
  workspace: {
    createPrefilledWorkspace: true,
    workspaceLabel: 'Private project workspace',
  },
})

describe('private project workspace routes', () => {
  test('requires an operator token', async () => {
    const response = await routeRequest(
      operatorRequest(baseRequest(), 'bad-token'),
      { resend: testResendConfig() },
    )

    expect(response.status).toBe(401)
  })

  test('creates a private project workspace and fans out invites with operator copies by default', async () => {
    const testStores = stores()
    const emailCalls: Array<PrivateWorkspaceInviteEmailInput> = []
    const response = await routeRequest(
      operatorRequest(baseRequest()),
      { resend: testResendConfig() },
      testStores,
      { emailCalls },
    )
    const body = await response.json<Record<string, unknown>>()
    const text = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body.team).toMatchObject({
      id: 'team_0',
      slug: 'private-project-workspace-team',
      status: 'active',
    })
    expect(body.project).toMatchObject({
      id: 'project_0',
      slug: 'private-project-workspace',
      status: 'active',
      teamId: 'team_0',
    })
    expect(body.workspace).toMatchObject({
      accessMode: 'private_team',
      id: 'workspace_1',
      projectName: 'Private Project Workspace',
      status: 'invited',
    })
    expect(body.invitations).toHaveLength(2)
    expect(emailCalls.map(call => call.to)).toEqual([
      'teammate@example.com',
      'operator-copy@example.com',
      'partner@example.com',
      'operator-copy@example.com',
    ])
    expect(emailCalls.map(call => call.idempotencyKey)).toEqual([
      'team_workspace_invite:invite_0:1',
      'team_workspace_invite:invite_0:1:operator_copy',
      'team_workspace_invite:invite_1:1',
      'team_workspace_invite:invite_1:1:operator_copy',
    ])
    expect(emailCalls.map(call => call.acceptUrl)).toEqual([
      'https://openagents.com/api/team-workspace-invites/accept?token=token-0',
      '[redacted operator copy invite link]',
      'https://openagents.com/api/team-workspace-invites/accept?token=token-1',
      '[redacted operator copy invite link]',
    ])
    expect(
      emailCalls
        .filter(call => call.to === 'operator-copy@example.com')
        .every(
          call =>
            !call.acceptUrl.includes('accept?token') &&
            !call.acceptUrl.includes('token-'),
        ),
    ).toBe(true)
    expect(testStores.invites.invites.get('invite_0')).toMatchObject({
      emailMessageId: 'email_msg_1',
      lastSentAt: nowIso,
      sendCount: 1,
    })
    expect(
      JSON.parse(
        testStores.invites.invites.get('invite_0')?.metadataJson ?? '{}',
      ),
    ).toMatchObject({
      copyOperator: true,
      participantKind: 'internal_team_member',
    })
    expect(text).not.toContain('teammate@example.com')
    expect(text).not.toContain('partner@example.com')
    expect(text).not.toContain('operator-copy@example.com')
    expect(text).not.toContain('token-')
    expect(text).not.toContain('accept?token')
  })

  test('reuses the same team project workspace and pending invite on retry', async () => {
    const testStores = stores()
    const emailCalls: Array<PrivateWorkspaceInviteEmailInput> = []

    await routeRequest(
      operatorRequest(baseRequest()),
      { resend: testResendConfig() },
      testStores,
      { emailCalls },
    )
    const second = await routeRequest(
      operatorRequest(baseRequest()),
      { resend: testResendConfig() },
      testStores,
      { emailCalls },
    )
    const body = await second.json<Record<string, unknown>>()

    expect(second.status).toBe(201)
    expect(testStores.privateProjects.teams.size).toBe(1)
    expect(testStores.privateProjects.projects.size).toBe(1)
    expect(testStores.workspaces.workspaces.size).toBe(1)
    expect(testStores.invites.invites.size).toBe(2)
    expect(body.workspace).toMatchObject({ id: 'workspace_1' })
    expect(emailCalls.map(call => call.idempotencyKey).slice(4)).toEqual([
      'team_workspace_invite:invite_0:2',
      'team_workspace_invite:invite_0:2:operator_copy',
      'team_workspace_invite:invite_1:2',
      'team_workspace_invite:invite_1:2:operator_copy',
    ])
  })

  test('allows operator copy opt-out at request level', async () => {
    const emailCalls: Array<PrivateWorkspaceInviteEmailInput> = []
    const response = await routeRequest(
      operatorRequest({
        ...baseRequest(),
        email: { copyOperator: false },
      }),
      { resend: testResendConfig() },
      stores(),
      { emailCalls },
    )
    const body = await response.json<{
      invitations: ReadonlyArray<
        Readonly<{ copy: Readonly<{ status: string }> }>
      >
    }>()

    expect(response.status).toBe(201)
    expect(emailCalls.map(call => call.to)).toEqual([
      'teammate@example.com',
      'partner@example.com',
    ])
    expect(body.invitations.map(invitation => invitation.copy.status)).toEqual([
      'disabled',
      'disabled',
    ])
  })

  test('creates invites safely when Resend config is missing', async () => {
    const testStores = stores()
    const response = await routeRequest(
      operatorRequest(baseRequest()),
      {},
      testStores,
    )
    const body = await response.json<{
      invitations: ReadonlyArray<
        Readonly<{
          copy: Readonly<{ status: string }>
          email: Readonly<{ status: string }>
        }>
      >
    }>()

    expect(response.status).toBe(201)
    expect(testStores.invites.invites.size).toBe(2)
    expect(body.invitations.map(invitation => invitation.email.status)).toEqual(
      ['missing_config', 'missing_config'],
    )
    expect(body.invitations.map(invitation => invitation.copy.status)).toEqual([
      'missing_config',
      'missing_config',
    ])
  })

  test('returns accept URLs only when explicitly requested', async () => {
    const response = await routeRequest(
      operatorRequest({
        ...baseRequest(),
        includeAcceptUrls: true,
      }),
      { resend: testResendConfig() },
    )
    const body = await response.json<{
      invitations: ReadonlyArray<Readonly<{ acceptUrl?: string }>>
    }>()

    expect(response.status).toBe(201)
    expect(body.invitations[0]?.acceptUrl).toBe(
      'https://openagents.com/api/team-workspace-invites/accept?token=token-0',
    )
    expect(body.invitations[1]?.acceptUrl).toBe(
      'https://openagents.com/api/team-workspace-invites/accept?token=token-1',
    )
  })
})
