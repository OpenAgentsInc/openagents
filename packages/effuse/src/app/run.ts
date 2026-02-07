import { Effect, Exit } from "effect"
import type { TemplateResult } from "../template/types.js"
import type {
  HydrationMode,
  NavigationSwapMode,
  Route,
  RouteContext,
  RouteHead,
  RouteMatch,
  RouteOkHints,
  RouteOutcome,
  RouteId,
} from "./route.js"

export type RouteRunStage = "guard" | "loader" | "head" | "view"

export type RouteRun<A> =
  | {
      readonly _tag: "Ok"
      readonly routeId: RouteId
      readonly match: RouteMatch
      readonly hydration: HydrationMode
      readonly navigationSwap: NavigationSwapMode
      readonly data: A
      readonly template: TemplateResult
      readonly head?: RouteHead
      readonly hints?: RouteOkHints
    }
  | {
      readonly _tag: "Redirect"
      readonly routeId: RouteId
      readonly match: RouteMatch
      readonly hydration: HydrationMode
      readonly navigationSwap: NavigationSwapMode
      readonly href: string
      readonly status?: 301 | 302 | 303 | 307 | 308
    }
  | {
      readonly _tag: "NotFound"
      readonly routeId: RouteId
      readonly match: RouteMatch
      readonly hydration: HydrationMode
      readonly navigationSwap: NavigationSwapMode
    }
  | {
      readonly _tag: "Fail"
      readonly routeId: RouteId
      readonly match: RouteMatch
      readonly hydration: HydrationMode
      readonly navigationSwap: NavigationSwapMode
      readonly status?: number
      readonly error: unknown
      readonly stage: RouteRunStage
    }

const defaults = <A, R>(route: Route<A, R>) => ({
  hydration: route.hydration ?? ("strict" satisfies HydrationMode),
  navigationSwap: route.navigation?.swap ?? ("outlet" satisfies NavigationSwapMode),
})

const normalizeNonOkOutcome = (
  routeId: RouteId,
  match: RouteMatch,
  hydration: HydrationMode,
  navigationSwap: NavigationSwapMode,
  stage: RouteRunStage,
  outcome: RouteOutcome<unknown>
): Exclude<RouteRun<unknown>, { readonly _tag: "Ok" }> => {
  switch (outcome._tag) {
    case "Redirect":
      return {
        _tag: "Redirect",
        routeId,
        match,
        hydration,
        navigationSwap,
        href: outcome.href,
        ...(outcome.status ? { status: outcome.status } : {}),
      }
    case "NotFound":
      return { _tag: "NotFound", routeId, match, hydration, navigationSwap }
    case "Fail":
      return {
        _tag: "Fail",
        routeId,
        match,
        hydration,
        navigationSwap,
        ...(outcome.status ? { status: outcome.status } : {}),
        error: outcome.error,
        stage,
      }
    case "Ok":
      return {
        _tag: "Fail",
        routeId,
        match,
        hydration,
        navigationSwap,
        status: 500,
        error: new Error("internal: expected non-Ok RouteOutcome"),
        stage,
      }
  }
}

/**
 * Execute a matched route (guard -> loader -> head/view) and normalize results into `RouteRun`.
 *
 * Notes:
 * - If `guard` returns a `RouteOutcome`, the run short-circuits.
 * - Any defect during guard/loader/head/view is normalized into `Fail` with `stage`.
 */
export const runRoute = <A, R>(
  route: Route<A, R>,
  ctx: RouteContext
): Effect.Effect<RouteRun<A>, never, R> => {
  const { hydration, navigationSwap } = defaults(route)
  const routeId = route.id
  const match = ctx.match

  const failFromCause = (stage: RouteRunStage, error: unknown): RouteRun<A> => ({
    _tag: "Fail",
    routeId,
    match,
    hydration,
    navigationSwap,
    status: 500,
    error,
    stage,
  })

  return Effect.gen(function* () {
    if (route.guard) {
      const guardExit = yield* route.guard(ctx).pipe(Effect.exit)
      if (Exit.isFailure(guardExit)) {
        return failFromCause("guard", guardExit.cause)
      }
      const guardResult = guardExit.value
      if (guardResult && typeof guardResult === "object" && "_tag" in guardResult) {
        if (guardResult._tag === "Ok") {
          return failFromCause("guard", new Error("Route.guard MUST NOT return Ok"))
        }
        return normalizeNonOkOutcome(
          routeId,
          match,
          hydration,
          navigationSwap,
          "guard",
          guardResult
        ) as RouteRun<A>
      }
    }

    const loaderExit = yield* route.loader(ctx).pipe(Effect.exit)
    if (Exit.isFailure(loaderExit)) {
      return failFromCause("loader", loaderExit.cause)
    }

    const outcome = loaderExit.value
    switch (outcome._tag) {
      case "Ok": {
        const data = outcome.data

        const head =
          route.head != null
            ? yield* route.head(ctx, data).pipe(
                Effect.exit,
                Effect.map((exit) =>
                  Exit.isFailure(exit)
                    ? failFromCause("head", exit.cause)
                    : exit.value
                )
              )
            : undefined

        if (head && typeof head === "object" && "_tag" in head) {
          return head as RouteRun<A>
        }

        const viewExit = yield* route.view(ctx, data).pipe(Effect.exit)
        if (Exit.isFailure(viewExit)) {
          return failFromCause("view", viewExit.cause)
        }

        return {
          _tag: "Ok",
          routeId,
          match,
          hydration,
          navigationSwap,
          data,
          template: viewExit.value,
          ...(route.head ? { head: head as RouteHead } : {}),
          ...(outcome.hints ? { hints: outcome.hints } : {}),
        }
      }
      case "Redirect":
      case "NotFound":
      case "Fail":
        return normalizeNonOkOutcome(
          routeId,
          match,
          hydration,
          navigationSwap,
          "loader",
          outcome
        ) as RouteRun<A>
    }
  })
}
