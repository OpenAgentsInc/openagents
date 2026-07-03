import { Effect, Schema as S } from 'effect'

import type { ResendEmailConfig } from './config'
import { PrivateWorkspaceInviteEmailInput } from './email'
import type { EmailLedgerSendResult, EmailServiceError } from './email'
import { makeD1PrivateProjectWorkspaceStore } from './private-project-workspace-routes'
import {
  makeEcommerceDesignPartnerWorkspaceInput,
  makeLegalDesignPartnerWorkspaceInput,
  makeMarketingAgencyDesignPartnerWorkspaceInput,
} from './prefilled-workspace-vertical-templates'
import {
  type BusinessSignupRecord,
  type BusinessSignupRuntime,
} from './business-signup-routes'
import type {
  CreatePrefilledWorkspaceInput,
  PrefilledWorkspaceRecord,
  PrefilledWorkspaceServiceShape,
  SeededMemoryEntry,
  StarterWorkflow,
} from './prefilled-workspace'
import { makePrefilledWorkspaceService } from './prefilled-workspace'
import {
  type TeamWorkspaceInviteCreateResult,
  type TeamWorkspaceInviteStore,
  makeD1TeamWorkspaceInviteStore,
} from './team-workspace-invites'

export type BusinessSignupFulfillmentStatus =
  | 'invited'
  | 'operator_parked'

type EmailDeliveryStatus =
  | 'accepted'
  | 'disabled'
  | 'failed'
  | 'missing_config'
  | 'not_attempted'

export type BusinessSignupFulfillmentRecord = Readonly<{
  emailDeliveryStatus: EmailDeliveryStatus
  emailMessageId: string | null
  enrichmentRef: string
  id: string
  inviteId: string | null
  projectId: string | null
  reason: string | null
  signupId: string
  status: BusinessSignupFulfillmentStatus
  teamId: string | null
  updatedAt: string
  workspaceId: string | null
}>

