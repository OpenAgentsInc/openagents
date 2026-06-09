import { Effect, Option, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from '../agent-registration'
import { parseJsonRecord } from '../json-boundary'
import { currentEpochMillis, currentIsoTimestamp } from '../runtime-primitives'
import {
  ForumActorSummary,
  type ForumActorSummary as ForumActorSummaryType,
  ForumUuid,
} from './schemas'

export const ForumWriterScope = S.Literals([
  'forum.bookmark',
  'forum.follow',
  'forum.notifications.read',
  'forum.read',
  'forum.watch',
  'forum.write',
  'forum.void.write',
])
export type ForumWriterScope = typeof ForumWriterScope.Type

export const ForumWriterActorKind = S.Literals(['human', 'agent', 'operator'])
export type ForumWriterActorKind = typeof ForumWriterActorKind.Type

export const ForumWriterAuthKind = S.Literals([
  'browser_session',
  'agent_bearer_token',
  'operator_test_grant',
])
export type ForumWriterAuthKind = typeof ForumWriterAuthKind.Type

export const ForumWriterAuthFailureKind = S.Literals([
  'missing_credentials',
  'malformed_credentials',
  'expired_credentials',
  'under_scoped',
  'wrong_owner',
  'wrong_team',
  'wrong_forum',
  'payment_not_authority',
  'unsafe_actor',
])
export type ForumWriterAuthFailureKind = typeof ForumWriterAuthFailureKind.Type

export const ForumWriterGrant = S.Struct({
  expiresAtEpochMillis: S.NullOr(S.Number),
  forumIds: S.Array(ForumUuid),
  ownerUserId: S.NullOr(S.String),
  scopes: S.Array(ForumWriterScope),
  status: S.Literals(['active', 'revoked']),
  teamId: S.NullOr(S.String),
})
export type ForumWriterGrant = typeof ForumWriterGrant.Type

export const ForumAgentGrant = S.Struct({
  expiresAt: S.NullOr(S.String),
  forumIds: S.Array(ForumUuid),
  grantId: S.optionalKey(S.String),
  ownerUserId: S.optionalKey(S.NullOr(S.String)),
  scopes: S.Array(ForumWriterScope),
  status: S.Literals(['active', 'revoked']),
  teamId: S.optionalKey(S.NullOr(S.String)),
})
export type ForumAgentGrant = typeof ForumAgentGrant.Type

export const ForumWriterContext = S.Struct({
  actor: ForumActorSummary,
  actorKind: ForumWriterActorKind,
  authKind: ForumWriterAuthKind,
  grantedScopes: S.Array(ForumWriterScope),
  targetForumId: ForumUuid,
})
export type ForumWriterContext = typeof ForumWriterContext.Type

export class ForumWriterAuthFailure extends S.TaggedErrorClass<ForumWriterAuthFailure>()(
  'ForumWriterAuthFailure',
  {
    failureKind: ForumWriterAuthFailureKind,
    reason: S.String,
  },
) {}

export type ForumHumanSessionActor = Readonly<{
  avatarUrl?: string | null
  email: string
  login: string
  name: string
  userId: string
}>

export type ForumOperatorActor = Readonly<{
  displayName: string
  operatorId: string
  slug: string
}>

export type ForumWriterActorInput =
  | Readonly<{
      _tag: 'Agent'
      session: ProgrammaticAgentSession
    }>
  | Readonly<{
      _tag: 'Human'
      session: ForumHumanSessionActor
    }>
  | Readonly<{
      _tag: 'Operator'
      operator: ForumOperatorActor
    }>

export type ForumWriterContextInput = Readonly<{
  actor: ForumWriterActorInput | undefined
  grant: ForumWriterGrant | undefined
  nowEpochMillis?: () => number
  paymentProofRef?: string | null
  requiredScope: ForumWriterScope
  targetForumId: string
  targetOwnerUserId?: string | null
  targetTeamId?: string | null
}>

const decodeForumActorSummary = S.decodeUnknownSync(ForumActorSummary)
const decodeForumAgentGrant = S.decodeUnknownOption(ForumAgentGrant)
const decodeForumWriterContext = S.decodeUnknownSync(ForumWriterContext)
const decodeForumWriterGrant = S.decodeUnknownSync(ForumWriterGrant)

const SLUG_UNSAFE_PATTERN = /[^a-z0-9-]+/g
const REPEATED_DASH_PATTERN = /-+/g

const compactSlug = (value: string, fallback: string): string => {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(SLUG_UNSAFE_PATTERN, '-')
    .replace(REPEATED_DASH_PATTERN, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 80)

  return slug === '' ? fallback : slug
}

const actorSummaryForInput = (
  actor: ForumWriterActorInput,
): ForumActorSummaryType => {
  if (actor._tag === 'Agent') {
    return decodeForumActorSummary({
      actorId: actor.session.user.id,
      actorRef: `agent:${actor.session.user.id}`,
      displayName: actor.session.user.displayName,
      groupRefs: ['agents'],
      isAgent: true,
      slug: compactSlug(actor.session.user.displayName, actor.session.user.id),
    })
  }

  if (actor._tag === 'Operator') {
    return decodeForumActorSummary({
      actorId: actor.operator.operatorId,
      actorRef: `operator:${actor.operator.operatorId}`,
      displayName: actor.operator.displayName,
      groupRefs: ['operators'],
      isAgent: false,
      slug: compactSlug(actor.operator.slug, actor.operator.operatorId),
    })
  }

  return decodeForumActorSummary({
    actorId: actor.session.userId,
    actorRef: `user:${actor.session.userId}`,
    displayName: actor.session.name,
    groupRefs: ['humans'],
    isAgent: false,
    slug: compactSlug(actor.session.login, actor.session.userId),
  })
}

const actorKindForInput = (
  actor: ForumWriterActorInput,
): ForumWriterActorKind =>
  actor._tag === 'Agent'
    ? 'agent'
    : actor._tag === 'Operator'
      ? 'operator'
      : 'human'

const authKindForInput = (actor: ForumWriterActorInput): ForumWriterAuthKind =>
  actor._tag === 'Agent'
    ? 'agent_bearer_token'
    : actor._tag === 'Operator'
      ? 'operator_test_grant'
      : 'browser_session'

const grantHasScope = (
  grant: ForumWriterGrant,
  scope: ForumWriterScope,
): boolean =>
  grant.scopes.includes(scope) ||
  (scope === 'forum.bookmark' && grant.scopes.includes('forum.write')) ||
  (scope === 'forum.follow' && grant.scopes.includes('forum.write')) ||
  (scope === 'forum.notifications.read' &&
    grant.scopes.includes('forum.read')) ||
  (scope === 'forum.watch' && grant.scopes.includes('forum.write')) ||
  (scope === 'forum.void.write' && grant.scopes.includes('forum.write'))

const grantAllowsForum = (
  grant: ForumWriterGrant,
  targetForumId: string,
): boolean => grant.forumIds.some(forumId => forumId === targetForumId)

export const forumAgentGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<ForumAgentGrant> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.forumGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeForumAgentGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

export const forumWriterGrantFromAgentGrant = (
  grant: ForumAgentGrant,
): ForumWriterGrant | undefined => {
  const expiresAtEpochMillis =
    grant.expiresAt === null ? null : Date.parse(grant.expiresAt)

  if (expiresAtEpochMillis !== null && Number.isNaN(expiresAtEpochMillis)) {
    return undefined
  }

  return decodeForumWriterGrant({
    expiresAtEpochMillis,
    forumIds: grant.forumIds,
    ownerUserId: grant.ownerUserId ?? null,
    scopes: grant.scopes,
    status: grant.status,
    teamId: grant.teamId ?? null,
  })
}

export const buildForumWriterContext = (
  input: ForumWriterContextInput,
): Effect.Effect<ForumWriterContext, ForumWriterAuthFailure> =>
  Effect.gen(function* () {
    if (input.actor === undefined) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'missing_credentials',
        reason:
          'Forum writes require an authenticated human, agent, or operator.',
      })
    }

    if (input.paymentProofRef !== undefined && input.paymentProofRef !== null) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'payment_not_authority',
        reason: 'Payment proof cannot replace Forum write permission.',
      })
    }

    if (input.grant === undefined || input.grant.status !== 'active') {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Forum write scope was not granted.',
      })
    }

    const nowEpochMillis = input.nowEpochMillis ?? currentEpochMillis

    if (
      input.grant.expiresAtEpochMillis !== null &&
      input.grant.expiresAtEpochMillis <= nowEpochMillis()
    ) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'expired_credentials',
        reason: 'Forum write grant is expired.',
      })
    }

    if (!grantHasScope(input.grant, input.requiredScope)) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Forum write grant does not include the required scope.',
      })
    }

    if (!grantAllowsForum(input.grant, input.targetForumId)) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'wrong_forum',
        reason: 'Forum write grant is not bound to the target forum.',
      })
    }

    if (
      input.targetOwnerUserId !== undefined &&
      input.targetOwnerUserId !== null &&
      input.grant.ownerUserId !== null &&
      input.grant.ownerUserId !== input.targetOwnerUserId
    ) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'wrong_owner',
        reason: 'Forum write grant is bound to a different owner.',
      })
    }

    if (
      input.targetTeamId !== undefined &&
      input.targetTeamId !== null &&
      input.grant.teamId !== null &&
      input.grant.teamId !== input.targetTeamId
    ) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'wrong_team',
        reason: 'Forum write grant is bound to a different team.',
      })
    }

    return decodeForumWriterContext({
      actor: actorSummaryForInput(input.actor),
      actorKind: actorKindForInput(input.actor),
      authKind: authKindForInput(input.actor),
      grantedScopes: input.grant.scopes,
      targetForumId: input.targetForumId,
    })
  })

