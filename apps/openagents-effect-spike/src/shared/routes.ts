// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { Route } from "@effect/ui"

export const AppRoute = Route.union({
  Home: Route.make("/"),
  NotFound: Route.make("/*")
})
export type AppRoute = typeof AppRoute.Type
