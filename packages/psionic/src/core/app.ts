import {
  FetchHttpClient,
  HttpMiddleware,
  HttpRouter,
  HttpServer,
  HttpServerRequest,
  HttpServerResponse
} from "@effect/platform"
import { BunHttpServer, BunRuntime } from "@effect/platform-bun"
import { Effect, Layer, pipe } from "effect"
import { convertElysiaRouter } from "../adapters/elysia-adapter"
import { discoverStories, renderComponentExplorer, renderStoryPage } from "../components/discovery"
import type { ComponentExplorerOptions, PsionicConfig, RouteHandler } from "../types"
import { wrapHtmlWithTailwind } from "../utils/tailwind"

interface Route {
  method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH"
  path: string
  handler: RouteHandler
}

interface ApiRouter {
  routes: Array<Route>
  prefix?: string
}

export class PsionicApp {
  private routes: Array<Route> = []
  private apiRouters: Array<ApiRouter> = []
  private staticDirs: Array<{ urlPath: string; fsPath: string }> = []
  private componentConfig?: { storiesDir: string; options?: ComponentExplorerOptions }
  public config: PsionicConfig

  constructor(config: PsionicConfig) {
    this.config = config
    if (config.staticDir) {
      this.staticDirs.push({ urlPath: "/", fsPath: config.staticDir })
    }
  }

  // Route methods
  get(path: string, handler: RouteHandler) {
    this.routes.push({ method: "GET", path, handler })
    return this
  }

  post(path: string, handler: RouteHandler) {
    this.routes.push({ method: "POST", path, handler })
    return this
  }

  put(path: string, handler: RouteHandler) {
    this.routes.push({ method: "PUT", path, handler })
    return this
  }

  delete(path: string, handler: RouteHandler) {
    this.routes.push({ method: "DELETE", path, handler })
    return this
  }

  patch(path: string, handler: RouteHandler) {
    this.routes.push({ method: "PATCH", path, handler })
    return this
  }

  route(path: string, handler: RouteHandler) {
    return this.get(path, handler)
  }

  // Plugin methods
  static(urlPath: string, options: { path: string }) {
    this.staticDirs.push({ urlPath, fsPath: options.path })
    return this
  }

  components(storiesDir?: string, options?: ComponentExplorerOptions & { path?: string }) {
    const componentsPath = options?.path || this.config.componentsPath || "/components"
    const dir = storiesDir || this.config.componentsDir || "stories"

    const componentOptions: ComponentExplorerOptions = {}

    const styles = options?.styles || this.config.componentExplorerOptions?.styles
    if (styles) {
      componentOptions.styles = styles
    }

    const navigation = options?.navigation || this.config.componentExplorerOptions?.navigation
    if (navigation) {
      componentOptions.navigation = navigation
    }

    const baseClass = options?.baseClass || this.config.componentExplorerOptions?.baseClass
    if (baseClass) {
      componentOptions.baseClass = baseClass
    }

    this.componentConfig = {
      storiesDir: dir,
      options: componentOptions
    }

    // Add component explorer routes
    this.get(componentsPath, async () => {
      try {
        const stories = await discoverStories(this.componentConfig!.storiesDir)
        return renderComponentExplorer(stories, componentsPath, this.componentConfig!.options)
      } catch (error) {
        console.error("Error rendering component explorer:", error)
        return new Response("Error loading component explorer", { status: 500 })
      }
    })

    this.get(`${componentsPath}/:component/:story`, async (context: any) => {
      try {
        const stories = await discoverStories(this.componentConfig!.storiesDir)
        const componentName = decodeURIComponent(context.params.component)
        const storyName = decodeURIComponent(context.params.story)

        const storyModule = stories.find((m) => m.title === componentName)
        if (!storyModule) {
          return new Response("Component not found", { status: 404 })
        }

        const story = storyModule.stories[storyName]
        if (!story) {
          return new Response("Story not found", { status: 404 })
        }

        return renderStoryPage(
          storyModule,
          storyName,
          story,
          componentsPath,
          this.componentConfig!.options
        )
      } catch (error) {
        console.error("Error rendering story page:", error)
        return new Response("Error loading story", { status: 500 })
      }
    })

    console.log(`📚 Component explorer enabled at ${componentsPath}`)
    return this
  }

