import { Effect } from "effect"
import {
  BrowserHistory,
  EffuseLive,
  makeEzRegistry,
  makeRouter,
  mountEzRuntimeWith,
} from "@openagentsinc/effuse"

import { AuthService } from "../effect/auth"
import { getAppConfig } from "../effect/config"
import { makeAppRuntime } from "../effect/runtime"
import { appRoutes } from "./routes"

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
  const ezRegistry = makeEzRegistry()

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
      }).pipe(Effect.provide(EffuseLive))
    )
    .catch((err) => {
      console.error("[EffuseApp] boot failed", err)
    })
}

