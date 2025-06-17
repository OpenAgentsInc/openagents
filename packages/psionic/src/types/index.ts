export interface PsionicConfig {
  name?: string
  port?: number
  catchAllRedirect?: boolean
  staticDir?: string // Path to static files directory
  // Component explorer configuration
  componentsDir?: string // Default: "stories"
  componentsPath?: string // Default: "/components"
  enableComponents?: boolean // Default: true
  // Future: WebSocket relay configuration
  // relays?: string[]
}

export type RouteHandler = (context: any) => string | Promise<string>

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