  docs(docsDir: string, options?: { path?: string }) {
    const docsPath = options?.path || "/docs"

    this.get(`${docsPath}/*`, async (context: any) => {
      try {
        const url = new URL(context.request.url)
        let filePath = url.pathname.slice(docsPath.length)

        if (!filePath || filePath === "/") {
          filePath = "/index.md"
        }

        if (!filePath.endsWith(".md")) {
          filePath += ".md"
        }

        const fullPath = docsDir + filePath
        const file = Bun.file(fullPath)

        if (!(await file.exists())) {
          return new Response("Not found", { status: 404 })
        }

        const content = await file.text()

        // Simple markdown rendering
        const html = `
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Documentation</title>
  <style>
    body { 
      font-family: system-ui, -apple-system, sans-serif;
      max-width: 800px;
      margin: 0 auto;
      padding: 2rem;
      line-height: 1.6;
    }
    pre { 
      background: #f4f4f4;
      padding: 1rem;
      overflow-x: auto;
    }
    code {
      background: #f4f4f4;
      padding: 0.2rem 0.4rem;
    }
  </style>
</head>
<body>
  <div class="markdown-content">
    ${content.replace(/</g, "&lt;").replace(/>/g, "&gt;")}
  </div>
</body>
</html>`

        return new Response(html, {
          headers: { "content-type": "text/html" }
        })
      } catch (error) {
        console.error("Error serving docs:", error)
        return new Response("Error loading documentation", { status: 500 })
      }
    })

    console.log(`📖 Docs enabled at ${docsPath}`)
    return this
  }

  api(router: any, options?: { prefix?: string }) {
    // Handle Elysia routers
    if (router && typeof router === "object" && router.routes) {
      const prefix = options?.prefix || ""
      const convertedRoutes = convertElysiaRouter(router, prefix)

      this.apiRouters.push({
        routes: convertedRoutes,
        prefix
      })

      console.log(`🔌 API router mounted${prefix ? ` at ${prefix}` : ""}`)
    }

    return this
  }

  websocket(path: string, _handler: any) {
    console.log(`🔌 WebSocket endpoint registered at ${path} (not implemented in Effect version)`)
    // Store for future implementation
    return this
  }

  // Legacy compatibility
  get elysia(): any {
    return {
      use: (plugin: any) => {
        if (plugin && typeof plugin === "function") {
          plugin(this)
        }
        return this.elysia
      },
      get: (path: string, handler: any) => {
        this.get(path, handler)
        return this.elysia
      },
      post: (path: string, handler: any) => {
        this.post(path, handler)
        return this.elysia
      },
      options: (path: string, handler: any) => {
        // For now, just use GET as OPTIONS isn't implemented yet
        this.get(path, handler)
        return this.elysia
      },
      ws: (path: string, handlers: any) => {
        this.websocket(path, handlers)
        return this.elysia
      },
      group: (prefix: string, fn: any) => {
        const groupRouter = {
          routes: [] as Array<any>,
          get(path: string, handler: any) {
            this.routes.push({ method: "GET", path: prefix + path, handler })
            return this
          },
          post(path: string, handler: any) {
            this.routes.push({ method: "POST", path: prefix + path, handler })
            return this
          }
        }
        fn(groupRouter)
        this.api(groupRouter)
        return this.elysia
      }
    }
  }

  private buildRouter() {
    let router: any = HttpRouter.empty

    // Add user-defined routes
    for (const route of this.routes) {
      router = this.addRoute(router, route)
    }

    // Add API routes
    for (const apiRouter of this.apiRouters) {
      for (const route of apiRouter.routes) {
        router = this.addRoute(router, route)
      }
    }

    // Add static file serving
    for (const { fsPath, urlPath } of this.staticDirs) {
      // Static file handler
      const staticHandler = Effect.gen(function*() {
        const request = yield* HttpServerRequest.HttpServerRequest
        const url = request.url.startsWith("http")
          ? new URL(request.url)
          : new URL(request.url, `http://localhost`)
        let filePath = url.pathname

        // Remove the URL path prefix
        if (urlPath !== "/" && filePath.startsWith(urlPath)) {
          filePath = filePath.slice(urlPath.length)
        }

        // Ensure leading slash
        if (!filePath.startsWith("/")) {
          filePath = "/" + filePath
        }

        // Default to index.html for directories
        if (filePath.endsWith("/")) {
          filePath += "index.html"
        }

        const fullPath = fsPath + filePath

        try {
          // Use Bun's file API
          const file = Bun.file(fullPath)
          const exists = yield* Effect.promise(() => file.exists())

          if (!exists) {
            return HttpServerResponse.text("Not found", { status: 404 })
          }

          const content = yield* Effect.promise(() => file.arrayBuffer())
          const ext = fullPath.split(".").pop() || ""

          // Determine content type
          let contentType = "application/octet-stream"
          if (ext === "html") contentType = "text/html"
          else if (ext === "css") contentType = "text/css"
          else if (ext === "js") contentType = "text/javascript"
          else if (ext === "json") contentType = "application/json"
          else if (ext === "png") contentType = "image/png"
          else if (ext === "jpg" || ext === "jpeg") contentType = "image/jpeg"
          else if (ext === "ico") contentType = "image/x-icon"
          else if (ext === "svg") contentType = "image/svg+xml"
          else if (ext === "txt") contentType = "text/plain"

          return HttpServerResponse.raw(content, {
            headers: {
              "content-type": contentType
            }
          })
        } catch (error) {
          console.error("Static file error:", error)
          return HttpServerResponse.text("Not found", { status: 404 })
        }
      })

      router = pipe(
        router,
        HttpRouter.get(`${urlPath}*` as any, staticHandler)
      )
    }

    // Don't add a catch-all 404 handler as it conflicts with static file serving
    // The static handler already returns 404 for missing files

    return router
  }