export const authenticateForumAgentToken = (
  store: AgentRegistrationStore,
  bearerToken: string | undefined,
  nowIso: () => string = currentIsoTimestamp,
): Effect.Effect<ForumWriterActorInput, ForumWriterAuthFailure> => {
  if (bearerToken === undefined || bearerToken.trim() === '') {
    return Effect.fail(
      new ForumWriterAuthFailure({
        failureKind: 'missing_credentials',
        reason: 'Forum agent auth requires a bearer token.',
      }),
    )
  }

  if (!bearerToken.startsWith(AGENT_TOKEN_PREFIX)) {
    return Effect.fail(
      new ForumWriterAuthFailure({
        failureKind: 'malformed_credentials',
        reason: 'Forum agent bearer token has an invalid prefix.',
      }),
    )
  }

  return Effect.tryPromise({
    catch: error =>
      new ForumWriterAuthFailure({
        failureKind: 'malformed_credentials',
        reason:
          error instanceof Error
            ? error.message
            : 'Forum agent bearer token could not be checked.',
      }),
    try: () => authenticateProgrammaticAgent(store, bearerToken, nowIso),
  }).pipe(
    Effect.flatMap(session =>
      session === undefined
        ? Effect.fail(
            new ForumWriterAuthFailure({
              failureKind: 'expired_credentials',
              reason: 'Forum agent bearer token is not active.',
            }),
          )
        : Effect.succeed({
            _tag: 'Agent' as const,
            session,
          }),
    ),
  )
}

export const humanForumWriterActor = (
  session: ForumHumanSessionActor | undefined,
): Effect.Effect<ForumWriterActorInput, ForumWriterAuthFailure> =>
  session === undefined
    ? Effect.fail(
        new ForumWriterAuthFailure({
          failureKind: 'missing_credentials',
          reason: 'Forum human auth requires a browser session.',
        }),
      )
    : Effect.succeed({ _tag: 'Human', session })
