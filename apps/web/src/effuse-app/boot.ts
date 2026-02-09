import { Effect, Stream } from "effect"
import {
  BrowserHistory,
  DomServiceTag,
  EffuseLive,
  html,
  makeEzRegistry,
  makeRouter,
  mountEzRuntimeWith,
} from "@openagentsinc/effuse"

import { AuthService, clearAuthClientCache } from "../effect/auth"
import { AutopilotStoreService } from "../effect/autopilotStore"
import { ChatService } from "../effect/chat"
import { getAppConfig } from "../effect/config"
import { ContractsApiService } from "../effect/contracts"
import { OwnedThreadIdAtom } from "../effect/atoms/chat"
import { SessionAtom, type Session } from "../effect/atoms/session"
import { makeAppRuntime } from "../effect/runtime"
import { TelemetryService } from "../effect/telemetry"
import { hydrateAtomRegistryFromDocument, makeAtomRegistry } from "./atomRegistry"
import { mountAutopilotController } from "./controllers/autopilotController"
import { mountModulesController, mountSignaturesController, mountToolsController } from "./controllers/contractsController"
import { mountDeckController } from "./controllers/deckController"
import { mountHomeController } from "./controllers/homeController"
import { mountLoginController } from "./controllers/loginController"
import { identityPillTemplate } from "../effuse-pages/identityPill"
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
  const store = runtime.runSync(
    Effect.gen(function* () {
      return yield* AutopilotStoreService
    }),
  )
  const contracts = runtime.runSync(
    Effect.gen(function* () {
      return yield* ContractsApiService
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

        // Identity pill: fixed bottom-left, shows user or "Not logged in", always visible.
        const pillContainer = document.createElement("div")
        pillContainer.setAttribute("data-identity-pill-root", "1")
        pillContainer.style.cssText =
          "position:fixed;bottom:12px;left:12px;z-index:9999;pointer-events:auto"
        shell.appendChild(pillContainer)

        const signOut = async () => {
          try {
            await fetch("/api/auth/signout", { method: "POST", credentials: "include" })
          } catch {
            // best-effort
          } finally {
            clearAuthClientCache()
            atoms.set(SessionAtom as any, { userId: null, user: null })
            atoms.set(OwnedThreadIdAtom as any, null)
            navigate("/")
          }
        }

        ezRegistry.set("app.identity.logout", () =>
          Effect.sync(() => {
            void signOut()
          }),
        )

        const renderIdentityPill = (session: Session) => {
          runtime
            .runPromise(
              Effect.gen(function* () {
                const dom = yield* DomServiceTag
                yield* dom.render(pillContainer, identityPillTemplate(session))
              }).pipe(Effect.provide(EffuseLive)),
            )
            .catch(() => {})
        }

        renderIdentityPill(atoms.get(SessionAtom))
        atoms.subscribe(SessionAtom, renderIdentityPill, { immediate: false })

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
                  : pathname === "/deck"
                    ? "deck"
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
                  runtime,
                  atoms,
                  telemetry,
                  store,
                  contracts,
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
            case "deck": {
              active = {
                kind: "deck",
                cleanup: mountDeckController({ container: outlet }).cleanup,
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
