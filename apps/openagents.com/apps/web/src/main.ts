import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command, Runtime } from 'foldkit'
import { replaceUrl } from 'foldkit/navigation'
import { Url } from 'foldkit/url'

import { AuthBootstrap, AuthSessionResponse } from './domain/session'
import {
  CompletedNavigateInternal,
  GotLoggedInMessage,
  GotLoggedOutMessage,
  Message,
} from './message'
import { Demo, LoggedIn, LoggedOut, Model } from './model'
import { type LoggedInRoute, urlToAppRoute } from './route'
import {
  type StartupRedirect,
  routeRequiresAuthBootstrap,
  startupRouteForLoggedIn,
  startupRouteForLoggedOut,
} from './routing/startup'

// FLAGS

export const Flags = S.Struct({
  maybeAuth: S.Option(AuthBootstrap),
})

class AuthBootstrapFetchError extends S.TaggedErrorClass<AuthBootstrapFetchError>()(
  'AuthBootstrapFetchError',
  { cause: S.Defect },
) {}

const fetchAuthBootstrap = Effect.gen(function* () {
  const response = yield* Effect.tryPromise({
    try: () =>
      fetch('/api/auth/session', {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      }),
    catch: error => new AuthBootstrapFetchError({ cause: error }),
  })

  if (!response.ok) {
    return Option.none<AuthBootstrap>()
  }

  const payload = yield* Effect.tryPromise({
    try: () => response.json(),
    catch: error => new AuthBootstrapFetchError({ cause: error }),
  })
  const decoded = yield* S.decodeUnknownEffect(AuthSessionResponse)(payload)

  return decoded.authenticated
    ? Option.some(decoded.bootstrap)
    : Option.none<AuthBootstrap>()
})

const shouldFetchAuthBootstrap = (): boolean => {
  if (typeof window === 'undefined') {
    return true
  }

  return routeRequiresAuthBootstrap(
    urlToAppRoute({
      protocol: window.location.protocol.replace(':', ''),
      host: window.location.hostname,
      port:
        window.location.port === ''
          ? Option.none()
          : Option.some(window.location.port),
      pathname: window.location.pathname,
      search:
        window.location.search === ''
          ? Option.none()
          : Option.some(window.location.search),
      hash:
        window.location.hash === ''
          ? Option.none()
          : Option.some(window.location.hash),
    }),
  )
}

export const flags: Effect.Effect<Flags> = Effect.gen(function* () {
  if (!shouldFetchAuthBootstrap()) {
    return Flags.make({ maybeAuth: Option.none() })
  }

  const maybeAuth = yield* fetchAuthBootstrap

  return Flags.make({ maybeAuth })
}).pipe(
  Effect.catch(() => Effect.succeed(Flags.make({ maybeAuth: Option.none() }))),
)

export type Flags = typeof Flags.Type

// COMMAND

const RedirectToHome = Command.define(
  'RedirectToHome',
  { href: S.String },
  CompletedNavigateInternal,
)(({ href }) => replaceUrl(href).pipe(Effect.as(CompletedNavigateInternal())))

const RedirectToDefaultLoggedInRoute = Command.define(
  'RedirectToDefaultLoggedInRoute',
  { href: S.String },
  CompletedNavigateInternal,
)(({ href }) => replaceUrl(href).pipe(Effect.as(CompletedNavigateInternal())))

const RedirectToOnboarding = Command.define(
  'RedirectToOnboarding',
  { href: S.String },
  CompletedNavigateInternal,
)(({ href }) => replaceUrl(href).pipe(Effect.as(CompletedNavigateInternal())))

const RedirectToInvite = Command.define(
  'RedirectToInvite',
  { href: S.String },
  CompletedNavigateInternal,
)(({ href }) => replaceUrl(href).pipe(Effect.as(CompletedNavigateInternal())))

const RedirectToOrder = Command.define(
  'RedirectToOrder',
  { href: S.String },
  CompletedNavigateInternal,
)(({ href }) => replaceUrl(href).pipe(Effect.as(CompletedNavigateInternal())))

const redirectCommand = (redirect: StartupRedirect): Command.Command<Message> =>
  M.value(redirect).pipe(
    M.tagsExhaustive({
      StartupRedirectToHome: ({ href }) => RedirectToHome({ href }),
      StartupRedirectToDefaultLoggedInRoute: ({ href }) =>
        RedirectToDefaultLoggedInRoute({ href }),
      StartupRedirectToOnboarding: ({ href }) => RedirectToOnboarding({ href }),
      StartupRedirectToInvite: ({ href }) => RedirectToInvite({ href }),
      StartupRedirectToOrder: ({ href }) => RedirectToOrder({ href }),
    }),
  )

const redirectCommands = (
  redirect: Option.Option<StartupRedirect>,
): ReadonlyArray<Command.Command<Message>> =>
  Option.match(redirect, {
    onNone: () => [],
    onSome: redirect => [redirectCommand(redirect)],
  })

const initLoggedInRoute = (
  route: LoggedInRoute,
  auth: AuthBootstrap,
): [LoggedIn.Model, ReadonlyArray<Command.Command<Message>>] => {
  const model = LoggedIn.init(route, auth)
  const initialCommands = Command.mapMessages(
    LoggedIn.initialCommands(model),
    message => GotLoggedInMessage({ message }),
  )

  if (route._tag !== 'Thread') {
    return [model, initialCommands]
  }

  const [nextModel, commands] = LoggedIn.update(
    model,
    LoggedIn.EnteredAutopilotRunRoute({ runId: route.threadId }),
  )

  return [
    nextModel,
    [
      ...initialCommands,
      ...Command.mapMessages(commands, message =>
        GotLoggedInMessage({ message }),
      ),
    ],
  ]
}

const loggedOutInitialCommands = (
  model: LoggedOut.Model,
): ReadonlyArray<Command.Command<Message>> =>
  Command.mapMessages(LoggedOut.initialCommands(model), message =>
    GotLoggedOutMessage({ message }),
  )

// INIT

type InitReturn = [Model, ReadonlyArray<Command.Command<Message>>]
const withInitReturn = M.withReturnType<InitReturn>()

export const init: Runtime.RoutingProgramInit<Model, Message, Flags> = (
  flags: Flags,
  url: Url,
): InitReturn => {
  const route = urlToAppRoute(url)

  if (Demo.isDemoAppRoute(route)) {
    return [Demo.init(route), []]
  }

  return Option.match(flags.maybeAuth, {
    onNone: () => {
      const resolution = startupRouteForLoggedOut(route)
      const model = LoggedOut.init(resolution.route)

      return [
        model,
        [
          ...loggedOutInitialCommands(model),
          ...redirectCommands(resolution.redirect),
        ],
      ]
    },

    onSome: auth =>
      M.value(startupRouteForLoggedIn(route, auth)).pipe(
        withInitReturn,
        M.tagsExhaustive({
          LoggedOutStartupRoute: resolution => {
            const model = LoggedOut.init(
              resolution.route,
              Option.some(auth.session),
            )

            return [
              model,
              [
                ...loggedOutInitialCommands(model),
                ...redirectCommands(resolution.redirect),
              ],
            ]
          },
          LoggedInStartupRoute: resolution => {
            const [model, commands] = initLoggedInRoute(resolution.route, auth)
            return [
              model,
              [...commands, ...redirectCommands(resolution.redirect)],
            ]
          },
        }),
      ),
  })
}
