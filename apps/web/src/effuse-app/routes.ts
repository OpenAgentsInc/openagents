import { Effect } from "effect"
import { RouteOutcome } from "@openagentsinc/effuse"

import { autopilotRouteShellTemplate } from "../effuse-pages/autopilotRoute"
import { homePageTemplate } from "../effuse-pages/home"
import { loginPageTemplate } from "../effuse-pages/login"
import { modulesPageTemplate } from "../effuse-pages/modules"
import { signaturesPageTemplate } from "../effuse-pages/signatures"
import { toolsPageTemplate } from "../effuse-pages/tools"

import type { Route, RouteMatch } from "@openagentsinc/effuse"
import type { LoginPageModel } from "../effuse-pages/login"
import type { ModulesPageData } from "../effuse-pages/modules"
import type { SignaturesPageData } from "../effuse-pages/signatures"
import type { ToolsPageData } from "../effuse-pages/tools"

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

type HomeData = { readonly year: number }
const home: Route<HomeData> = {
  id: "/",
  match: matchExact("/"),
  loader: () => Effect.succeed(RouteOutcome.ok({ year: new Date().getFullYear() })),
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

const login: Route<LoginPageModel> = {
  id: "/login",
  match: matchExact("/login"),
  loader: () => Effect.succeed(RouteOutcome.ok(defaultLoginModel)),
  view: (_ctx, model) => Effect.succeed(loginPageTemplate(model)),
  head: () => Effect.succeed({ title: "Log in" }),
}

const autopilot: Route<{}> = {
  id: "/autopilot",
  match: matchExact("/autopilot"),
  loader: () => Effect.succeed(RouteOutcome.ok({})),
  view: () => Effect.succeed(autopilotRouteShellTemplate()),
  head: () => Effect.succeed({ title: "Autopilot" }),
}

const modules: Route<ModulesPageData> = {
  id: "/modules",
  match: matchExact("/modules"),
  loader: () =>
    Effect.succeed(RouteOutcome.ok({ errorText: null, sorted: [] })),
  view: (_ctx, data) => Effect.succeed(modulesPageTemplate(data)),
  head: () => Effect.succeed({ title: "Modules" }),
}

const signatures: Route<SignaturesPageData> = {
  id: "/signatures",
  match: matchExact("/signatures"),
  loader: () =>
    Effect.succeed(RouteOutcome.ok({ errorText: null, sorted: [] })),
  view: (_ctx, data) => Effect.succeed(signaturesPageTemplate(data)),
  head: () => Effect.succeed({ title: "Signatures" }),
}

const tools: Route<ToolsPageData> = {
  id: "/tools",
  match: matchExact("/tools"),
  loader: () =>
    Effect.succeed(RouteOutcome.ok({ errorText: null, sorted: [] })),
  view: (_ctx, data) => Effect.succeed(toolsPageTemplate(data)),
  head: () => Effect.succeed({ title: "Tools" }),
}

// Legacy route: keep /chat/:id redirecting to /autopilot.
const chatLegacyRedirect: Route<{}> = {
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
] as const satisfies ReadonlyArray<Route<any>>
