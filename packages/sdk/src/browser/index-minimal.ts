/**
 * Minimal Browser SDK exports
 * Only includes services that actually work in browsers without Node.js dependencies
 */

import * as CloudflareLanguageModel from "@openagentsinc/ai/providers/cloudflare/CloudflareLanguageModel"
import { Layer } from "effect"
import { AgentServiceLive } from "./AgentService.js"
import { ChannelServiceLive } from "./ChannelService.js"
import { WebSocketServiceLive } from "./WebSocketService.js"

export * from "./AgentService.js"
export * from "./ChannelService.js"
export * from "./WebSocketService.js"

/**
 * Create Cloudflare Language Model layer for browser
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
 * Minimal browser service layer
 * Only includes WebSocket-based services that work in browsers
 */
export const createBrowserServicesLayer = (cloudflareConfig?: {
  apiKey?: string
  accountId?: string
  model?: string
}) => {
  const ChannelWithWebSocketLive = ChannelServiceLive.pipe(Layer.provide(WebSocketServiceLive))
  const CloudflareLanguageModelLive = createCloudflareLanguageModelLayer(cloudflareConfig)

  return Layer.mergeAll(
    WebSocketServiceLive,
    ChannelWithWebSocketLive,
    AgentServiceLive.pipe(Layer.provide(WebSocketServiceLive)),
    CloudflareLanguageModelLive
  )
}
