import type { HttpServerRequest } from "@effect/platform"
import type { Effect } from "effect"

export interface PsionicConfig {
  name?: string
  port?: number
  host?: string // Hostname to bind to (default: "localhost")
  catchAllRedirect?: boolean
  staticDir?: string // Path to static files directory
  // Component explorer configuration
  componentsDir?: string // Default: "stories"
  componentsPath?: string // Default: "/components"
  enableComponents?: boolean // Default: true
  componentExplorerOptions?: {
    styles?: string
    navigation?: string
    baseClass?: string
  }
  // Tailwind CSS configuration
  tailwind?: {
    enabled?: boolean // Default: true
    cdn?: boolean // Default: true (use Play CDN)
    config?: string // Custom @theme CSS configuration
  }
  // Future: WebSocket relay configuration
  // relays?: string[]
}

export interface RouteContext {
  request: HttpServerRequest.HttpServerRequest
  params: Record<string, string>
}

// Support both legacy Promise handlers and new Effect handlers
export type RouteHandler =
  | ((context: RouteContext) => Effect.Effect<any, any, any>)
  | ((context: any) => string | Promise<string> | Response | Promise<Response> | any)

export interface ComponentExplorerOptions {
  styles?: string
  navigation?: string
  baseClass?: string
}

// Future: Component interface
export interface PsionicComponent {
  render(): string
}

// Future: WebSocket event types
export interface PsionicEvent {
  id: string
  type: string
  data: any
  timestamp: number
}

// Component story interfaces
export interface PsionicStory {
  name: string
  html: string
  description?: string
  props?: Record<string, any>
}

export interface StoryModule {
  title: string
  component?: string
  stories: Record<string, PsionicStory>
  filePath: string
}
