import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { load, pushUrl, replaceUrl } from 'foldkit/navigation'
import { evo } from 'foldkit/struct'
import { toString as urlToString } from 'foldkit/url'

import { ClearSession, LogError } from './command'
import {
  CompletedLoadExternal,
  CompletedNavigateInternal,
  GotLoggedInMessage,
  GotLoggedOutMessage,
  Message,
} from './message'
import { Demo, LoggedIn, LoggedOut, Model } from './model'
import { ThreadRouteIdle } from './page/loggedIn/thread-route'
import {
  OverviewTab as WorkroomOverviewTab,
  init as initWorkroom,
  tabFromRef as workroomTabFromRef,
} from './page/loggedIn/page/workroom'
import {
  defaultLoggedInHrefForAuth,
  loggedInWorkroomAllowed,
} from './product-policy'
import {
  HomeRoute,
  type LoggedInRoute,
  chatRouter,
  urlToAppRoute,
} from './route'
import {
  type StartupRedirect,
  startupRouteForLoggedIn,
  startupRouteForLoggedOut,
} from './routing/startup'

const NavigateInternal = Command.define(
  'NavigateInternal',
  { url: S.String },
  CompletedNavigateInternal,
)(({ url }) => pushUrl(url).pipe(Effect.as(CompletedNavigateInternal())))

const LoadExternal = Command.define(
  'LoadExternal',
  { href: S.String },
  CompletedLoadExternal,
)(({ href }) => load(href).pipe(Effect.as(CompletedLoadExternal())))

export const RedirectToChat = Command.define(
  'RedirectToChat',
  CompletedNavigateInternal,
)(replaceUrl(chatRouter()).pipe(Effect.as(CompletedNavigateInternal())))

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

