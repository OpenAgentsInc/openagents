import { HttpRouter, HttpServer, HttpServerResponse } from "@effect/platform"
import { BunHttpServer, BunRuntime } from "@effect/platform-bun"
import { Effect, Layer } from "effect"
import type { PsionicConfig, RouteHandler } from "../types"

export class PsionicApp {
  private routes: Array<{ path: string; handler: RouteHandler }> = []
  public config: PsionicConfig

  constructor(config: PsionicConfig) {
    this.config = config
  }

  // Temporary getter for backward compatibility
  // TODO: Remove when all apps are migrated to Effect-based API
  get elysia(): any {
    throw new Error(
      "Psionic has been refactored to use Effect HTTP server. " +
      "Direct access to Elysia is no longer supported. " +
      "Please update your code to use the Effect-based API."
    )
  }

  route(path: string, handler: RouteHandler) {
    this.routes.push({ path, handler })
    return this
  }

  start() {
    const port = this.config.port || 3000
    const host = this.config.host || "localhost"

    // Build router by chaining routes
    let router = HttpRouter.empty

    for (const { handler, path } of this.routes) {
      router = router.pipe(
        HttpRouter.get(
          path as any, // Temporary type assertion
          Effect.gen(function*() {
            const result = yield* Effect.promise(() => Promise.resolve(handler({})))

            // If result looks like HTML, set content type
            if (typeof result === "string" && result.trim().startsWith("<")) {
              return HttpServerResponse.html(result)
            }

            return HttpServerResponse.text(result)
          })
        )
      )
    }

    const HttpLive = HttpServer.serve(router).pipe(
      Layer.provide(BunHttpServer.layer({ port }))
    )

    BunRuntime.runMain(Layer.launch(HttpLive))

    console.log(`🧠 ${this.config.name || "Psionic"} is running at http://${host}:${port}`)

    return this
  }
}

export function createPsionicApp(config: PsionicConfig) {
  return new PsionicApp(config)
}
