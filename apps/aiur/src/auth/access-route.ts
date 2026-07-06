import { resolveAiurAccess, type ResolveAiurAccessDeps } from './access'
import type { AiurEnv } from './config'

export const AIUR_ACCESS_PATH = '/api/aiur/access'

type AiurAccessResponseBody =
  | Readonly<{ kind: 'signed_out' }>
  | Readonly<{
      kind: 'denied'
      user: Readonly<{ login: string | undefined; name: string; avatarUrl: string }>
    }>
  | Readonly<{
      kind: 'owner'
      user: Readonly<{ login: string | undefined; name: string; avatarUrl: string }>
    }>

const noStoreJson = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

/**
 * The one JSON endpoint the dashboard React shell reads to decide what to
 * render (sign-in / access-denied / dashboard). This is UX-only — every
 * route that actually touches data (the Khala Sync proxy, the future
 * credits/ops routes) re-checks `resolveAiurAccess` itself and fails
 * closed independently, so a client bug here can never leak real data.
 */
export const handleAiurAccessRequest = async (
  request: Request,
  env: AiurEnv,
  deps: ResolveAiurAccessDeps = {},
): Promise<Response> => {
  const access = await resolveAiurAccess(request, env, deps)

  const body: AiurAccessResponseBody =
    access.kind === 'signed_out'
      ? { kind: 'signed_out' }
      : {
          kind: access.kind,
          user:
            access.kind === 'denied'
              ? {
                  login: access.user.login,
                  name: access.user.name,
                  avatarUrl: access.user.avatarUrl,
                }
              : {
                  login: access.session.user.login,
                  name: access.session.user.name,
                  avatarUrl: access.session.user.avatarUrl,
                },
        }

  return noStoreJson(body)
}
