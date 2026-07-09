import type {
  Experimental_RealtimeClientEvent as RealtimeClientEvent,
  Experimental_RealtimeModel as RealtimeModel,
  Experimental_RealtimeServerEvent as RealtimeServerEvent,
  Experimental_RealtimeSessionConfig as RealtimeSessionConfig,
} from "ai";

const gatewayRealtimeSubprotocol = "ai-gateway-realtime.v1";
const gatewayAuthSubprotocolPrefix = "ai-gateway-auth.";

export function gatewayRealtimeBrowserModel(modelId: string): RealtimeModel {
  return {
    specificationVersion: "v4",
    provider: "gateway.realtime.browser",
    modelId,
    async doCreateClientSecret() {
      throw new Error("Use the /api/realtime/token setup endpoint.");
    },
    getWebSocketConfig({ token, url }) {
      return {
        url,
        protocols: [
          gatewayRealtimeSubprotocol,
          `${gatewayAuthSubprotocolPrefix}${token}`,
        ],
      };
    },
    parseServerEvent(raw: unknown) {
      return raw as RealtimeServerEvent;
    },
    serializeClientEvent(event: RealtimeClientEvent) {
      return event;
    },
    buildSessionConfig(config: RealtimeSessionConfig) {
      return config;
    },
  };
}
