import { Effect, Option, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import { parseJsonRecord } from './json-boundary'

export const CustomerOrderAgentScope = S.Literals([
  'customer_orders.read',
  'customer_orders.write',
  'customer_orders.feedback',
])
export type CustomerOrderAgentScope = typeof CustomerOrderAgentScope.Type

export const CustomerOrderAgentAuthFailureKind = S.Literals([
  'missing_credentials',
  'malformed_credentials',
  'expired_credentials',
  'under_scoped',
  'wrong_owner',
])
export type CustomerOrderAgentAuthFailureKind =
  typeof CustomerOrderAgentAuthFailureKind.Type

export const CustomerOrderAgentGrant = S.Struct({
  expiresAt: S.NullOr(S.String),
  grantId: S.optionalKey(S.String),
  ownerUserId: S.String,
  scopes: S.Array(CustomerOrderAgentScope),
  status: S.Literals(['active', 'revoked']),
})
export type CustomerOrderAgentGrant = typeof CustomerOrderAgentGrant.Type

export type CustomerOrderAgentContext = Readonly<{
  agent: ProgrammaticAgentSession
  grant: CustomerOrderAgentGrant
  ownerUserId: string
  grantedScopes: ReadonlyArray<CustomerOrderAgentScope>
}>

export class CustomerOrderAgentAuthFailure extends S.TaggedErrorClass<CustomerOrderAgentAuthFailure>()(
  'CustomerOrderAgentAuthFailure',
  {
    failureKind: CustomerOrderAgentAuthFailureKind,
    reason: S.String,
  },
) {}

const decodeGrant = S.decodeUnknownOption(CustomerOrderAgentGrant)

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token
    : undefined
}

const customerOrderGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<CustomerOrderAgentGrant> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.customerOrderGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

const grantHasScope = (
  grant: CustomerOrderAgentGrant,
  scope: CustomerOrderAgentScope,
): boolean =>
  grant.scopes.includes(scope) ||
  (scope === 'customer_orders.feedback' &&
    grant.scopes.includes('customer_orders.write'))

const grantIsExpired = (
  grant: CustomerOrderAgentGrant,
  nowIso: () => string,
): boolean => grant.expiresAt !== null && grant.expiresAt <= nowIso()

export const authenticateCustomerOrderAgentRequest = (
  request: Request,
  store: AgentRegistrationStore | undefined,
  input: Readonly<{
    nowIso: () => string
    ownerUserId?: string
    requiredScope: CustomerOrderAgentScope
  }>,
): Effect.Effect<CustomerOrderAgentContext, CustomerOrderAgentAuthFailure> => {
  const bearerToken = bearerTokenFromRequest(request)

  if (bearerToken === undefined || bearerToken.trim() === '') {
    return Effect.fail(
      new CustomerOrderAgentAuthFailure({
        failureKind: 'missing_credentials',
        reason: 'Customer order agent auth requires a bearer token.',
      }),
    )
  }

  if (!bearerToken.startsWith(AGENT_TOKEN_PREFIX)) {
    return Effect.fail(
      new CustomerOrderAgentAuthFailure({
        failureKind: 'malformed_credentials',
        reason: 'Customer order agent bearer token has an invalid prefix.',
      }),
    )
  }

  if (store === undefined) {
    return Effect.fail(
      new CustomerOrderAgentAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Customer order agent auth is not configured.',
      }),
    )
  }

  return Effect.tryPromise({
    catch: error =>
      new CustomerOrderAgentAuthFailure({
        failureKind: 'malformed_credentials',
        reason:
          error instanceof Error
            ? error.message
            : 'Customer order agent bearer token could not be checked.',
      }),
    try: () => authenticateProgrammaticAgent(store, bearerToken, input.nowIso),
  }).pipe(
    Effect.flatMap(session => {
      if (session === undefined) {
        return Effect.fail(
          new CustomerOrderAgentAuthFailure({
            failureKind: 'expired_credentials',
            reason: 'Customer order agent bearer token is not active.',
          }),
        )
      }

      const grants = customerOrderGrantsFromSession(session)
      const grant = grants.find(candidate => {
        if (candidate.status !== 'active') {
          return false
        }

        if (grantIsExpired(candidate, input.nowIso)) {
          return false
        }

        if (!grantHasScope(candidate, input.requiredScope)) {
          return false
        }

        return input.ownerUserId === undefined
          ? true
          : candidate.ownerUserId === input.ownerUserId
      })

      if (grant === undefined) {
        return Effect.fail(
          new CustomerOrderAgentAuthFailure({
            failureKind: 'under_scoped',
            reason:
              'Customer order grant is missing, expired, revoked, owner-mismatched, or under-scoped.',
          }),
        )
      }

      if (
        input.ownerUserId !== undefined &&
        grant.ownerUserId !== input.ownerUserId
      ) {
        return Effect.fail(
          new CustomerOrderAgentAuthFailure({
            failureKind: 'wrong_owner',
            reason: 'Customer order grant is bound to a different owner.',
          }),
        )
      }

      return Effect.succeed({
        agent: session,
        grant,
        grantedScopes: grant.scopes,
        ownerUserId: grant.ownerUserId,
      })
    }),
  )
}
