import { readRequestJsonEffect } from '@openagentsinc/effect-boundary'
import { badRequest } from '@openagentsinc/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import type { ResendEmailConfig } from './config'
import { PrivateWorkspaceInviteEmailInput } from './email'
import type { EmailLedgerSendResult, EmailServiceError } from './email'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
  unauthorized,
} from './http/responses'
import type {
  CreatePrefilledWorkspaceInput,
  PrefilledWorkspaceRecord,
  PrefilledWorkspaceServiceShape,
} from './prefilled-workspace'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  type TeamWorkspaceInviteCreateResult,
  TeamWorkspaceInviteRole,
  type TeamWorkspaceInviteStore,
  normalizeTeamWorkspaceInviteEmail,
  safeTeamWorkspaceInviteProjection,
} from './team-workspace-invites'

type HttpResponse = globalThis.Response

const PrivateProjectParticipantKind = S.Literals([
  'internal_team_member',
  'external_partner',
  'client',
])
type PrivateProjectParticipantKind = typeof PrivateProjectParticipantKind.Type

const PrivateProjectEmailCopyMode = S.Literals(['bcc_or_ledger_copy'])
type PrivateProjectEmailCopyMode = typeof PrivateProjectEmailCopyMode.Type

const SeededMemoryInput = S.Struct({
  label: S.String,
  value: S.String,
  publicSourceRef: S.String,
})

const StarterWorkflowInput = S.Struct({
  title: S.String,
  description: S.String,
  outcomeKind: S.String,
  status: S.optionalKey(
    S.Literals(['queued', 'ready', 'completed', 'dismissed']),
  ),
})

const IntroReceiptInput = S.Struct({
  summary: S.String,
  publicSourceRefs: S.Array(S.String),
})

const PrivateProjectInput = S.Struct({
  description: S.optionalKey(S.String),
  name: S.String,
  projectSlug: S.optionalKey(S.String),
  slug: S.optionalKey(S.String),
  teamName: S.optionalKey(S.String),
  teamSlug: S.optionalKey(S.String),
})
type PrivateProjectInput = typeof PrivateProjectInput.Type

const PrivateProjectWorkspaceInput = S.Struct({
  createPrefilledWorkspace: S.optionalKey(S.Boolean),
  holderRef: S.optionalKey(S.String),
  introReceipt: S.optionalKey(IntroReceiptInput),
  seededMemory: S.optionalKey(S.Array(SeededMemoryInput)),
  starterWorkflows: S.optionalKey(S.Array(StarterWorkflowInput)),
  status: S.optionalKey(S.Literals(['draft', 'invited', 'active'])),
  workspaceLabel: S.optionalKey(S.String),
})
type PrivateProjectWorkspaceInput = typeof PrivateProjectWorkspaceInput.Type

const PrivateProjectEmailInput = S.Struct({
  copyMode: S.optionalKey(PrivateProjectEmailCopyMode),
  copyOperator: S.optionalKey(S.Boolean),
  operatorCopyEmail: S.optionalKey(S.String),
  sendEmail: S.optionalKey(S.Boolean),
})
type PrivateProjectEmailInput = typeof PrivateProjectEmailInput.Type

const PrivateProjectInvitationInput = S.Struct({
  copyOperator: S.optionalKey(S.Boolean),
  email: S.String,
  expiresAt: S.optionalKey(S.String),
  expiresInHours: S.optionalKey(S.Number),
  participantKind: PrivateProjectParticipantKind,
  recipientDisplayName: S.optionalKey(S.String),
  role: S.optionalKey(TeamWorkspaceInviteRole),
  sendEmail: S.optionalKey(S.Boolean),
})
type PrivateProjectInvitationInput = typeof PrivateProjectInvitationInput.Type

const CreatePrivateProjectWorkspaceRequest = S.Struct({
  email: S.optionalKey(PrivateProjectEmailInput),
  includeAcceptUrls: S.optionalKey(S.Boolean),
  invitations: S.Array(PrivateProjectInvitationInput),
  project: PrivateProjectInput,
  workspace: S.optionalKey(PrivateProjectWorkspaceInput),
})
type CreatePrivateProjectWorkspaceRequest =
  typeof CreatePrivateProjectWorkspaceRequest.Type

export type PrivateProjectWorkspaceRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

const systemRuntime: PrivateProjectWorkspaceRuntime = {
  makeId: prefix => `${prefix}_${randomUuid()}`,
  nowIso: currentIsoTimestamp,
}

