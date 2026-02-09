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
import { marketingShellTemplate } from "../effuse-pages/marketingShell"
import { deckPageShellTemplate } from "../effuse-pages/deck"
import { storybookCanvasTemplate, storybookManagerTemplate } from "../effuse-pages/storybook"

import { AppConfigService } from "../effect/config"
import { AuthService } from "../effect/auth"
import { SessionAtom } from "../effect/atoms/session"
import { getStoryById, listStoryMeta } from "../storybook"

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

const matchStorybook = (url: URL): RouteMatch | null => {
  if (url.pathname === "/__storybook") {
    return { pathname: url.pathname, params: { view: "index" }, search: url.searchParams }
  }

  const prefix = "/__storybook/canvas/"
  if (url.pathname.startsWith(prefix)) {
    const raw = url.pathname.slice(prefix.length)
    if (!raw || raw.includes("/")) return null
    try {
      const storyId = decodeURIComponent(raw)
      return { pathname: url.pathname, params: { view: "canvas", storyId }, search: url.searchParams }
    } catch {
      return null
    }
  }

  return null
}

const isLocalHost = (host: string): boolean =>
  host === "localhost" || host === "127.0.0.1" || host === "::1"

/** When prelaunch is on, redirect to home unless bypass (query key or cookie). */
const prelaunchRedirectGuard = (
  ctx: RouteContext,
): Effect.Effect<RouteOutcome<never> | undefined, never, AppServices> =>
  Effect.gen(function* () {
    const config = yield* AppConfigService
    if (!config.prelaunch) return undefined
    const bypass = yield* (ctx._tag === "Server"
      ? Effect.sync(() => {
          const key = config.prelaunchBypassKey
          if (!key) return false
          const cookie = ctx.request.headers.get("Cookie") ?? ""
          if (cookie.includes("prelaunch_bypass=1")) return true
          return ctx.url.searchParams.get("key") === key
        })
      : Effect.sync(
          () =>
            typeof document !== "undefined" &&
            document.cookie.includes("prelaunch_bypass=1"),
        ))
    if (bypass) return undefined
    return RouteOutcome.redirect("/", 302)
  })

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

type HomeData = { readonly year: number; readonly prelaunch: boolean }
const home: Route<HomeData, AppServices> = {
  id: "/",
  match: matchExact("/"),
  loader: (ctx) =>
    Effect.gen(function* () {
      const config = yield* AppConfigService
      return yield* okWithSession(ctx, {
        year: new Date().getFullYear(),
        prelaunch: config.prelaunch,
      })
    }),
  view: (_ctx, data) =>
    Effect.succeed(
      marketingShellTemplate({
        isHome: true,
        isLogin: false,
        prelaunch: data.prelaunch,
        content: homePageTemplate(data.year, data.prelaunch),
      }),
    ),
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
  guard: (ctx) =>
    Effect.gen(function* () {
      const redirect = yield* prelaunchRedirectGuard(ctx)
      if (redirect) return redirect
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (session.userId) return RouteOutcome.redirect("/autopilot", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, defaultLoginModel),
  view: (_ctx, model) =>
    Effect.succeed(
      marketingShellTemplate({
        isHome: false,
        isLogin: true,
        content: loginPageTemplate(model),
      }),
    ),
  head: () => Effect.succeed({ title: "Log in" }),
}

const autopilot: Route<{}, AppServices> = {
  id: "/autopilot",
  match: matchExact("/autopilot"),
  guard: (ctx) => prelaunchRedirectGuard(ctx),
  loader: (ctx) => okWithSession(ctx, {}),
  view: () => Effect.succeed(autopilotRouteShellTemplate()),
  head: () => Effect.succeed({ title: "Autopilot" }),
}

const deck: Route<{}, AppServices> = {
  id: "/deck",
  match: matchExact("/deck"),
  guard: (ctx) =>
    Effect.gen(function* () {
      const redirect = yield* prelaunchRedirectGuard(ctx)
      if (redirect) return redirect
      if (!isLocalHost(ctx.url.hostname)) return RouteOutcome.notFound()
      return undefined
    }),
  loader: () => Effect.succeed(RouteOutcome.ok({})),
  view: () => Effect.succeed(deckPageShellTemplate()),
  head: () => Effect.succeed({ title: "Deck" }),
}

const modules: Route<ModulesPageData, AppServices> = {
  id: "/modules",
  match: matchExact("/modules"),
  guard: (ctx) =>
    Effect.gen(function* () {
      const redirect = yield* prelaunchRedirectGuard(ctx)
      if (redirect) return redirect
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
  guard: (ctx) =>
    Effect.gen(function* () {
      const redirect = yield* prelaunchRedirectGuard(ctx)
      if (redirect) return redirect
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
  guard: (ctx) =>
    Effect.gen(function* () {
      const redirect = yield* prelaunchRedirectGuard(ctx)
      if (redirect) return redirect
      const auth = yield* AuthService
      const session = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)))
      if (!session.userId) return RouteOutcome.redirect("/", 302)
      return
    }),
  loader: (ctx) => okWithSession(ctx, { errorText: null, sorted: null }),
  view: (_ctx, data) => Effect.succeed(authedShellTemplate(toolsPageTemplate(data))),
  head: () => Effect.succeed({ title: "Tools" }),
}

type StorybookData =
  | {
      readonly mode: "index"
      readonly stories: ReturnType<typeof listStoryMeta>
      readonly defaultStoryId: string | null
    }
  | { readonly mode: "canvas"; readonly storyId: string }

const storybook: Route<StorybookData, AppServices> = {
  id: "/__storybook",
  match: matchStorybook,
  guard: (ctx) => prelaunchRedirectGuard(ctx),
  loader: (ctx) =>
    Effect.sync(() => {
      const view = ctx.match.params.view
      const stories = listStoryMeta()

      if (view === "canvas") {
        const storyId = ctx.match.params.storyId ?? ""
        const story = storyId ? getStoryById(storyId) : null
        if (!story) return RouteOutcome.notFound()
        return RouteOutcome.ok({ mode: "canvas", storyId })
      }

      const defaultStoryId = stories.length > 0 ? stories[0]!.id : null
      return RouteOutcome.ok({ mode: "index", stories, defaultStoryId })
    }),
  view: (_ctx, data) =>
    Effect.succeed(
      data.mode === "canvas"
        ? storybookCanvasTemplate(getStoryById(data.storyId)!)
        : storybookManagerTemplate({ stories: data.stories, defaultStoryId: data.defaultStoryId }),
    ),
  head: (_ctx, data) =>
    Effect.succeed(
      data.mode === "canvas"
        ? { title: getStoryById(data.storyId)?.title ?? "Story" }
        : { title: "Storybook" },
    ),
}

// Legacy route: keep /chat/:id redirecting to /autopilot.
const chatLegacyRedirect: Route<{}, AppServices> = {
  id: "/chat/$chatId",
  match: matchChatLegacy,
  guard: (ctx) => prelaunchRedirectGuard(ctx),
  loader: () => Effect.succeed(RouteOutcome.redirect("/autopilot", 302)),
  view: () => Effect.succeed(homePageTemplate()),
}

export const appRoutes = [
  home,
  login,
  autopilot,
  deck,
  storybook,
  modules,
  signatures,
  tools,
  chatLegacyRedirect,
] as const satisfies ReadonlyArray<Route<any, AppServices>>
