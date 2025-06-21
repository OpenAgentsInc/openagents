/**
 * Browser SDK exports
 * Effect.js services for frontend WebSocket operations
 */

import * as CloudflareLanguageModel from "@openagentsinc/ai/providers/cloudflare/CloudflareLanguageModel"
import * as NostrLib from "@openagentsinc/nostr"
import { Layer } from "effect"
import { AgentServiceLive } from "./AgentService.js"
import { AutonomousChatAgentLive } from "./AutonomousChatAgent.js"
import { AutonomousMarketplaceAgentLive } from "./AutonomousMarketplaceAgent.js"
import { ChannelServiceLive } from "./ChannelService.js"
import { ServiceOfferingServiceLive } from "./ServiceOfferingService.js"
import { SparkServiceLive } from "./SparkService.js"
import { WebSocketServiceLive } from "./WebSocketService.js"

export * from "./AgentService.js"
export * from "./AutonomousChatAgent.js"
export * from "./AutonomousMarketplaceAgent.js"
export * from "./ChannelService.js"
export * from "./ServiceOfferingService.js"
export * from "./SparkService.js"
export * from "./WebSocketService.js"

/**
 * Create Cloudflare Language Model layer for browser
 * Uses default Llama 3.1 8B model for autonomous agents
 */
export const createCloudflareLanguageModelLayer = (config?: {
  apiKey?: string
  accountId?: string
  model?: string
}) => {
  return CloudflareLanguageModel.layer({
    model: config?.model || "@cf/meta/llama-3.1-8b-instruct",
    temperature: 0.7,
    maxTokens: 200
  })
}

/**
 * Complete browser service layer with AI
 * Provides all WebSocket-based services and Cloudflare AI for frontend
 */
export const createBrowserServicesLayer = (cloudflareConfig?: {
  apiKey?: string
  accountId?: string
  model?: string
}) => {
  const ChannelWithWebSocketLive = ChannelServiceLive.pipe(Layer.provide(WebSocketServiceLive))
  const CloudflareLanguageModelLive = createCloudflareLanguageModelLayer(cloudflareConfig)

  // Nostr services for marketplace
  const NostrServicesLive = Layer.mergeAll(
    NostrLib.CryptoService.CryptoServiceLive,
    NostrLib.EventService.EventServiceLive,
    NostrLib.RelayService.RelayServiceLive.pipe(Layer.provide(NostrLib.WebSocketService.WebSocketServiceLive)),
    NostrLib.Nip90Service.Nip90ServiceLive.pipe(
      Layer.provide(Layer.merge(
        NostrLib.EventService.EventServiceLive,
        NostrLib.RelayService.RelayServiceLive.pipe(Layer.provide(NostrLib.WebSocketService.WebSocketServiceLive))
      ))
    )
  )

  return Layer.mergeAll(
    WebSocketServiceLive,
    ChannelWithWebSocketLive,
    AgentServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
    ServiceOfferingServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
    CloudflareLanguageModelLive,
    SparkServiceLive,
    AutonomousChatAgentLive.pipe(Layer.provide(Layer.merge(ChannelWithWebSocketLive, CloudflareLanguageModelLive))),
    AutonomousMarketplaceAgentLive.pipe(
      Layer.provide(Layer.mergeAll(
        ChannelWithWebSocketLive,
        CloudflareLanguageModelLive,
        AutonomousChatAgentLive.pipe(Layer.provide(Layer.merge(ChannelWithWebSocketLive, CloudflareLanguageModelLive))),
        NostrServicesLive,
        SparkServiceLive
      ))
    )
  )
}

/**
 * Default browser services layer
 * Uses environment variables for Cloudflare configuration
 */
export const BrowserServicesLive = createBrowserServicesLayer()