type BusinessSignupFulfillmentDependencies = Readonly<{
  appOrigin: string
  getResendEmailConfig?: () => ResendEmailConfig | undefined
  inviteStore: TeamWorkspaceInviteStore
  privateProjectStore: ReturnType<typeof makeD1PrivateProjectWorkspaceStore>
  runtime: BusinessSignupRuntime
  sendInviteEmailWithLedger?: (
    config: ResendEmailConfig,
    input: PrivateWorkspaceInviteEmailInput,
  ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>
  workspaceStore: PrefilledWorkspaceServiceShape
}>

export type BusinessSignupFulfillmentOptions = Readonly<{
  appOrigin?: string | undefined
  getResendEmailConfig?: (() => ResendEmailConfig | undefined) | undefined
  inviteStore?: TeamWorkspaceInviteStore | undefined
  privateProjectStore?: ReturnType<typeof makeD1PrivateProjectWorkspaceStore>
  sendInviteEmailWithLedger?:
    | ((
        config: ResendEmailConfig,
        input: PrivateWorkspaceInviteEmailInput,
      ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>)
    | undefined
  workspaceStore?: PrefilledWorkspaceServiceShape | undefined
}>

type FulfillmentRow = Readonly<{
  email_delivery_status: EmailDeliveryStatus
  email_message_id: string | null
  enrichment_ref: string
  id: string
  invite_id: string | null
  project_id: string | null
  reason: string | null
  status: BusinessSignupFulfillmentStatus
  team_id: string | null
  updated_at: string
  workspace_id: string | null
}>

class BusinessSignupFulfillmentFailure extends S.TaggedErrorClass<BusinessSignupFulfillmentFailure>()(
  'BusinessSignupFulfillmentFailure',
  {
    cause: S.Unknown,
  },
) {}

const promiseEffect = <A>(
  tryPromise: () => Promise<A>,
): Effect.Effect<A, BusinessSignupFulfillmentFailure> =>
  Effect.tryPromise({
    catch: cause => new BusinessSignupFulfillmentFailure({ cause }),
    try: tryPromise,
  })

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const slugFromText = (value: string): string => {
  const slug = compactText(value, 80)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 70)

  return slug === '' ? 'business-signup' : slug
}

const fulfillmentRef = (signupId: string): string =>
  `business_signup_fulfillment:${signupId}`

const enrichmentRef = (signupId: string): string =>
  `business_signup_enrichment:${signupId}:v1`

const sourceRef = (signupId: string): string =>
  `business_signup_request:${signupId}`

const readFulfillment = async (
  db: D1Database,
  signupId: string,
): Promise<BusinessSignupFulfillmentRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, business_signup_request_id, status, reason, enrichment_ref,
              team_id, project_id, workspace_id, invite_id, email_message_id,
              email_delivery_status, updated_at
         FROM business_signup_fulfillments
        WHERE business_signup_request_id = ?
        LIMIT 1`,
    )
    .bind(signupId)
    .first<FulfillmentRow & Readonly<{ business_signup_request_id: string }>>()

  return row === null
    ? undefined
    : {
        emailDeliveryStatus: row.email_delivery_status,
        emailMessageId: row.email_message_id,
        enrichmentRef: row.enrichment_ref,
        id: row.id,
        inviteId: row.invite_id,
        projectId: row.project_id,
        reason: row.reason,
        signupId: row.business_signup_request_id,
        status: row.status,
        teamId: row.team_id,
        updatedAt: row.updated_at,
        workspaceId: row.workspace_id,
      }
}

const writeFulfillment = async (
  db: D1Database,
  input: BusinessSignupFulfillmentRecord,
  metadata: Readonly<Record<string, unknown>>,
): Promise<BusinessSignupFulfillmentRecord> => {
  await db
    .prepare(
      `INSERT INTO business_signup_fulfillments
        (id, business_signup_request_id, status, reason, enrichment_ref,
         team_id, project_id, workspace_id, invite_id, email_message_id,
         email_delivery_status, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(business_signup_request_id) DO UPDATE SET
         status = excluded.status,
         reason = excluded.reason,
         enrichment_ref = excluded.enrichment_ref,
         team_id = excluded.team_id,
         project_id = excluded.project_id,
         workspace_id = excluded.workspace_id,
         invite_id = excluded.invite_id,
         email_message_id = excluded.email_message_id,
         email_delivery_status = excluded.email_delivery_status,
         metadata_json = excluded.metadata_json,
         updated_at = excluded.updated_at`,
    )
    .bind(
      input.id,
      input.signupId,
      input.status,
      input.reason,
      input.enrichmentRef,
      input.teamId,
      input.projectId,
      input.workspaceId,
      input.inviteId,
      input.emailMessageId,
      input.emailDeliveryStatus,
      JSON.stringify(metadata),
      input.updatedAt,
      input.updatedAt,
    )
    .run()

  await db
    .prepare(
      `UPDATE business_signup_requests
          SET fulfillment_status = ?,
              fulfillment_ref = ?,
              fulfillment_reason = ?,
              updated_at = ?
        WHERE id = ?`,
    )
    .bind(input.status, input.id, input.reason, input.updatedAt, input.signupId)
    .run()

  return input
}

const fulfillmentDependencies = (
  db: D1Database,
  runtime: BusinessSignupRuntime,
  options: BusinessSignupFulfillmentOptions,
): BusinessSignupFulfillmentDependencies => {
  const dependencies: BusinessSignupFulfillmentDependencies = {
    appOrigin: options.appOrigin ?? 'https://openagents.com',
    inviteStore: options.inviteStore ?? makeD1TeamWorkspaceInviteStore(db),
    privateProjectStore:
      options.privateProjectStore ?? makeD1PrivateProjectWorkspaceStore(db),
    runtime,
    workspaceStore: options.workspaceStore ?? makePrefilledWorkspaceService(db),
  }

  return {
    ...dependencies,
    ...(options.getResendEmailConfig === undefined
      ? {}
      : { getResendEmailConfig: options.getResendEmailConfig }),
    ...(options.sendInviteEmailWithLedger === undefined
      ? {}
      : { sendInviteEmailWithLedger: options.sendInviteEmailWithLedger }),
  }
}

const verticalTemplateForSignup = (
  signup: BusinessSignupRecord,
): CreatePrefilledWorkspaceInput => {
  const text =
    `${signup.businessName} ${signup.website ?? ''} ${signup.helpWith ?? ''}`.toLowerCase()

  if (/legal|law|attorney|contract|nda|intake/.test(text)) {
    return makeLegalDesignPartnerWorkspaceInput()
  }

  if (/agency|marketing|landing|email|white.?label|funnel/.test(text)) {
    return makeMarketingAgencyDesignPartnerWorkspaceInput()
  }

  return makeEcommerceDesignPartnerWorkspaceInput()
}

const appendSignupMemory = (
  template: CreatePrefilledWorkspaceInput,
  signup: BusinessSignupRecord,
): ReadonlyArray<SeededMemoryEntry> => [
  ...(template.seededMemory ?? []),
  {
    label: 'Intake source',
    publicSourceRef: sourceRef(signup.id),
    value:
      'Created from a /business signup. Contact details stay in the private invite ledger; workspace seed material stays public-safe until the customer signs in.',
  },
  {
    label: 'Requested help',
    publicSourceRef: sourceRef(signup.id),
    value: compactText(signup.helpWith ?? 'General business automation intake.', 600),
  },
]

const appendSignupWorkflows = (
  template: CreatePrefilledWorkspaceInput,
): ReadonlyArray<StarterWorkflow> => [
  ...(template.starterWorkflows ?? []).slice(0, 2),
  {
    description:
      'Confirm the requested outcome, missing access, approval gates, and first receipt plan before any external send, publish, or spend.',
    outcomeKind: 'business_intake_scope_confirmation',
    status: 'queued',
    title: 'Confirm first scope',
  },
]

const workspaceInputForSignup = (
  signup: BusinessSignupRecord,
  teamId: string,
  projectId: string,
): CreatePrefilledWorkspaceInput => {
  const template = verticalTemplateForSignup(signup)

  return {
    ...template,
    accessMode: 'private_team',
    holderRef: `business_signup.${signup.id}`,
    introReceipt: {
      publicSourceRefs: [
        ...(template.introReceipt.publicSourceRefs ?? []),
        sourceRef(signup.id),
      ],
      summary:
        'Provisioned from the /business intake path with public-safe template seed material, an explicit scope-confirmation starter, and no external authority until review receipts exist.',
    },
    privateProjectId: projectId,
    privateTeamId: teamId,
    projectName: `${compactText(signup.businessName, 80)} Workspace`,
    seededMemory: appendSignupMemory(template, signup),
    starterWorkflows: appendSignupWorkflows(template),
    status: 'invited',
  }
}

const acceptUrl = (appOrigin: string, token: string): string =>
  `${appOrigin}/api/team-workspace-invites/accept?token=${encodeURIComponent(token)}`

const inviteEmailInput = (
  signup: BusinessSignupRecord,
  invite: Extract<
    TeamWorkspaceInviteCreateResult,
    { _tag: 'Created' | 'Refreshed' }
  >['invite'],
  token: string,
  appOrigin: string,
): PrivateWorkspaceInviteEmailInput =>
  new PrivateWorkspaceInviteEmailInput({
    acceptUrl: acceptUrl(appOrigin, token),
    displayName: 'there',
    expiresAt: invite.expiresAt,
    idempotencyKey: `business_signup_invite:${signup.id}:${invite.sendCount + 1}`,
    inviteId: invite.id,
    projectId: invite.projectId,
    teamId: invite.teamId,
    to: invite.inviteeEmail,
    workspaceLabel: `${compactText(signup.businessName, 80)} OpenAgents workspace`,
  })

const parked = (
  db: D1Database,
  dependencies: BusinessSignupFulfillmentDependencies,
  signup: BusinessSignupRecord,
  reason: string,
  partial: Partial<BusinessSignupFulfillmentRecord> = {},
): Promise<BusinessSignupFulfillmentRecord> => {
  const now = dependencies.runtime.nowIso()

  return writeFulfillment(
    db,
    {
      emailDeliveryStatus: partial.emailDeliveryStatus ?? 'not_attempted',
      emailMessageId: partial.emailMessageId ?? null,
      enrichmentRef: enrichmentRef(signup.id),
      id: fulfillmentRef(signup.id),
      inviteId: partial.inviteId ?? null,
      projectId: partial.projectId ?? null,
      reason,
      signupId: signup.id,
      status: 'operator_parked',
      teamId: partial.teamId ?? null,
      updatedAt: now,
      workspaceId: partial.workspaceId ?? null,
    },
    { reason, sourceIssue: 8074 },
  )
}

const sendInvite = (
  dependencies: BusinessSignupFulfillmentDependencies,
  signup: BusinessSignupRecord,
  invite: Extract<
    TeamWorkspaceInviteCreateResult,
    { _tag: 'Created' | 'Refreshed' }
  >['invite'],
  token: string,
): Effect.Effect<
  Readonly<{
    emailDeliveryStatus: EmailDeliveryStatus
    emailMessageId: string | null
    reason: string | null
  }>
> => {
  const config = dependencies.getResendEmailConfig?.()

  if (
    config === undefined ||
    dependencies.sendInviteEmailWithLedger === undefined
  ) {
    return Effect.succeed({
      emailDeliveryStatus: 'missing_config',
      emailMessageId: null,
      reason: 'business_signup_invite_email_config_missing',
    })
  }

  return dependencies
    .sendInviteEmailWithLedger(
      config,
      inviteEmailInput(signup, invite, token, dependencies.appOrigin),
    )
    .pipe(
      Effect.flatMap(result =>
        Effect.gen(function* () {
          if (result.emailMessageId !== null) {
            yield* Effect.tryPromise({
              catch: () => undefined,
              try: () =>
                dependencies.inviteStore.recordEmailAttempt({
                  attemptedAt: dependencies.runtime.nowIso(),
                  emailMessageId: result.emailMessageId,
                  inviteId: invite.id,
                }),
            }).pipe(Effect.catch(() => Effect.void))
          }

          return result.ok
            ? {
                emailDeliveryStatus: 'accepted' as const,
                emailMessageId: result.emailMessageId,
                reason: null,
              }
            : {
                emailDeliveryStatus: 'failed' as const,
                emailMessageId: result.emailMessageId,
                reason: result.errorName ?? 'business_signup_invite_email_failed',
              }
        }),
      ),
      Effect.catch(() =>
        Effect.succeed({
          emailDeliveryStatus: 'failed' as const,
          emailMessageId: null,
          reason: 'business_signup_invite_email_failed',
        }),
      ),
    )
}

export const fulfillBusinessSignup = (
  db: D1Database,
  signup: BusinessSignupRecord,
  runtime: BusinessSignupRuntime,
  options: BusinessSignupFulfillmentOptions = {},
): Effect.Effect<BusinessSignupFulfillmentRecord> => {
  const dependencies = fulfillmentDependencies(db, runtime, options)
  const parkForException = () =>
    Effect.promise(() =>
      parked(
        db,
        dependencies,
        signup,
        'business_signup_fulfillment_exception',
      ),
    )

  return Effect.gen(function* () {
    const existing = yield* promiseEffect(() => readFulfillment(db, signup.id))

    if (existing !== undefined) {
      return existing
    }

    const slugSuffix = `${slugFromText(signup.businessName)}-${signup.id
      .replace(/[^A-Za-z0-9]+/g, '-')
      .toLowerCase()
      .slice(-16)}`
    const privateProject = yield* promiseEffect(() =>
      dependencies.privateProjectStore.createOrUpdateProject({
        description: 'Business signup private workspace.',
        projectName: `${compactText(signup.businessName, 100)} Intake`,
        projectSlug: slugSuffix,
        teamName: `${compactText(signup.businessName, 100)} Team`,
        teamSlug: `${slugSuffix}-team`,
      }),
    )
    const existingWorkspace = yield* promiseEffect(() =>
      dependencies.workspaceStore.readPrivateWorkspaceByTarget(
        privateProject.team.id,
        privateProject.project.id,
      ),
    )
    const workspace: PrefilledWorkspaceRecord =
      existingWorkspace ??
      (yield* promiseEffect(() =>
        dependencies.workspaceStore.createWorkspace(
          workspaceInputForSignup(
            signup,
            privateProject.team.id,
            privateProject.project.id,
          ),
        ),
      ))
    const inviteResult = yield* promiseEffect(() =>
      dependencies.inviteStore.createOrRefreshInvite({
        email: signup.contactEmail,
        invitedByActorRef: 'system:business_signup_fulfillment',
        metadataJson: JSON.stringify({
          businessSignupRequestId: signup.id,
          source: 'business_signup_fulfillment',
          sourceIssue: 8074,
          workspaceId: workspace.id,
        }),
        projectId: privateProject.project.id,
        role: 'member',
        teamId: privateProject.team.id,
      }),
    )

    if (inviteResult._tag !== 'Created' && inviteResult._tag !== 'Refreshed') {
      return yield* promiseEffect(() =>
        parked(
          db,
          dependencies,
          signup,
          `business_signup_invite_${inviteResult._tag.toLowerCase()}`,
          {
            projectId: privateProject.project.id,
            teamId: privateProject.team.id,
            workspaceId: workspace.id,
          },
        ),
      )
    }

    const delivery = yield* sendInvite(
      dependencies,
      signup,
      inviteResult.invite,
      inviteResult.token,
    )
    const status =
      delivery.emailDeliveryStatus === 'accepted'
        ? 'invited'
        : 'operator_parked'
    const reason =
      status === 'invited'
        ? null
        : (delivery.reason ?? 'business_signup_invite_not_sent')

    return yield* promiseEffect(() =>
      writeFulfillment(
        db,
        {
          emailDeliveryStatus: delivery.emailDeliveryStatus,
          emailMessageId: delivery.emailMessageId,
          enrichmentRef: enrichmentRef(signup.id),
          id: fulfillmentRef(signup.id),
          inviteId: inviteResult.invite.id,
          projectId: privateProject.project.id,
          reason,
          signupId: signup.id,
          status,
          teamId: privateProject.team.id,
          updatedAt: dependencies.runtime.nowIso(),
          workspaceId: workspace.id,
        },
        {
          sourceIssue: 8074,
          workspaceStatus: workspace.status,
        },
      ),
    )
  }).pipe(Effect.catch(() => parkForException()))
}