const RedirectToHome = Command.define(
  'RedirectToHome',
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

const publicDocumentPaths = new Set([
  '/AGENTS-CORE.md',
  '/AGENTS.md',
  '/HEARTBEAT.md',
  '/RULES.md',
  '/skill.json',
])

const shouldLoadDocument = (pathname: string): boolean =>
  pathname.startsWith('/auth/') ||
  pathname.startsWith('/api/') ||
  pathname.startsWith('/.well-known/') ||
  pathname === '/login/github' ||
  publicDocumentPaths.has(pathname) ||
  pathname === '/docs' ||
  pathname.startsWith('/docs/') ||
  pathname === '/blog' ||
  pathname.startsWith('/blog/') ||
  pathname === '/forum' ||
  pathname.startsWith('/forum/') ||
  pathname === '/training/runs' ||
  pathname.startsWith('/training/runs/')

type UpdateReturn = readonly [Model, ReadonlyArray<Command.Command<Message>>]
const withUpdateReturn = M.withReturnType<UpdateReturn>()

const updateLoggedInRoute = (
  loggedInModel: LoggedIn.Model,
  route: LoggedInRoute,
): UpdateReturn => {
  const routedModel = evo(loggedInModel, { route: () => route })

  if (route._tag === 'Workroom' || route._tag === 'WorkroomTab') {
    const activeTab =
      route._tag === 'WorkroomTab'
        ? workroomTabFromRef(route.tab)
        : WorkroomOverviewTab
    const enteredModel = evo(routedModel, {
      threadRoute: () => ThreadRouteIdle(),
      workroom: () => initWorkroom(route.workroomId, activeTab),
    })

    return [
      enteredModel,
      Command.mapMessages(LoggedIn.initialCommands(enteredModel), message =>
        GotLoggedInMessage({ message }),
      ),
    ]
  }

  if (
    route._tag === 'Onboarding' ||
    route._tag === 'Order' ||
    route._tag === 'OrderDetail' ||
    route._tag === 'AutopilotWork' ||
    route._tag === 'AutopilotWorkDetail'
  ) {
    return [
      evo(routedModel, { threadRoute: () => ThreadRouteIdle() }),
      Command.mapMessages(LoggedIn.initialCommands(routedModel), message =>
        GotLoggedInMessage({ message }),
      ),
    ]
  }

  if (route._tag !== 'Thread') {
    return [evo(routedModel, { threadRoute: () => ThreadRouteIdle() }), []]
  }

  const [nextModel, commands] = LoggedIn.update(
    routedModel,
    LoggedIn.EnteredAutopilotRunRoute({ runId: route.threadId }),
  )

  return [
    nextModel,
    Command.mapMessages(commands, message => GotLoggedInMessage({ message })),
  ]
}

const routeLoadCommands = (
  model: LoggedIn.Model,
): ReadonlyArray<Command.Command<Message>> =>
  Command.mapMessages(LoggedIn.initialCommands(model), message =>
    GotLoggedInMessage({ message }),
  )

const loggedInInitialCommands = (
  model: LoggedIn.Model,
): ReadonlyArray<Command.Command<Message>> =>
  Command.mapMessages(LoggedIn.initialCommands(model), message =>
    GotLoggedInMessage({ message }),
  )

const loggedOutInitialCommands = (
  model: LoggedOut.Model,
): ReadonlyArray<Command.Command<Message>> =>
  Command.mapMessages(LoggedOut.initialCommands(model), message =>
    GotLoggedOutMessage({ message }),
  )

export const update = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      ClickedLink: ({ request }) =>
        M.value(request).pipe(
          withUpdateReturn,
          M.tagsExhaustive({
            Internal: ({ url }) => {
              const href = urlToString(url)

              if (shouldLoadDocument(url.pathname)) {
                return [model, [LoadExternal({ href })]]
              }

              return [model, [NavigateInternal({ url: href })]]
            },
            External: ({ href }) => [model, [LoadExternal({ href })]],
          }),
        ),

      ChangedUrl: ({ url }) => {
        const route = urlToAppRoute(url)

        if (Demo.isDemoAppRoute(route)) {
          return M.value(model).pipe(
            withUpdateReturn,
            M.tagsExhaustive({
              Demo: demoModel => {
                if (Demo.demoModeForRoute(route) !== demoModel.mode) {
                  return [Demo.init(route), []]
                }

                if (demoModel.mode === 'training') {
                  return [demoModel, []]
                }

                return [
                  evo(demoModel, {
                    loggedIn: loggedIn =>
                      evo(loggedIn, {
                        route: () => Demo.loggedInRouteForDemoRoute(route),
                      }),
                  }),
                  [],
                ]
              },
              LoggedIn: () => [Demo.init(route), []],
              LoggedOut: () => [Demo.init(route), []],
            }),
          )
        }

        return M.value(model).pipe(
          withUpdateReturn,
          M.tagsExhaustive({
            LoggedOut: loggedOutModel => {
              const resolution = startupRouteForLoggedOut(route)
              const nextModel = evo(loggedOutModel, {
                route: () => resolution.route,
              })

              return [
                nextModel,
                [
                  ...loggedOutInitialCommands(nextModel),
                  ...redirectCommands(resolution.redirect),
                ],
              ]
            },

            LoggedIn: loggedInModel =>
              M.value(startupRouteForLoggedIn(route, loggedInModel.auth)).pipe(
                withUpdateReturn,
                M.tagsExhaustive({
                  LoggedOutStartupRoute: resolution => {
                    const nextModel = LoggedOut.init(resolution.route)

                    return [
                      nextModel,
                      [
                        ...loggedOutInitialCommands(nextModel),
                        ...redirectCommands(resolution.redirect),
                      ],
                    ]
                  },
                  LoggedInStartupRoute: resolution => {
                    if (
                      resolution.route._tag !== 'TeamProjectChat' &&
                      resolution.route._tag !== 'TeamFiles' &&
                      resolution.route._tag !== 'TeamFile' &&
                      resolution.route._tag !== 'PersonalFile'
                    ) {
                      const [nextModel, commands] = updateLoggedInRoute(
                        loggedInModel,
                        resolution.route,
                      )

                      return [
                        nextModel,
                        [...commands, ...redirectCommands(resolution.redirect)],
                      ]
                    }

                    const routedModel = evo(loggedInModel, {
                      route: () => resolution.route,
                      threadRoute: () => ThreadRouteIdle(),
                    })

                    return [
                      routedModel,
                      [
                        ...routeLoadCommands(routedModel),
                        ...redirectCommands(resolution.redirect),
                      ],
                    ]
                  },
                }),
              ),
            Demo: () => {
              const resolution = startupRouteForLoggedOut(route)
              const nextModel = LoggedOut.init(resolution.route)

              return [
                nextModel,
                [
                  ...loggedOutInitialCommands(nextModel),
                  ...redirectCommands(resolution.redirect),
                ],
              ]
            },
          }),
        )
      },

      LoadedSession: ({ session }) =>
        M.value(session).pipe(
          withUpdateReturn,
          M.tagsExhaustive({
            Some: ({ value }) => {
              return M.value(startupRouteForLoggedIn(HomeRoute(), value)).pipe(
                withUpdateReturn,
                M.tagsExhaustive({
                  LoggedOutStartupRoute: resolution => {
                    const nextModel = LoggedOut.init(resolution.route)

                    return [
                      nextModel,
                      [
                        ...loggedOutInitialCommands(nextModel),
                        ...redirectCommands(resolution.redirect),
                      ],
                    ]
                  },
                  LoggedInStartupRoute: resolution => {
                    const loggedIn = LoggedIn.init(resolution.route, value)

                    return [
                      loggedIn,
                      [
                        ...loggedInInitialCommands(loggedIn),
                        ...redirectCommands(resolution.redirect),
                      ],
                    ]
                  },
                }),
              )
            },
            None: () => [model, []],
          }),
        ),

      FailedClearSession: ({ error }) => [
        model,
        [LogError({ entries: ['Failed to clear session:', error] })],
      ],

      RequestedLoggedOutLogout: () => [
        model,
        [ClearSession(), LoadExternal({ href: '/auth/logout' })],
      ],

      GotLoggedOutMessage: ({ message }) =>
        handleGotLoggedOutMessage(model, message),

      GotLoggedInMessage: ({ message }) =>
        handleGotLoggedInMessage(model, message),
      GotDemoMessage: ({ message }) => handleGotDemoMessage(model, message),
    }),
    M.tag(
      'CompletedNavigateInternal',
      'CompletedLoadExternal',
      'CompletedLogError',
      'SucceededClearSession',
      () => [model, []],
    ),
    M.exhaustive,
  )

