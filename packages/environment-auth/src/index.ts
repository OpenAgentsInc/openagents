/**
 * ENV-2 (openagents #8780): typed contracts for DPoP-bound, scope-limited
 * capability tokens on local runtime sockets and Khala Sync device grants.
 *
 * This package carries the auth-metadata language the ENV-1 vocabulary
 * (docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md,
 * "Environment and endpoint vocabulary") explicitly deferred:
 *
 * - capability scopes drawn from the EXISTING OpenAgents MCP authority-class
 *   vocabulary (`@openagentsinc/mcp-contract`), not a copy of any external
 *   scope strings;
 * - RFC 8693-shaped token exchange from an environment-bootstrap subject
 *   token, with a narrowing-only scope evaluator (an exchange can never
 *   widen scopes);
 * - DPoP (RFC 9449) proof claims and RFC 7638 key-thumbprint binding (see
 *   `./dpop`); and
 * - grant records bound to an owner-scoped ExecutionEnvironment identity
 *   (ENV-1 `ExecutionEnvironmentRef`), never to a hostname, address, or
 *   AccessEndpoint.
 *
 * Raw token or key material never appears in these records; every grant is
 * refs + thumbprint only (`tokenMaterial: "excluded"`).
 */
import { Schema as S } from "effect"
import type { OpenAgentsMcpAuthorityClass } from "@openagentsinc/mcp-contract"
import {
  ExecutionEnvironmentRef,
  PortableRef,
  PortableTimestamp,
} from "@openagentsinc/portable-session-contract"

import { DpopSha256Base64Url } from "./dpop.js"

export const ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION =
  "openagents.environment_token_exchange.v1" as const
export const ENVIRONMENT_CAPABILITY_GRANT_SCHEMA_VERSION =
  "openagents.environment_capability_grant.v1" as const

/**
 * The scope vocabulary for environment sockets IS a subset of the existing
 * OpenAgents MCP authority classes — `satisfies` keeps the two vocabularies
 * aligned at compile time. Mapping to what a local runtime socket does:
 *
 * - `operator_read` — read environment snapshots, status, events, health.
 * - `workspace_read` — read work-context filesystem/VCS/tab state.
 * - `workspace_write` — dispatch operations that mutate workspace state.
 * - `local_node_control` — terminal/process/runtime control on the node.
 * - `coding_session_control` — start/steer/stop coding sessions.
 * - `approval_resolution` — resolve approvals routed to this client.
 * - `admin` — pairing/grant administration (bootstrap credentials only).
 */
export const ENVIRONMENT_CAPABILITY_SCOPES = [
  "operator_read",
  "workspace_read",
  "workspace_write",
  "local_node_control",
  "coding_session_control",
  "approval_resolution",
  "admin",
] as const satisfies ReadonlyArray<OpenAgentsMcpAuthorityClass>

export const EnvironmentCapabilityScope = S.Literals(
  ENVIRONMENT_CAPABILITY_SCOPES,
)
export type EnvironmentCapabilityScope =
  typeof EnvironmentCapabilityScope.Type

/**
 * Default scope set for an ordinary paired client. `admin` (pairing/grant
 * administration) stays exclusive to bootstrap credentials, so an ordinary
 * client token can never be exchanged into grant administration.
 */
export const ENVIRONMENT_CLIENT_DEFAULT_SCOPES: ReadonlyArray<EnvironmentCapabilityScope> = [
  "operator_read",
  "workspace_read",
  "workspace_write",
  "local_node_control",
  "coding_session_control",
  "approval_resolution",
]

/** Scope set carried by an environment-bootstrap credential. */
export const ENVIRONMENT_BOOTSTRAP_SCOPES: ReadonlyArray<EnvironmentCapabilityScope> =
  ENVIRONMENT_CAPABILITY_SCOPES

// RFC 8693 vocabulary. The bootstrap subject-token type is OpenAgents-private,
// exactly like the environment-bootstrap pattern this profiles.
export const OPENAGENTS_TOKEN_EXCHANGE_GRANT_TYPE =
  "urn:ietf:params:oauth:grant-type:token-exchange" as const
export const OPENAGENTS_ENVIRONMENT_BOOTSTRAP_TOKEN_TYPE =
  "urn:openagents:params:oauth:token-type:environment-bootstrap" as const
export const OPENAGENTS_ACCESS_TOKEN_TYPE =
  "urn:ietf:params:oauth:token-type:access_token" as const

/**
 * RFC 8693-shaped exchange request: a one-time environment-bootstrap subject
 * token is exchanged for a scope-limited access token. The requested token is
 * DPoP-bound from birth: `clientKeyThumbprint` (RFC 7638) is required, so a
 * bearer-shaped token can never fall out of this exchange.
 */
