import { Layer } from "effect"
import { TauriEventServiceLive } from "./TauriEventService"
import { ClaudeStreamingServiceLive } from "./ClaudeStreamingService"

// Export all services
export * from "./TauriEventService"
export * from "./ClaudeStreamingService"

// Application Layer composition
export const ServicesLayer = Layer.mergeAll(
  TauriEventServiceLive,
  ClaudeStreamingServiceLive
)

// Individual service layers for testing
export const TestServicesLayer = {
  TauriEvent: TauriEventServiceLive,
  ClaudeStreaming: ClaudeStreamingServiceLive
}