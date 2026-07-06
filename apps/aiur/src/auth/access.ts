import {
  AIUR_ACCESS_COOKIE,
  AIUR_REFRESH_COOKIE,
  parseCookies,
} from './cookies'
import { type AiurEnv } from './config'
import { isAllowedOwnerUserId, parseOwnerAllowlist } from './owner-gate'
import {
  type AiurAuthClientLike,
  makeAiurAuthClient,
  verifyAiurSession,
  type AiurVerifiedSession,
} from './session'

/**
 * The one place Aiur decides "can this request see anything". Every other
 * route (the dashboard page, the credits console, the ops views, the Khala
 * Sync proxy) must funnel through this and treat anything other than
 * `{ kind: "owner" }` as a hard deny — FAIL CLOSED.
 */
export type AiurAccess =
  | Readonly<{ kind: 'signed_out' }>
  | Readonly<{ kind: 'denied'; user: AiurVerifiedSession['user'] }>
  | Readonly<{ kind: 'owner'; session: AiurVerifiedSession }>

export type ResolveAiurAccessDeps = Readonly<{
  client?: AiurAuthClientLike
}>

export const resolveAiurAccess = async (
  request: Request,
  env: AiurEnv,
  deps: ResolveAiurAccessDeps = {},
): Promise<AiurAccess> => {
  const cookies = parseCookies(request)
  const access = cookies.get(AIUR_ACCESS_COOKIE)

  if (access === undefined) {
    return { kind: 'signed_out' }
  }

  const client = deps.client ?? makeAiurAuthClient(env)
  const session = await verifyAiurSession(
    access,
    cookies.get(AIUR_REFRESH_COOKIE),
    client,
  )

  if (session === undefined) {
    return { kind: 'signed_out' }
  }

  const allowlist = parseOwnerAllowlist(env.AIUR_OWNER_USER_IDS)

  if (!isAllowedOwnerUserId(session.user.userId, allowlist)) {
    return { kind: 'denied', user: session.user }
  }

  return { kind: 'owner', session }
}