export type PrivateProjectWorkspaceTeamRecord = Readonly<{
  id: string
  name: string
  slug: string
  status: 'active' | 'archived'
}>

export type PrivateProjectWorkspaceProjectRecord = Readonly<{
  id: string
  name: string
  slug: string
  status: 'active' | 'archived'
  teamId: string
}>

export type PrivateProjectWorkspaceStore = Readonly<{
  createOrUpdateProject: (input: CreateOrUpdatePrivateProjectInput) => Promise<
    Readonly<{
      project: PrivateProjectWorkspaceProjectRecord
      team: PrivateProjectWorkspaceTeamRecord
    }>
  >
}>

export type CreateOrUpdatePrivateProjectInput = Readonly<{
  description: string
  projectName: string
  projectSlug: string
  teamName: string
  teamSlug: string
}>

type TeamRow = Readonly<{
  id: string
  name: string
  slug: string
  status: 'active' | 'archived'
}>

type ProjectRow = Readonly<{
  id: string
  name: string
  slug: string
  status: 'active' | 'archived'
  team_id: string
}>

class PrivateProjectWorkspaceWriteError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PrivateProjectWorkspaceWriteError'
  }
}

type PrivateProjectWorkspaceEmailDelivery =
  | Readonly<{
      emailMessageId: string
      providerMessageId: string | null
      status: 'accepted'
    }>
  | Readonly<{
      emailMessageId: string | null
      errorMessage: string
      errorName: string
      status: 'failed'
    }>
  | Readonly<{
      errorMessage: string
      errorName: string
      status: 'missing_config'
    }>
  | Readonly<{
      errorMessage: string
      errorName: string
      status: 'missing_copy_recipient'
    }>
  | Readonly<{ status: 'disabled' }>

type PrivateProjectWorkspaceRoutesDependencies<Bindings> = Readonly<{
  appOrigin: (env: Bindings) => string
  getResendEmailConfig?: (env: Bindings) => ResendEmailConfig | undefined
  makeInviteStore: (env: Bindings) => TeamWorkspaceInviteStore
  makePrivateProjectStore: (env: Bindings) => PrivateProjectWorkspaceStore
  makeWorkspaceStore: (env: Bindings) => PrefilledWorkspaceServiceShape
  nowIso?: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  sendInviteEmailWithLedger?: (
    env: Bindings,
    config: ResendEmailConfig,
    input: PrivateWorkspaceInviteEmailInput,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
}>

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const slugFromText = (value: string): string =>
  compactText(value, 120)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)

const normalizeSlug = (value: string): string | undefined => {
  const normalized = value.trim().toLowerCase()

  return /^[a-z0-9][a-z0-9_-]{0,78}[a-z0-9]$/.test(normalized) ||
    /^[a-z0-9]$/.test(normalized)
    ? normalized
    : undefined
}

const teamRecordFromRow = (
  row: TeamRow,
): PrivateProjectWorkspaceTeamRecord => ({
  id: row.id,
  name: row.name,
  slug: row.slug,
  status: row.status,
})

const projectRecordFromRow = (
  row: ProjectRow,
): PrivateProjectWorkspaceProjectRecord => ({
  id: row.id,
  name: row.name,
  slug: row.slug,
  status: row.status,
  teamId: row.team_id,
})

const readTeamBySlug = async (
  db: D1Database,
  slug: string,
): Promise<PrivateProjectWorkspaceTeamRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, name, slug, status
         FROM teams
        WHERE slug = ?
        LIMIT 1`,
    )
    .bind(slug)
    .first<TeamRow>()

  return row === null ? undefined : teamRecordFromRow(row)
}

const readProjectByTeamAndSlug = async (
  db: D1Database,
  teamId: string,
  slug: string,
): Promise<PrivateProjectWorkspaceProjectRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, team_id, slug, name, status
         FROM team_projects
        WHERE team_id = ?
          AND slug = ?
        LIMIT 1`,
    )
    .bind(teamId, slug)
    .first<ProjectRow>()

  return row === null ? undefined : projectRecordFromRow(row)
}