export const EnvironmentTokenExchangeRequest = S.Struct({
  schema: S.Literal(ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION),
  grantType: S.Literal(OPENAGENTS_TOKEN_EXCHANGE_GRANT_TYPE),
  subjectToken: S.String.check(S.isMinLength(16), S.isMaxLength(4096)),
  subjectTokenType: S.Literal(OPENAGENTS_ENVIRONMENT_BOOTSTRAP_TOKEN_TYPE),
  requestedTokenType: S.Literal(OPENAGENTS_ACCESS_TOKEN_TYPE),
  /** Empty means "inherit the full subject-token scope set" (RFC 8693 §2.1). */
  scopes: S.Array(EnvironmentCapabilityScope),
  clientKeyThumbprint: DpopSha256Base64Url,
  /** Presentation hint only — never an authorization input. */
  clientLabel: S.optionalKey(S.String.check(S.isMinLength(1), S.isMaxLength(120))),
})
export type EnvironmentTokenExchangeRequest =
  typeof EnvironmentTokenExchangeRequest.Type

export const EnvironmentTokenExchangeResponse = S.Struct({
  schema: S.Literal(ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION),
  accessToken: S.String.check(S.isMinLength(16), S.isMaxLength(4096)),
  issuedTokenType: S.Literal(OPENAGENTS_ACCESS_TOKEN_TYPE),
  /** Never "Bearer": issued environment tokens are proof-of-possession bound. */
  tokenType: S.Literal("DPoP"),
  expiresInSeconds: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  scopes: S.Array(EnvironmentCapabilityScope),
  clientKeyThumbprint: DpopSha256Base64Url,
  grantRef: PortableRef,
})
export type EnvironmentTokenExchangeResponse =
  typeof EnvironmentTokenExchangeResponse.Type

/**
 * Durable record of one issued environment capability grant, bound to the
 * owner-scoped ExecutionEnvironment identity (ENV-1) it authorizes access to
 * and to the client key thumbprint that must prove possession on use.
 * `sourceGrantRef` records exchange lineage; every link in that chain is
 * narrowing-only by `evaluateEnvironmentScopeExchange`.
 */
export const EnvironmentCapabilityGrant = S.Struct({
  schema: S.Literal(ENVIRONMENT_CAPABILITY_GRANT_SCHEMA_VERSION),
  grantRef: PortableRef,
  ownerRef: PortableRef,
  environmentRef: ExecutionEnvironmentRef,
  scopes: S.Array(EnvironmentCapabilityScope).check(S.isMinLength(1)),
  clientKeyThumbprint: DpopSha256Base64Url,
  sourceGrantRef: S.optionalKey(PortableRef),
  clientLabel: S.optionalKey(S.String.check(S.isMinLength(1), S.isMaxLength(120))),
  issuedAt: PortableTimestamp,
  expiresAt: PortableTimestamp,
  state: S.Literals(["active", "revoked", "expired"]),
  /** Raw token/key material never enters a grant record. */
  tokenMaterial: S.Literal("excluded"),
})
export type EnvironmentCapabilityGrant =
  typeof EnvironmentCapabilityGrant.Type

export const decodeEnvironmentTokenExchangeRequest = S.decodeUnknownSync(
  EnvironmentTokenExchangeRequest,
)
export const decodeEnvironmentTokenExchangeResponse = S.decodeUnknownSync(
  EnvironmentTokenExchangeResponse,
)
export const decodeEnvironmentCapabilityGrant = S.decodeUnknownSync(
  EnvironmentCapabilityGrant,
)

export type EnvironmentScopeExchangeDecision =
  | {
      readonly ok: true
      /** Always a subset of the subject scopes — never a widening. */
      readonly grantedScopes: ReadonlyArray<EnvironmentCapabilityScope>
    }
  | {
      readonly ok: false
      readonly reason: "scope_widening_rejected" | "subject_scopes_empty"
      readonly offendingScopes: ReadonlyArray<EnvironmentCapabilityScope>
    }

const dedupeSorted = (
  scopes: ReadonlyArray<EnvironmentCapabilityScope>,
): ReadonlyArray<EnvironmentCapabilityScope> =>
  [...new Set(scopes)].sort((left, right) => left.localeCompare(right))

/**
 * Narrowing-only scope exchange (RFC 8693 subset semantics): the granted set
 * is always a subset of the subject-token scopes. Requesting any scope the
 * subject token does not hold rejects the whole exchange — it is never
 * silently down-scoped, so a client cannot mistake a partial grant for the
 * authority it asked for. An empty request inherits the full subject set.
 */
export function evaluateEnvironmentScopeExchange(input: {
  readonly subjectScopes: ReadonlyArray<EnvironmentCapabilityScope>
  readonly requestedScopes: ReadonlyArray<EnvironmentCapabilityScope>
}): EnvironmentScopeExchangeDecision {
  const subject = dedupeSorted(input.subjectScopes)
  if (subject.length === 0) {
    return { ok: false, reason: "subject_scopes_empty", offendingScopes: [] }
  }
  const requested = dedupeSorted(input.requestedScopes)
  if (requested.length === 0) {
    return { ok: true, grantedScopes: subject }
  }
  const subjectSet = new Set(subject)
  const offendingScopes = requested.filter((scope) => !subjectSet.has(scope))
  if (offendingScopes.length > 0) {
    return { ok: false, reason: "scope_widening_rejected", offendingScopes }
  }
  return { ok: true, grantedScopes: requested }
}

export * from "./broker-verifier.js"
export * from "./dpop.js"
