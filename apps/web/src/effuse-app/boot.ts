import { Effect, Stream } from "effect"
import {
  BrowserHistory,
  EffuseLive,
  html,
  makeEzRegistry,
  makeRouter,
  mountEzRuntimeWith,
} from "@openagentsinc/effuse"

import { AgentApiService } from "../effect/agentApi"
import { AuthService } from "../effect/auth"
import { ChatService } from "../effect/chat"
import { getAppConfig } from "../effect/config"
import { SessionAtom } from "../effect/atoms/session"
import { makeAppRuntime } from "../effect/runtime"
import { TelemetryService } from "../effect/telemetry"
import { hydrateAtomRegistryFromDocument, makeAtomRegistry } from "./atomRegistry"
import { mountAutopilotController } from "./controllers/autopilotController"
import { mountModulesController, mountSignaturesController, mountToolsController } from "./controllers/contractsController"
import { mountHomeController } from "./controllers/homeController"
import { mountLoginController } from "./controllers/loginController"
import { loadPostHog } from "./posthog"
import { appRoutes } from "./routes"
import { UiBlobStore } from "./blobStore"

export type BootOptions = {
  readonly shellSelector?: string
  readonly outletSelector?: string
}

/**
 * Client boot for the Effuse-hosted app.
 *
 * Strict hydration default:
 * - boot MUST NOT call `DomService.swap`
 * - router.start installs navigation listeners but does not re-render
 */
export const bootEffuseApp = (options?: BootOptions): void => {
  const shellSelector = options?.shellSelector ?? "[data-effuse-shell]"
  const outletSelector = options?.outletSelector ?? "[data-effuse-outlet]"
  const shell = document.querySelector(shellSelector)

  if (!shell) {
    console.error(`[EffuseApp] Shell not found: ${shellSelector}`)
    return
  }

  const runtime = makeAppRuntime(getAppConfig())
  const atoms = makeAtomRegistry()
  hydrateAtomRegistryFromDocument(atoms)

  // Best-effort analytics: load PostHog before any telemetry events.
  loadPostHog()

  const ezRegistry = makeEzRegistry()
  ezRegistry.set("effuse.blob.view", ({ params }) =>
    Effect.sync(() => {
      const blobId = params.blobId ?? params.id ?? ""
      if (!blobId) {
        return html`[missing blobId]`
      }
      const text = UiBlobStore.getText(blobId)
      if (text == null) {
        return html`[blob not found: ${blobId}]`
      }
      return html`${text}`
    }),
  )

  // Extract service clients once so EZ actions can run without requiring Effect env.
  const telemetry = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService
    }),
  )
  const api = runtime.runSync(
    Effect.gen(function* () {
      return yield* AgentApiService
    }),
  )
  const chat = runtime.runSync(
    Effect.gen(function* () {
      return yield* ChatService
    }),
  )

  // Identify user in PostHog (best-effort) when SessionAtom becomes authenticated.
  let lastIdentifiedUserId: string | null = null
  atoms.subscribe(
    SessionAtom,
    (session) => {
      if (!session.userId) return
      if (session.userId === lastIdentifiedUserId) return
      lastIdentifiedUserId = session.userId
      Effect.runPromise(telemetry.withNamespace("auth.workos").identify(session.userId, { userId: session.userId })).catch(() => {})
    },
    { immediate: true },
  )

  runtime
    .runPromise(
      Effect.gen(function* () {
        // Mount delegated hypermedia runtime on the shell (does not mutate DOM).
        yield* mountEzRuntimeWith(shell, ezRegistry)

        const router = yield* makeRouter({
          routes: appRoutes,
          history: BrowserHistory,
          shell,
          outletSelector,
          sessionScopeKey: Effect.flatMap(AuthService, (auth) => auth.sessionScopeKey()),
        })

        // Install listeners (strict hydration: no swap).
        yield* router.start

        const outlet = shell.querySelector(outletSelector)
        if (!(outlet instanceof Element)) {
          console.error(`[EffuseApp] Outlet not found: ${outletSelector}`)
          return
        }

        const navigate = (href: string) => {
          runtime
            .runPromise(router.navigate(href).pipe(Effect.provide(EffuseLive)))
            .catch(() => {})
        }

        type ActiveController = { readonly kind: string; readonly cleanup: () => void }
        let active: ActiveController | null = null

        const stopActive = () => {
          if (!active) return
          try {
            active.cleanup()
          } catch (err) {
            console.warn("[EffuseApp] controller cleanup failed", err)
          }
          active = null
        }

        const startForPath = (pathname: string) => {
          const desired =
            pathname === "/autopilot"
              ? "autopilot"
              : pathname === "/"
                ? "home"
              : pathname === "/login"
                ? "login"
                : pathname === "/modules"
                  ? "modules"
                  : pathname === "/tools"
                    ? "tools"
                    : pathname === "/signatures"
                      ? "signatures"
                      : "none"

          if (desired === active?.kind) return
          stopActive()

          switch (desired) {
            case "home": {
              active = {
                kind: "home",
                cleanup: mountHomeController({ container: outlet }).cleanup,
              }
              return
            }
            case "autopilot": {
              active = {
                kind: "autopilot",
                cleanup: mountAutopilotController({
                  container: outlet,
                  ez: ezRegistry,
                  atoms,
                  telemetry,
                  api,
                  chat,
                  router,
                  navigate,
                }).cleanup,
              }
              return
            }
            case "login": {
              active = {
                kind: "login",
                cleanup: mountLoginController({
                  container: outlet,
                  ez: ezRegistry,
                  runtime,
                  atoms,
                  telemetry,
                  navigate,
                }).cleanup,
              }
              return
            }
            case "modules": {
              active = {
                kind: "modules",
                cleanup: mountModulesController({ container: outlet, atoms }).cleanup,
              }
              return
            }
            case "tools": {
              active = {
                kind: "tools",
                cleanup: mountToolsController({ container: outlet, atoms }).cleanup,
              }
              return
            }
            case "signatures": {
              active = {
                kind: "signatures",
                cleanup: mountSignaturesController({ container: outlet, atoms }).cleanup,
              }
              return
            }
            default:
              return
          }
        }

        let lastPathname = ""
        const onRouterState = (state: { readonly status: string; readonly url: URL }) => {
          const pathname = state.url.pathname
          if (pathname !== lastPathname) {
            lastPathname = pathname
            Effect.runPromise(telemetry.withNamespace("app").event("page_view", { path: pathname })).catch(() => {})
          }

          if (state.status === "navigating") {
            stopActive()
            return
          }

          if (state.status === "idle") {
            startForPath(pathname)
          }
        }

        yield* Stream.runForEach(router.state.changes, (s) =>
          Effect.sync(() => onRouterState(s as any)),
        ).pipe(Effect.forkDaemon)
      }).pipe(Effect.provide(EffuseLive))
    )
    .catch((err) => {
      console.error("[EffuseApp] boot failed", err)
    })
}