export const makeD1PrivateProjectWorkspaceStore = (
  db: D1Database,
  runtime: PrivateProjectWorkspaceRuntime = systemRuntime,
): PrivateProjectWorkspaceStore => ({
  createOrUpdateProject: async input => {
    const now = runtime.nowIso()
    const existingTeam = await readTeamBySlug(db, input.teamSlug)
    const teamId = existingTeam?.id ?? runtime.makeId('team')

    await db
      .prepare(
        `INSERT INTO teams
          (id, name, slug, kind, plan, owner_user_id, status, created_at,
           updated_at, archived_at)
         VALUES (?, ?, ?, 'organization', 'team', NULL, 'active', ?, ?, NULL)
         ON CONFLICT(slug) DO UPDATE SET
           name = excluded.name,
           kind = excluded.kind,
           plan = excluded.plan,
           status = 'active',
           archived_at = NULL,
           updated_at = excluded.updated_at`,
      )
      .bind(teamId, input.teamName, input.teamSlug, now, now)
      .run()

    const team = await readTeamBySlug(db, input.teamSlug)

    if (team === undefined) {
      throw new PrivateProjectWorkspaceWriteError(
        'Private project team was not found after write.',
      )
    }

    const existingProject = await readProjectByTeamAndSlug(
      db,
      team.id,
      input.projectSlug,
    )
    const projectId = existingProject?.id ?? runtime.makeId('team_project')

    await db
      .prepare(
        `INSERT INTO team_projects
          (id, team_id, slug, name, description, status, metadata_json,
           created_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, NULL)
         ON CONFLICT(team_id, slug) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           status = 'active',
           metadata_json = excluded.metadata_json,
           archived_at = NULL,
           updated_at = excluded.updated_at`,
      )
      .bind(
        projectId,
        team.id,
        input.projectSlug,
        input.projectName,
        input.description,
        JSON.stringify({
          source: 'operator_private_project_workspace_api',
          sourceIssue: 5156,
        }),
        now,
        now,
      )
      .run()

    const project = await readProjectByTeamAndSlug(
      db,
      team.id,
      input.projectSlug,
    )

    if (project === undefined) {
      throw new PrivateProjectWorkspaceWriteError(
        'Private project was not found after write.',
      )
    }

    return { project, team }
  },
})

const routeNowIso = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const decodeBody = <A>(
  schema: S.Decoder<A>,
  request: Request,
): Effect.Effect<A, HttpResponse> =>
  readRequestJsonEffect(
    schema,
    request,
    'private_project_workspace.body',
  ).pipe(
    Effect.mapError(error =>
      badRequest(
        error.reasonRef === 'boundary.json.malformed'
          ? 'Malformed JSON request body.'
          : 'Private project workspace request did not match the expected schema.',
      ),
    ),
  )

const dependencyPromise = <A>(
  tryPromise: () => Promise<A>,
): Effect.Effect<A, HttpResponse> =>
  Effect.tryPromise({
    catch: () => serverError(),
    try: tryPromise,
  })

const recordInviteEmailAttempt = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    attemptedAt: string
    emailMessageId: string
    inviteId: string
  }>,
): Effect.Effect<void> => {
  const store = dependencies.makeInviteStore(env)

  return Effect.tryPromise({
    catch: () => undefined,
    try: () => store.recordEmailAttempt(input),
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )
}

const acceptUrl = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
  env: Bindings,
  token: string,
): string =>
  `${dependencies.appOrigin(env)}/api/team-workspace-invites/accept?token=${encodeURIComponent(token)}`

const inviteEmailIdempotencyKey = (
  inviteId: string,
  sendCount: number,
): string => `team_workspace_invite:${inviteId}:${sendCount + 1}`

const redactedOperatorCopyAcceptUrl = '[redacted operator copy invite link]'

const inviteEmailInput = (
  invitation: PrivateProjectInvitationInput,
  workspaceLabel: string,
  invite: Readonly<{
    expiresAt: string
    id: string
    inviteeEmail: string
    projectId: string | null
    sendCount: number
    teamId: string
  }>,
  inviteAcceptUrl: string,
  idempotencyKey: string,
  to: string,
): PrivateWorkspaceInviteEmailInput =>
  new PrivateWorkspaceInviteEmailInput({
    acceptUrl: inviteAcceptUrl,
    displayName: invitation.recipientDisplayName ?? 'there',
    expiresAt: invite.expiresAt,
    idempotencyKey,
    inviteId: invite.id,
    projectId: invite.projectId,
    teamId: invite.teamId,
    to,
    workspaceLabel,
  })

const safeInviteEmailFailure = (
  error: EmailServiceError,
): PrivateProjectWorkspaceEmailDelivery => ({
  emailMessageId: null,
  errorMessage: error.message,
  errorName: 'email_service_error',
  status: 'failed',
})

