import { Effect, Stream } from "effect"
import {
  BrowserHistory,
  EffuseLive,
  html,
  makeEzRegistry,
  makeRouter,
  mountEzRuntimeWith,
} from "@openagentsinc/effuse"

import { AuthService, clearAuthClientCache } from "../effect/auth"
import { AutopilotStoreService } from "../effect/autopilotStore"
import { ChatService } from "../effect/chat"
import { ConvexService } from "../effect/convex"
import { getAppConfig } from "../effect/config"
import { ContractsApiService } from "../effect/contracts"
import { OwnedThreadIdAtom } from "../effect/atoms/chat"
import { SessionAtom } from "../effect/atoms/session"
import { makeAppRuntime } from "../effect/runtime"
import { TelemetryService } from "../effect/telemetry"
import { hydrateAtomRegistryFromDocument, makeAtomRegistry } from "./atomRegistry"
import { mountModulesController, mountSignaturesController, mountToolsController } from "./controllers/contractsController"
import { mountDseVizController } from "./controllers/dseVizController"
import { mountDeckController } from "./controllers/deckController"
import { mountHomeController } from "./controllers/homeController"
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
  const _store = runtime.runSync(
    Effect.gen(function* () {
      return yield* AutopilotStoreService
    }),
  )
  const _contracts = runtime.runSync(
    Effect.gen(function* () {
      return yield* ContractsApiService
    }),
  )
  const chat = runtime.runSync(
    Effect.gen(function* () {
      return yield* ChatService
    }),
  )
  const convex = runtime.runSync(
    Effect.gen(function* () {
      return yield* ConvexService
    }),
  )

  // Identify user in PostHog (best-effort) when SessionAtom becomes authenticated.
  let lastIdentifiedUserId: string | null = null
  let lastConvexAuthedUserId: string | null = null
  atoms.subscribe(
    SessionAtom,
    (session) => {
      if (!session.userId) {
        lastIdentifiedUserId = null
        lastConvexAuthedUserId = null
        return
      }

      if (session.userId !== lastIdentifiedUserId) {
        lastIdentifiedUserId = session.userId
        Effect.runPromise(
          telemetry.withNamespace("auth.workos").identify(session.userId, { userId: session.userId }),
        ).catch(() => { })
      }

      // Convex can become "stuck" unauthenticated if it booted before a user logged in.
      // Refresh auth whenever we observe a new authenticated userId.
      if (session.userId !== lastConvexAuthedUserId) {
        lastConvexAuthedUserId = session.userId
        runtime.runPromise(convex.refreshAuth()).catch(() => { })
      }
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
            .catch(() => { })
        }

        const signOut = async () => {
          try {
            await fetch("/api/auth/signout", { method: "POST", credentials: "include" })
          } catch {
            // best-effort
          } finally {
            clearAuthClientCache()
            atoms.set(SessionAtom as any, { userId: null, user: null })
            atoms.set(OwnedThreadIdAtom as any, null)
            // Full reload so Convex client and all in-memory state (including cached auth token) are dropped.
            // Prevents the next user from seeing the previous user's chats.
            window.location.replace("/")
          }
        }

        ezRegistry.set("app.identity.logout", () =>
          Effect.sync(() => {
            void signOut()
          }),
        )

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
            pathname === "/"
              ? "home"
              : pathname === "/deck"
                ? "deck"
                : pathname.startsWith("/dse")
                  ? `dse:${pathname}`
                  : pathname === "/modules"
                    ? "modules"
                    : pathname === "/tools"
                      ? "tools"
                      : pathname === "/signatures"
                        ? "signatures"
                        : "none"

          if (desired === "none") {
            navigate("/")
            return
          }

          if (desired === active?.kind) return
          stopActive()

          const desiredKind = desired.startsWith("dse:") ? "dse" : desired

          switch (desiredKind) {
            case "home": {
              active = {
                kind: "home",
                cleanup: mountHomeController({
                  container: outlet,
                  runtime,
                  atoms,
                  navigate,
                  signOut: () => void signOut(),
                  chat,
                  refreshConvexAuth: () => runtime.runPromise(convex.refreshAuth()),
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
            case "dse": {
              active = {
                kind: desired,
                cleanup: mountDseVizController({ container: outlet, atoms }).cleanup,
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
            Effect.runPromise(telemetry.withNamespace("app").event("page_view", { path: pathname })).catch(() => { })
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
