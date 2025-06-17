export interface PsionicConfig {
  name?: string
  port?: number
  catchAllRedirect?: boolean
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