const deliveryFromLedgerResult = (
  result: EmailLedgerSendResult,
): PrivateProjectWorkspaceEmailDelivery =>
  result.ok
    ? {
        emailMessageId: result.emailMessageId,
        providerMessageId: result.providerMessageId,
        status: 'accepted',
      }
    : {
        emailMessageId: result.emailMessageId,
        errorMessage: result.errorMessage,
        errorName: result.errorName ?? 'resend_error',
        status: 'failed',
      }

const missingConfigDelivery = (): PrivateProjectWorkspaceEmailDelivery => ({
  errorMessage: 'Resend email configuration is not set.',
  errorName: 'email_config_missing',
  status: 'missing_config',
})

const sendLedgerEmail = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
  env: Bindings,
  config: ResendEmailConfig,
  input: PrivateWorkspaceInviteEmailInput,
): Effect.Effect<PrivateProjectWorkspaceEmailDelivery> =>
  dependencies.sendInviteEmailWithLedger === undefined
    ? Effect.succeed(missingConfigDelivery())
    : dependencies.sendInviteEmailWithLedger(env, config, input).pipe(
        Effect.map(deliveryFromLedgerResult),
        Effect.catch(error => Effect.succeed(safeInviteEmailFailure(error))),
      )

const resolveOperatorCopyEmail = (
  requestEmail: PrivateProjectEmailInput | undefined,
  config: ResendEmailConfig,
): string | undefined => {
  const requested = requestEmail?.operatorCopyEmail

  if (requested !== undefined && requested.trim() !== '') {
    return requested.trim()
  }

  return config.replyToEmail
}

const metadataJsonForInvitation = (
  invitation: PrivateProjectInvitationInput,
  copyOperator: boolean,
): string =>
  JSON.stringify({
    copyOperator,
    participantKind: invitation.participantKind,
    source: 'operator_private_project_workspace_api',
    sourceIssue: 5156,
  })

const sendInviteAndOperatorCopy = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
  env: Bindings,
  requestEmail: PrivateProjectEmailInput | undefined,
  invitation: PrivateProjectInvitationInput,
  workspaceLabel: string,
  copyOperator: boolean,
  invite: Extract<
    TeamWorkspaceInviteCreateResult,
    { _tag: 'Created' | 'Refreshed' }
  >['invite'],
  token: string,
): Effect.Effect<
  Readonly<{
    copy: PrivateProjectWorkspaceEmailDelivery
    email: PrivateProjectWorkspaceEmailDelivery
  }>
> => {
  const sendEmail = invitation.sendEmail ?? requestEmail?.sendEmail ?? true

  if (!sendEmail) {
    return Effect.succeed({
      copy: { status: 'disabled' },
      email: { status: 'disabled' },
    })
  }

  const config = dependencies.getResendEmailConfig?.(env)

  if (config === undefined) {
    return Effect.succeed({
      copy: copyOperator ? missingConfigDelivery() : { status: 'disabled' },
      email: missingConfigDelivery(),
    })
  }

  const inviteAcceptUrl = acceptUrl(dependencies, env, token)
  const baseIdempotencyKey = inviteEmailIdempotencyKey(
    invite.id,
    invite.sendCount,
  )

  return Effect.gen(function* () {
    const email = yield* sendLedgerEmail(
      dependencies,
      env,
      config,
      inviteEmailInput(
        invitation,
        workspaceLabel,
        invite,
        inviteAcceptUrl,
        baseIdempotencyKey,
        invite.inviteeEmail,
      ),
    )

    const emailMessageId =
      'emailMessageId' in email ? email.emailMessageId : null

    if (emailMessageId !== null) {
      yield* recordInviteEmailAttempt(dependencies, env, {
        attemptedAt: routeNowIso(dependencies),
        emailMessageId,
        inviteId: invite.id,
      })
    }

    if (!copyOperator) {
      return { copy: { status: 'disabled' } as const, email }
    }

    const operatorCopyEmail = resolveOperatorCopyEmail(requestEmail, config)

    if (
      operatorCopyEmail === undefined ||
      normalizeTeamWorkspaceInviteEmail(operatorCopyEmail) === undefined
    ) {
      return {
        copy: {
          errorMessage:
            'Operator copy was requested, but no valid operator copy email was configured.',
          errorName: 'operator_copy_email_missing',
          status: 'missing_copy_recipient',
        },
        email,
      }
    }

    const copy = yield* sendLedgerEmail(
      dependencies,
      env,
      config,
      inviteEmailInput(
        invitation,
        workspaceLabel,
        invite,
        redactedOperatorCopyAcceptUrl,
        `${baseIdempotencyKey}:operator_copy`,
        operatorCopyEmail,
      ),
    )

    return { copy, email }
  })
}

