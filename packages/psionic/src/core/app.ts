import { staticPlugin } from "@elysiajs/static"
import { Elysia } from "elysia"
import { discoverStories, renderComponentExplorer, renderStoryPage } from "../components/discovery"
import type { PsionicConfig, RouteHandler } from "../types"

export class PsionicApp {
  private app: Elysia
  private config: PsionicConfig

  constructor(config: PsionicConfig) {
    this.config = config
    this.app = new Elysia()

    // Add static file serving if configured
    if (config.staticDir) {
      this.app.use(staticPlugin({
        assets: config.staticDir,
        prefix: ""
      }))
    }

    // Add component explorer routes if enabled
    if (config.enableComponents !== false) {
      this.setupComponentExplorer()
    }

    // Add catch-all redirect to root by default
    if (config.catchAllRedirect !== false) {
      this.app.onError(({ code, set }) => {
        if (code === "NOT_FOUND") {
          set.status = 302
          set.headers["location"] = "/"
        }
      })
    }
  }

  private async setupComponentExplorer() {
    const componentsPath = this.config.componentsPath || "/components"
    const componentsDir = this.config.componentsDir || "stories"

    // Main component explorer route
    this.app.get(componentsPath, async ({ set }) => {
      try {
        const stories = await discoverStories(componentsDir)
        const html = renderComponentExplorer(stories, componentsPath)
        set.headers["content-type"] = "text/html; charset=utf-8"
        return html
      } catch (error) {
        console.error("Error rendering component explorer:", error)
        set.status = 500
        return "Error loading component explorer"
      }
    })

    // Individual story routes: /components/:component/:story
    this.app.get(`${componentsPath}/:component/:story`, async ({ params, set }) => {
      try {
        const stories = await discoverStories(componentsDir)
        const storyModule = stories.find((m) => m.title === params.component)

        if (!storyModule) {
          set.status = 404
          return "Component not found"
        }

        const story = storyModule.stories[params.story]
        if (!story) {
          set.status = 404
          return "Story not found"
        }

        const html = renderStoryPage(storyModule, params.story, story, componentsPath)
        set.headers["content-type"] = "text/html; charset=utf-8"
        return html
      } catch (error) {
        console.error("Error rendering story page:", error)
        set.status = 500
        return "Error loading story"
      }
    })

    console.log(`📚 Component explorer enabled at ${componentsPath}`)
  }

  route(path: string, handler: RouteHandler) {
    this.app.get(path, async ({ set, ...context }) => {
      const result = await handler(context)

      // If result is HTML string, set content type
      if (typeof result === "string" && result.trim().startsWith("<")) {
        set.headers["content-type"] = "text/html; charset=utf-8"
      }

      return result
    })

    return this
  }

  start() {
    const port = this.config.port || 3000
    this.app.listen(port)

    console.log(`🧠 ${this.config.name || "Psionic"} is running at http://localhost:${port}`)

    return this
  }

  // Expose underlying Elysia instance for advanced usage
  get elysia() {
    return this.app
  }
}

export function createPsionicApp(config: PsionicConfig) {
  return new PsionicApp(config)
}
