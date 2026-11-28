// Firm reason: 1 (Effect UI browser hydration entry wiring SessionStream, SessionCommands, BrowserRouter, and DevTools for docs/effect/half-day-sprint.md)
import { Router } from "@effect/ui"
import { DevTools } from "@effect/ui-devtools"
import { BrowserHost, BrowserRouter, HtmlHydrate } from "@effect/ui/html"
import { Effect, Layer } from "effect"
import { App } from "./App.ts"
import { SessionCommandsBrowser } from "./commands/SessionCommands.ts"
import { SessionStream } from "./queries/SessionStream.ts"
import { AppRoute } from "../shared/routes.ts"

const Services = Layer.mergeAll(
  BrowserRouter.layer({ routes: AppRoute }),
  SessionStream.layerBrowser,
  SessionCommandsBrowser,
  import.meta.env.DEV ? DevTools.layer({ position: "BottomRight" }) : Layer.empty
)

const Main = Layer.effectDiscard(
  Effect.gen(function*() {
    const rootEl = document.getElementById("app")
    if (!rootEl) return
    const stateEl = document.getElementById("state")
    if (stateEl?.textContent) {
      const root = yield* HtmlHydrate.hydrateEffect(
        App,
        Router.props(AppRoute),
        rootEl,
        JSON.parse(stateEl.textContent)
      )
      yield* root.adopted
    } else {
      yield* BrowserHost.mountEffect(App, Router.props(AppRoute), rootEl)
    }
  })
).pipe(Layer.provide(Services))

Layer.launch(Main).pipe(Effect.runFork)