const validateCreateRequest = (
  body: CreatePrivateProjectWorkspaceRequest,
): Effect.Effect<
  Readonly<{
    description: string
    projectName: string
    projectSlug: string
    teamName: string
    teamSlug: string
    workspaceLabel: string
  }>,
  HttpResponse
> => {
  const projectName = compactText(body.project.name, 200)

  if (projectName === '') {
    return Effect.fail(badRequest('A project name is required.'))
  }

  if (body.invitations.length === 0) {
    return Effect.fail(badRequest('At least one invitation is required.'))
  }

  const invalidEmail = body.invitations.find(
    invitation =>
      normalizeTeamWorkspaceInviteEmail(invitation.email) === undefined,
  )

  if (invalidEmail !== undefined) {
    return Effect.fail(badRequest('Every invitation needs a valid email.'))
  }

  const projectSlugCandidate =
    body.project.projectSlug ?? body.project.slug ?? slugFromText(projectName)
  const teamSlugCandidate =
    body.project.teamSlug ?? `${projectSlugCandidate}-team`
  const projectSlug = normalizeSlug(projectSlugCandidate)
  const teamSlug = normalizeSlug(teamSlugCandidate)

  if (projectSlug === undefined) {
    return Effect.fail(badRequest('A valid project slug is required.'))
  }

  if (teamSlug === undefined) {
    return Effect.fail(badRequest('A valid team slug is required.'))
  }

  return Effect.succeed({
    description:
      body.project.description === undefined
        ? 'Private project workspace.'
        : compactText(body.project.description, 500),
    projectName,
    projectSlug,
    teamName:
      body.project.teamName === undefined || body.project.teamName.trim() === ''
        ? `${projectName} Team`
        : compactText(body.project.teamName, 200),
    teamSlug,
    workspaceLabel:
      body.workspace?.workspaceLabel === undefined ||
      body.workspace.workspaceLabel.trim() === ''
        ? 'Private project workspace'
        : compactText(body.workspace.workspaceLabel, 200),
  })
}

const workspaceCreateInput = (
  body: CreatePrivateProjectWorkspaceRequest,
  project: PrivateProjectWorkspaceProjectRecord,
  team: PrivateProjectWorkspaceTeamRecord,
  projectName: string,
): CreatePrefilledWorkspaceInput => ({
  accessMode: 'private_team',
  holderRef: body.workspace?.holderRef,
  introReceipt: body.workspace?.introReceipt ?? {
    publicSourceRefs: [],
    summary: `Private workspace created for ${projectName}.`,
  },
  privateProjectId: project.id,
  privateTeamId: team.id,
  projectName,
  seededMemory: body.workspace?.seededMemory,
  starterWorkflows: body.workspace?.starterWorkflows?.map(workflow => ({
    description: workflow.description,
    outcomeKind: workflow.outcomeKind,
    status: workflow.status ?? 'queued',
    title: workflow.title,
  })),
  status: body.workspace?.status ?? 'invited',
})

const workspaceView = (record: PrefilledWorkspaceRecord | null) =>
  record === null
    ? null
    : {
        accessMode: record.accessMode,
        id: record.id,
        projectName: record.projectName,
        status: record.status,
      }

const teamView = (team: PrivateProjectWorkspaceTeamRecord) => ({
  id: team.id,
  slug: team.slug,
  status: team.status,
})

const projectView = (project: PrivateProjectWorkspaceProjectRecord) => ({
  id: project.id,
  slug: project.slug,
  status: project.status,
  teamId: project.teamId,
})

