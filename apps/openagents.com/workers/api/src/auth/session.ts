import { type Tokens } from '@openauthjs/openauth/client'

import { appendSessionCookies } from '../auth-cookies'

export type VerifiedSession<User> = Readonly<{
  user: User
  tokens?: Tokens
}>

export type BrowserSessionBoundary<User, Bindings> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: VerifiedSession<User>,
  ) => Response
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
}>

export const makeBrowserSessionBoundary = <User, Bindings>(
  dependencies: Readonly<{
    persistUser: (env: Bindings, user: User) => Promise<void>
    verifySession: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ) => Promise<VerifiedSession<User> | undefined>
  }>,
): BrowserSessionBoundary<User, Bindings> => {
  const requireBrowserSession = async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<VerifiedSession<User> | undefined> => {
    const session = await dependencies.verifySession(request, env, ctx)

    if (session === undefined) {
      return undefined
    }

    await dependencies.persistUser(env, session.user)

    return session
  }

  const appendRefreshedSessionCookies = (
    response: Response,
    session: VerifiedSession<User>,
  ): Response => {
    if (session.tokens !== undefined) {
      appendSessionCookies(response.headers, session.tokens)
    }

    return response
  }

  return {
    appendRefreshedSessionCookies,
    requireBrowserSession,
  }
}
