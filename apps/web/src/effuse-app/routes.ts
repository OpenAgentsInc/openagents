import { Hydration, Registry } from "@effect-atom/atom"
import { Effect } from "effect"
import { RouteOutcome } from "@openagentsinc/effuse"

import { autopilotRouteShellTemplate } from "../effuse-pages/autopilotRoute"
import { authedShellTemplate } from "../effuse-pages/authedShell"
import { homePageTemplate } from "../effuse-pages/home"
import { loginPageTemplate } from "../effuse-pages/login"
import { modulesPageTemplate } from "../effuse-pages/modules"
import { signaturesPageTemplate } from "../effuse-pages/signatures"
import { toolsPageTemplate } from "../effuse-pages/tools"

import { AuthService } from "../effect/auth"
import { SessionAtom } from "../effect/atoms/session"

import type { Route, RouteMatch } from "@openagentsinc/effuse"
import type { LoginPageModel } from "../effuse-pages/login"
import type { ModulesPageData } from "../effuse-pages/modules"
import type { SignaturesPageData } from "../effuse-pages/signatures"
import type { ToolsPageData } from "../effuse-pages/tools"
import type { RouteContext, RouteOkHints } from "@openagentsinc/effuse"
import type { AppServices } from "../effect/layer"

const matchExact =
  (pathname: string) =>
  (url: URL): RouteMatch | null => {
    if (url.pathname !== pathname) return null
    return { pathname, params: {}, search: url.searchParams }
  }

const matchChatLegacy = (url: URL): RouteMatch | null => {
  if (!url.pathname.startsWith("/chat/")) return null
  const rest = url.pathname.slice("/chat/".length)
  if (!rest || rest.includes("/")) return null
  return { pathname: url.pathname, params: { chatId: rest }, search: url.searchParams }
}

const sessionDehydrate = (ctx: RouteContext): Effect.Effect<RouteOkHints["dehydrate"] | undefined, never, AppServices> =>
  Effect.gen(function* () {
    if (ctx._tag !== "Server") return undefined
    const auth = yield* AuthService
    const session = yield* auth.getSession().pipe(
      Effect.catchAll(() =>
        Effect.succeed({
          userId: null,
          sessionId: null,
          user: null,
        })
      )
    )

    const user =
      session.user && typeof session.user === "object"
        ? {
            id: session.user.id,
            email: session.user.email,
            firstName: session.user.firstName,
            lastName: session.user.lastName,
          }
        : null

    const atomRegistry = Registry.make()
    atomRegistry.set(SessionAtom, { userId: session.userId, user })
    const atomState = Hydration.dehydrate(atomRegistry)
    atomRegistry.dispose()

    return { atomState }
  })

const okWithSession = <A>(
  ctx: RouteContext,
  data: A,
): Effect.Effect<ReturnType<typeof RouteOutcome.ok<A>>, never, AppServices> =>
  Effect.gen(function* () {
    const dehydrate = yield* sessionDehydrate(ctx)
    return RouteOutcome.ok(data, dehydrate ? { dehydrate } : undefined)
  })

type HomeData = { readonly year: number }
const home: Route<HomeData, AppServices> = {
  id: "/",
  match: matchExact("/"),
  loader: (ctx) => okWithSession(ctx, { year: new Date().getFullYear() }),
  view: (_ctx, data) => Effect.succeed(homePageTemplate(data.year)),
  head: () => Effect.succeed({ title: "OpenAgents" }),
}

const defaultLoginModel: LoginPageModel = {
  step: "email",
  email: "",
  code: "",
  isBusy: false,
  errorText: null,
}

const login: Route<LoginPageModel, AppServices> = {
  id: "/login",
  match: matchExact("/login"),
  guard: (_ctx) =>
    Effect.gen(function* () {
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (session.userId) return RouteOutcome.redirect("/autopilot", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, defaultLoginModel),
  view: (_ctx, model) => Effect.succeed(loginPageTemplate(model)),
  head: () => Effect.succeed({ title: "Log in" }),
}

const autopilot: Route<{}, AppServices> = {
  id: "/autopilot",
  match: matchExact("/autopilot"),
  loader: (ctx) => okWithSession(ctx, {}),
  view: () => Effect.succeed(autopilotRouteShellTemplate()),
  head: () => Effect.succeed({ title: "Autopilot" }),
}

const modules: Route<ModulesPageData, AppServices> = {
  id: "/modules",
  match: matchExact("/modules"),
  guard: (_ctx) =>
    Effect.gen(function* () {
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (!session.userId) return RouteOutcome.redirect("/", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, { errorText: null, sorted: null }),
  view: (_ctx, data) => Effect.succeed(authedShellTemplate(modulesPageTemplate(data))),
  head: () => Effect.succeed({ title: "Modules" }),
}

const signatures: Route<SignaturesPageData, AppServices> = {
  id: "/signatures",
  match: matchExact("/signatures"),
  guard: (_ctx) =>
    Effect.gen(function* () {
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (!session.userId) return RouteOutcome.redirect("/", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, { errorText: null, sorted: null }),
  view: (_ctx, data) => Effect.succeed(authedShellTemplate(signaturesPageTemplate(data))),
  head: () => Effect.succeed({ title: "Signatures" }),
}

const tools: Route<ToolsPageData, AppServices> = {
  id: "/tools",
  match: matchExact("/tools"),
  guard: (_ctx) =>
    Effect.gen(function* () {
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (!session.userId) return RouteOutcome.redirect("/", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, { errorText: null, sorted: null }),
  view: (_ctx, data) => Effect.succeed(authedShellTemplate(toolsPageTemplate(data))),
  head: () => Effect.succeed({ title: "Tools" }),
}

// Legacy route: keep /chat/:id redirecting to /autopilot.
const chatLegacyRedirect: Route<{}, AppServices> = {
  id: "/chat/$chatId",
  match: matchChatLegacy,
  loader: () => Effect.succeed(RouteOutcome.redirect("/autopilot", 302)),
  view: () => Effect.succeed(homePageTemplate()),
}

export const appRoutes = [
  home,
  login,
  autopilot,
  modules,
  signatures,
  tools,
  chatLegacyRedirect,
] as const satisfies ReadonlyArray<Route<any, AppServices>>