const createPrivateProjectWorkspace = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const authorized = yield* dependencyPromise(() =>
      dependencies.requireAdminApiToken(request, env),
    )

    if (!authorized) {
      return unauthorized()
    }

    const body = yield* decodeBody(
      CreatePrivateProjectWorkspaceRequest,
      request,
    )
    const normalized = yield* validateCreateRequest(body)
    const privateProject = yield* dependencyPromise(() =>
      dependencies.makePrivateProjectStore(env).createOrUpdateProject({
        description: normalized.description,
        projectName: normalized.projectName,
        projectSlug: normalized.projectSlug,
        teamName: normalized.teamName,
        teamSlug: normalized.teamSlug,
      }),
    )
    const workspace =
      body.workspace?.createPrefilledWorkspace === false
        ? null
        : yield* dependencyPromise(() =>
            dependencies
              .makeWorkspaceStore(env)
              .readPrivateWorkspaceByTarget(
                privateProject.team.id,
                privateProject.project.id,
              ),
          ).pipe(
            Effect.flatMap(existing =>
              existing === undefined
                ? dependencyPromise(() =>
                    dependencies
                      .makeWorkspaceStore(env)
                      .createWorkspace(
                        workspaceCreateInput(
                          body,
                          privateProject.project,
                          privateProject.team,
                          normalized.projectName,
                        ),
                      ),
                  )
                : Effect.succeed(existing),
            ),
          )
    const invitationResults = yield* Effect.forEach(
      body.invitations.map((invitation, index) => ({ index, invitation })),
      ({ index, invitation }) =>
        Effect.gen(function* () {
          const copyOperator =
            invitation.copyOperator ?? body.email?.copyOperator ?? true
          const inviteResult = yield* dependencyPromise(() =>
            dependencies.makeInviteStore(env).createOrRefreshInvite({
              email: invitation.email,
              expiresAt: invitation.expiresAt,
              expiresInHours: invitation.expiresInHours,
              invitedByActorRef: 'operator:admin_api',
              metadataJson: metadataJsonForInvitation(invitation, copyOperator),
              projectId: privateProject.project.id,
              role: invitation.role,
              teamId: privateProject.team.id,
            }),
          )

          return yield* M.value(inviteResult).pipe(
            M.tags({
              Created: ({ invite, token }) =>
                Effect.gen(function* () {
                  const delivery = yield* sendInviteAndOperatorCopy(
                    dependencies,
                    env,
                    body.email,
                    invitation,
                    normalized.workspaceLabel,
                    copyOperator,
                    invite,
                    token,
                  )

                  return {
                    copy: delivery.copy,
                    email: delivery.email,
                    ...(body.includeAcceptUrls === true
                      ? { acceptUrl: acceptUrl(dependencies, env, token) }
                      : {}),
                    index,
                    invite: safeTeamWorkspaceInviteProjection(invite),
                    participantKind: invitation.participantKind,
                  }
                }),
              InvalidEmail: () =>
                Effect.fail(
                  badRequest('Every invitation needs a valid email.'),
                ),
              ProjectNotFound: () =>
                Effect.fail(
                  noStoreJsonResponse(
                    {
                      error: 'private_project_workspace_project_not_found',
                      reason: 'Project not found for the requested team.',
                    },
                    { status: 404 },
                  ),
                ),
              Refreshed: ({ invite, token }) =>
                Effect.gen(function* () {
                  const delivery = yield* sendInviteAndOperatorCopy(
                    dependencies,
                    env,
                    body.email,
                    invitation,
                    normalized.workspaceLabel,
                    copyOperator,
                    invite,
                    token,
                  )

                  return {
                    copy: delivery.copy,
                    email: delivery.email,
                    ...(body.includeAcceptUrls === true
                      ? { acceptUrl: acceptUrl(dependencies, env, token) }
                      : {}),
                    index,
                    invite: safeTeamWorkspaceInviteProjection(invite),
                    participantKind: invitation.participantKind,
                  }
                }),
              TeamNotFound: () =>
                Effect.fail(
                  noStoreJsonResponse(
                    {
                      error: 'private_project_workspace_team_not_found',
                      reason: 'Team not found.',
                    },
                    { status: 404 },
                  ),
                ),
            }),
            M.exhaustive,
          )
        }),
    )

    return noStoreJsonResponse(
      {
        generatedAt: routeNowIso(dependencies),
        invitations: invitationResults,
        project: projectView(privateProject.project),
        team: teamView(privateProject.team),
        workspace: workspaceView(workspace),
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(error)))

export const makePrivateProjectWorkspaceRoutes = <Bindings>(
  dependencies: PrivateProjectWorkspaceRoutesDependencies<Bindings>,
) => ({
  routePrivateProjectWorkspaceRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/private-project-workspaces') {
      return createPrivateProjectWorkspace(dependencies, request, env)
    }

    return undefined
  },
})
