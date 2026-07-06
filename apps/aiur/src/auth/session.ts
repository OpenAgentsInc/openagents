import { createClient } from '@openauthjs/openauth/client'

import { openAuthClientId, openAuthIssuerUrl, type AiurEnv } from './config'
import { aiurSubjects, type AiurUserSubject } from './subjects'

export type AiurTokens = Readonly<{ access: string; refresh: string }>

export type AiurVerifiedSession = Readonly<{
  user: AiurUserSubject
  tokens?: AiurTokens
}>

export type AiurAuthClientDeps = Readonly<{
  fetch?: typeof fetch
}>

export const makeAiurAuthClient = (
  env: AiurEnv,
  deps: AiurAuthClientDeps = {},
) =>
  createClient({
    clientID: openAuthClientId(env),
    issuer: openAuthIssuerUrl(env),
    ...(deps.fetch === undefined ? {} : { fetch: deps.fetch }),
  })

export type AiurAuthClientLike = Readonly<{
  verify: ReturnType<typeof makeAiurAuthClient>['verify']
}>

/**
 * Verifies an Aiur session's OpenAuth access token (with optional refresh)
 * against the shared `auth.openagents.com` issuer. Returns `undefined` for
 * ANY failure — expired/invalid/network — never throws, so callers always
 * get an explicit "no session" rather than an uncaught rejection.
 */
export const verifyAiurSession = async (
  access: string,
  refresh: string | undefined,
  client: AiurAuthClientLike,
): Promise<AiurVerifiedSession | undefined> => {
  const verified = await (refresh === undefined
    ? client.verify(aiurSubjects, access)
    : client.verify(aiurSubjects, access, { refresh })
  ).catch(() => undefined)

  if (verified === undefined) {
    return undefined
  }

  if (verified.err !== undefined) {
    return undefined
  }

  if (verified.subject.type !== 'user') {
    return undefined
  }

  if (verified.tokens === undefined) {
    return { user: verified.subject.properties }
  }

  return { user: verified.subject.properties, tokens: verified.tokens }
}