  private addRoute(router: HttpRouter.HttpRouter<any, any>, route: Route) {
    const { handler, method, path } = route
    const config = this.config

    // Convert our RouteHandler to Effect handler
    const effectHandler = Effect.gen(function*() {
      const request = yield* HttpServerRequest.HttpServerRequest

      // Create context object
      const context = {
        request,
        params: {} as Record<string, string>
      }

      // Extract route parameters if any
      const paramRegex = /:(\w+)/g
      const params: Array<string> = []
      let match
      while ((match = paramRegex.exec(path)) !== null) {
        params.push(match[1])
      }

      if (params.length > 0) {
        // TODO: Proper param extraction with Effect Router
        // For now, we'll do basic pattern matching
        const pathname = request.url

        // Convert :param to regex pattern
        let pattern = path
        params.forEach(() => {
          pattern = pattern.replace(/:(\w+)/, "([^/]+)")
        })
        pattern = `^${pattern}$`

        const regex = new RegExp(pattern)
        const routeMatch = pathname.match(regex)

        if (routeMatch) {
          params.forEach((param, index) => {
            context.params[param] = routeMatch[index + 1]
          })
        }
      }

      // Handle the request
      try {
        // Check if handler returns an Effect
        const handlerResult = handler(context)
        console.log(`🔍 PSIONIC: Route ${path} handler returned:`, typeof handlerResult, Effect.isEffect(handlerResult))

        // If it's an Effect, yield it directly
        if (Effect.isEffect(handlerResult)) {
          console.log(`✅ PSIONIC: Route ${path} returned Effect, yielding it`)
          return yield* handlerResult
        }

        // Otherwise, handle legacy Promise/value returns
        const result = yield* Effect.promise(() => Promise.resolve(handlerResult))

        // Handle different response types
        if (result instanceof Response) {
          const arrayBuffer = yield* Effect.promise(() => result.arrayBuffer())
          return HttpServerResponse.raw(arrayBuffer, {
            status: result.status,
            headers: Object.fromEntries(result.headers.entries())
          })
        } else if (typeof result === "string") {
          if (result.trim().startsWith("<")) {
            // Inject Tailwind if enabled
            const htmlWithTailwind = wrapHtmlWithTailwind(result, config)
            return HttpServerResponse.html(htmlWithTailwind)
          } else {
            return HttpServerResponse.text(result)
          }
        } else if (typeof result === "object") {
          return HttpServerResponse.json(result)
        } else {
          return HttpServerResponse.text(String(result))
        }
      } catch (error) {
        console.error(`❌ PSIONIC: Route ${path} handler error:`, error)
        return HttpServerResponse.json(
          { error: "Internal server error" },
          { status: 500 }
        )
      }
    })

    // Ensure path starts with /
    const routePath = path.startsWith("/") ? path : `/${path}`

    // Add route based on method
    switch (method) {
      case "GET":
        return pipe(router, HttpRouter.get(routePath as any, effectHandler as any))
      case "POST":
        return pipe(router, HttpRouter.post(routePath as any, effectHandler as any))
      case "PUT":
        return pipe(router, HttpRouter.put(routePath as any, effectHandler as any))
      case "DELETE":
        return pipe(router, HttpRouter.del(routePath as any, effectHandler as any))
      case "PATCH":
        return pipe(router, HttpRouter.patch(routePath as any, effectHandler as any))
      default:
        return router
    }
  }

  start() {
    const port = this.config.port || 3000
    const host = this.config.host || "localhost"

    const router = this.buildRouter()

    console.log("🔧 PSIONIC: Setting up HTTP server with FetchHttpClient.layer")
    const HttpLive = pipe(
      HttpServer.serve(HttpMiddleware.logger(router)),
      HttpServer.withLogAddress,
      Layer.provide(
        Layer.merge(
          BunHttpServer.layer({ port, hostname: host }),
          FetchHttpClient.layer
        )
      )
    )
    console.log("✅ PSIONIC: HTTP server layers configured")

    BunRuntime.runMain(Layer.launch(HttpLive) as any)

    console.log(`🧠 ${this.config.name || "Psionic"} is running at http://${host}:${port}`)

    return this
  }
}

export function createPsionicApp(config: PsionicConfig) {
  return new PsionicApp(config)
}
