import { Elysia } from "elysia"
import type { PsionicConfig, RouteHandler } from "../types"

export class PsionicApp {
  private app: Elysia
  private config: PsionicConfig
  
  constructor(config: PsionicConfig) {
    this.config = config
    this.app = new Elysia()
    
    // Add catch-all redirect to root by default
    if (config.catchAllRedirect !== false) {
      this.app.onError(({ code, set }) => {
        if (code === 'NOT_FOUND') {
          set.status = 302
          set.headers['location'] = '/'
        }
      })
    }
  }
  
  route(path: string, handler: RouteHandler) {
    this.app.get(path, async ({ set, ...context }) => {
      const result = await handler(context)
      
      // If result is HTML string, set content type
      if (typeof result === 'string' && result.trim().startsWith('<')) {
        set.headers['content-type'] = 'text/html; charset=utf-8'
      }
      
      return result
    })
    
    return this
  }
  
  start() {
    const port = this.config.port || 3000
    this.app.listen(port)
    
    console.log(`🧠 ${this.config.name || 'Psionic'} is running at http://localhost:${port}`)
    
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