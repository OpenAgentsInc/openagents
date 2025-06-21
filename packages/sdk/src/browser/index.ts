/**
 * Browser SDK exports
 * Effect.js services for frontend WebSocket operations
 */

import { Layer } from "effect"
import { AgentServiceLive } from "./AgentService.js"
import { ChannelServiceLive } from "./ChannelService.js"
import { ServiceOfferingServiceLive } from "./ServiceOfferingService.js"
import { WebSocketServiceLive } from "./WebSocketService.js"

export * from "./AgentService.js"
export * from "./ChannelService.js"
export * from "./ServiceOfferingService.js"
export * from "./WebSocketService.js"

/**
 * Complete browser service layer
 * Provides all WebSocket-based services for frontend
 */
export const BrowserServicesLive = Layer.mergeAll(
  WebSocketServiceLive,
  ChannelServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
  AgentServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
  ServiceOfferingServiceLive.pipe(Layer.provide(WebSocketServiceLive))
)