const handleGotLoggedOutMessage = (
  model: Model,
  message: LoggedOut.Message,
): UpdateReturn => {
  if (model._tag !== 'LoggedOut') {
    return [model, []]
  }

  const [nextModel, commands] = LoggedOut.update(model, message)

  const mappedCommands = Command.mapMessages(commands, message =>
    GotLoggedOutMessage({ message }),
  )

  return [nextModel, mappedCommands]
}

const handleGotDemoMessage = (
  model: Model,
  message: Demo.Message,
): UpdateReturn => {
  if (model._tag !== 'Demo') {
    return [model, []]
  }

  const [nextModel] = Demo.update(model, message)

  return [nextModel, []]
}

const handleGotLoggedInMessage = (
  model: Model,
  message: LoggedIn.Message,
): UpdateReturn => {
  if (model._tag !== 'LoggedIn') {
    return [model, []]
  }

  const [nextModel, commands, maybeOutMessage] = LoggedIn.update(model, message)

  const mappedCommands = Command.mapMessages(commands, message =>
    GotLoggedInMessage({ message }),
  )
  const commandsWithNavigation =
    message._tag === 'ClickedNewChat' &&
    loggedInWorkroomAllowed(nextModel.auth) &&
    nextModel.route._tag === 'Chat'
      ? [...mappedCommands, RedirectToChat()]
      : mappedCommands

  return Option.match(maybeOutMessage, {
    onNone: () => [nextModel, commandsWithNavigation],
    onSome: outMessage =>
      M.value(outMessage).pipe(
        withUpdateReturn,
        M.tagsExhaustive({
          CompletedOnboarding: () => [
            nextModel,
            [
              ...commandsWithNavigation,
              RedirectToDefaultLoggedInRoute({
                href: defaultLoggedInHrefForAuth(nextModel.auth),
              }),
            ],
          ],
          RequestedLogout: () => [
            model,
            [
              ...commandsWithNavigation,
              ClearSession(),
              LoadExternal({ href: '/auth/logout' }),
            ],
          ],
        }),
      ),
  })
}
